use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::{AccumulationDelta, Accumulator, CapabilityFlags};
use crate::mysql::packet::Packet;
use crate::mysql::types::{
    Converter, IntFixedLen, IntLenEnc, StringFixedLen, StringLenEnc, StringNullEnc,
};
use std::any::Any;
use std::collections::HashMap;

const FILLER: &str = "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

/// https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase_packets_protocol_handshake_response.html#sect_protocol_connection_phase_packets_protocol_handshake_response41

#[derive(Debug, Default, Clone)]
pub struct HandshakeResponseAccumulator {
    pub client_flag: u32,
    max_packet_size: u32,
    character_set: u8,
    filler: String,
    username: String,
    auth_response_length: Option<u8>,
    auth_response: Option<String>,
    database: Option<String>,
    client_plugin_name: Option<String>,
    connection_attrs_length: u8,
    connection_attrs: HashMap<String, String>,
    zstd_compression_level: u8,
    accumulation_complete: bool,
}

impl Accumulator for HandshakeResponseAccumulator {
    fn consume(&mut self, packet: &Packet, _connection: &Connection) -> Phase {
        let mut offset: usize = 0;

        let client_flag = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(4));
            offset += result.offset_increment;
            result.result as u32
        };

        if client_flag & CapabilityFlags::ClientProtocol41 as u32 == 0 {
            panic!("Client protocol not supported");
        }

        let max_packet_size = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(4));
            offset += result.offset_increment;
            result.result as u32
        };
        let character_set = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
            offset += result.offset_increment;
            result.result as u8
        };
        let filler = {
            let result = StringFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(23));
            offset += result.offset_increment;
            assert!(FILLER.eq(&result.result));
            result.result
        };
        let username = {
            let result = StringNullEnc::from_bytes(&packet.body[offset..].to_vec(), None);
            offset += result.offset_increment;
            result.result
        };

        let auth_response_length;
        let auth_response;
        if client_flag & CapabilityFlags::ClientPluginAuthLenEncClientData as u32 != 0 {
            auth_response_length = None;
            auth_response = {
                let result = StringLenEnc::from_bytes(&packet.body[offset..].to_vec(), None);
                offset += result.offset_increment;
                Some(result.result)
            }
        } else {
            auth_response_length = {
                let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
                offset += result.offset_increment;
                Some(result.result as u8)
            };
            auth_response = {
                let result = StringFixedLen::from_bytes(
                    &packet.body[offset..].to_vec(),
                    Some(auth_response_length.unwrap() as usize),
                );
                offset += result.offset_increment;
                Some(result.result)
            }
        }

        let database;
        if client_flag & CapabilityFlags::ClientConnectWithDB as u32 != 0 {
            database = {
                let result = StringNullEnc::from_bytes(&packet.body[offset..].to_vec(), None);
                offset += result.offset_increment;
                Some(result.result)
            }
        } else {
            database = None;
        }

        let client_plugin_name;
        if client_flag & CapabilityFlags::ClientPluginAuth as u32 != 0 {
            client_plugin_name = {
                let result = StringNullEnc::from_bytes(&packet.body[offset..].to_vec(), None);
                offset += result.offset_increment;
                Some(result.result)
            }
        } else {
            client_plugin_name = None;
        }

        let connection_attrs_length;
        if client_flag & CapabilityFlags::ClientConnectAttrs as u32 != 0 {
            connection_attrs_length = {
                let result = IntLenEnc::from_bytes(&packet.body[offset..].to_vec(), None);
                offset += result.offset_increment;
                result.result as u8
            }
        } else {
            connection_attrs_length = 0
        }

        let mut connection_attrs = HashMap::new();
        let connection_attrs_start_offset = offset;
        while offset < connection_attrs_start_offset + connection_attrs_length as usize {
            let key = StringLenEnc::from_bytes(&packet.body[offset..].to_vec(), None);
            offset += key.offset_increment;
            let value = StringLenEnc::from_bytes(&packet.body[offset..].to_vec(), None);
            offset += value.offset_increment;
            connection_attrs.insert(key.result, value.result);
        }

        let zstd_compression_level;
        if client_flag & CapabilityFlags::ClientZSTDCompressionAlgorithm as u32 != 0 {
            zstd_compression_level = {
                let result = IntLenEnc::from_bytes(&packet.body[offset..].to_vec(), Some(1));
                offset += result.offset_increment;
                result.result as u8
            }
        } else {
            zstd_compression_level = 0;
        }

        self.accumulation_complete = true;
        assert_eq!(offset, packet.body.len());

        self.client_flag = client_flag;
        self.max_packet_size = max_packet_size;
        self.character_set = character_set;
        self.filler = filler;
        self.username = username;
        self.auth_response_length = auth_response_length;
        self.auth_response = auth_response;
        self.database = database;
        self.client_plugin_name = client_plugin_name;
        self.connection_attrs_length = connection_attrs_length;
        self.connection_attrs = connection_attrs;
        self.zstd_compression_level = zstd_compression_level;

        Phase::AuthInit
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }

    fn get_accumulation_delta(&self) -> Option<AccumulationDelta> {
        Some(AccumulationDelta {
            handshake_response: Some(self.clone()), // Lots of performance being left on the table with this, fix it later
            ..AccumulationDelta::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::{Connection, Phase};
    use crate::mysql::accumulator::handshake_response::HandshakeResponseAccumulator;
    use crate::mysql::accumulator::Accumulator;
    use crate::mysql::packet::Packet;

    #[test]
    fn test_handshake_response() {
        let connection = Connection::default();
        let packet = Packet::from_bytes(
            &[
                0xe2, 0x00, 0x00, 0x01, 0x8d, 0xa6, 0xff, 0x19, 0x00, 0x00, 0x00, 0x01, 0xff, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72, 0x6f, 0x6f, 0x74, 0x00, 0x20,
                0x6d, 0xa0, 0xcf, 0x99, 0x9c, 0xa0, 0x73, 0x04, 0xbd, 0xc1, 0x4d, 0xe8, 0xe4, 0x1b,
                0xa8, 0x35, 0x6e, 0x9d, 0xad, 0xa0, 0x53, 0xec, 0xa4, 0xa8, 0xef, 0x5e, 0x1c, 0x0f,
                0xb3, 0xd4, 0xe4, 0xd5, 0x73, 0x77, 0x69, 0x74, 0x63, 0x68, 0x72, 0x6f, 0x75, 0x74,
                0x65, 0x72, 0x00, 0x63, 0x61, 0x63, 0x68, 0x69, 0x6e, 0x67, 0x5f, 0x73, 0x68, 0x61,
                0x32, 0x5f, 0x70, 0x61, 0x73, 0x73, 0x77, 0x6f, 0x72, 0x64, 0x00, 0x78, 0x04, 0x5f,
                0x70, 0x69, 0x64, 0x06, 0x31, 0x37, 0x39, 0x30, 0x31, 0x38, 0x09, 0x5f, 0x70, 0x6c,
                0x61, 0x74, 0x66, 0x6f, 0x72, 0x6d, 0x06, 0x78, 0x38, 0x36, 0x5f, 0x36, 0x34, 0x03,
                0x5f, 0x6f, 0x73, 0x05, 0x4c, 0x69, 0x6e, 0x75, 0x78, 0x0c, 0x5f, 0x63, 0x6c, 0x69,
                0x65, 0x6e, 0x74, 0x5f, 0x6e, 0x61, 0x6d, 0x65, 0x08, 0x6c, 0x69, 0x62, 0x6d, 0x79,
                0x73, 0x71, 0x6c, 0x07, 0x6f, 0x73, 0x5f, 0x75, 0x73, 0x65, 0x72, 0x08, 0x64, 0x61,
                0x6e, 0x69, 0x79, 0x61, 0x61, 0x6c, 0x0f, 0x5f, 0x63, 0x6c, 0x69, 0x65, 0x6e, 0x74,
                0x5f, 0x76, 0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x06, 0x38, 0x2e, 0x30, 0x2e, 0x34,
                0x30, 0x0c, 0x70, 0x72, 0x6f, 0x67, 0x72, 0x61, 0x6d, 0x5f, 0x6e, 0x61, 0x6d, 0x65,
                0x05, 0x6d, 0x79, 0x73, 0x71, 0x6c,
            ],
            Phase::HandshakeResponse,
        );

        let response =
            HandshakeResponseAccumulator::default().consume(&packet.unwrap(), &connection);
        println!("{:#?}", response);
    }

    #[test]
    fn test_handshake_response_2() {
        let connection = Connection::default();
        let packet = Packet::from_bytes(
            &[
                0xe1, 0x00, 0x00, 0x01, 0x8d, 0xa6, 0xff, 0x19, 0x00, 0x00, 0x00, 0x01, 0xff, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72, 0x6f, 0x6f, 0x74, 0x00, 0x20,
                0xa3, 0x29, 0xcf, 0x0d, 0xb4, 0x09, 0x75, 0x22, 0xd6, 0x20, 0x7a, 0x03, 0xef, 0x30,
                0x53, 0x8f, 0x3e, 0x0d, 0x29, 0xd1, 0xa6, 0x72, 0x7d, 0x5f, 0x35, 0x0e, 0x1b, 0xbf,
                0x90, 0xd1, 0x72, 0xcc, 0x73, 0x77, 0x69, 0x74, 0x63, 0x68, 0x72, 0x6f, 0x75, 0x74,
                0x65, 0x72, 0x00, 0x63, 0x61, 0x63, 0x68, 0x69, 0x6e, 0x67, 0x5f, 0x73, 0x68, 0x61,
                0x32, 0x5f, 0x70, 0x61, 0x73, 0x73, 0x77, 0x6f, 0x72, 0x64, 0x00, 0x77, 0x04, 0x5f,
                0x70, 0x69, 0x64, 0x05, 0x39, 0x39, 0x37, 0x36, 0x37, 0x09, 0x5f, 0x70, 0x6c, 0x61,
                0x74, 0x66, 0x6f, 0x72, 0x6d, 0x06, 0x78, 0x38, 0x36, 0x5f, 0x36, 0x34, 0x03, 0x5f,
                0x6f, 0x73, 0x05, 0x4c, 0x69, 0x6e, 0x75, 0x78, 0x0c, 0x5f, 0x63, 0x6c, 0x69, 0x65,
                0x6e, 0x74, 0x5f, 0x6e, 0x61, 0x6d, 0x65, 0x08, 0x6c, 0x69, 0x62, 0x6d, 0x79, 0x73,
                0x71, 0x6c, 0x07, 0x6f, 0x73, 0x5f, 0x75, 0x73, 0x65, 0x72, 0x08, 0x64, 0x61, 0x6e,
                0x69, 0x79, 0x61, 0x61, 0x6c, 0x0f, 0x5f, 0x63, 0x6c, 0x69, 0x65, 0x6e, 0x74, 0x5f,
                0x76, 0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x06, 0x38, 0x2e, 0x30, 0x2e, 0x34, 0x30,
                0x0c, 0x70, 0x72, 0x6f, 0x67, 0x72, 0x61, 0x6d, 0x5f, 0x6e, 0x61, 0x6d, 0x65, 0x05,
                0x6d, 0x79, 0x73, 0x71, 0x6c,
            ],
            Phase::HandshakeResponse,
        );

        let response =
            HandshakeResponseAccumulator::default().consume(&packet.unwrap(), &connection);
        println!("{:#?}", response);
    }
}
