use crate::connection::Connection;
use crate::mysql::accumulator::Accumulator;
use crate::mysql::packet::Packet;
use crate::mysql::protocol::CapabilityFlags;
use crate::mysql::types::{Converter, IntFixedLen, StringFixedLen, StringNullEnc};
use std::cmp::max;

const RESERVED_STRING: &str = "\0\0\0\0\0\0\0\0\0\0";
const PLUGIN_DATA_MAX_LENGTH: u8 = 21;

/// https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase_packets_protocol_handshake_v10.html
#[derive(Debug, Default)]
pub struct Handshake {
    accumulation_complete: bool,
    protocol_version: u8,
    server_version: String,
    thread_id: u64,
    auth_plugin_data_part_1: String,
    filler: u8,
    capability_flags_1: u16,
    character_set: u8,
    status_flags: u16,
    capability_flags_2: u16,
    auth_plugin_data_len: u8,
    auth_plugin_data_part_2: String,
    auth_plugin_name: Option<String>,

    capability_flags: u32,
}

impl Accumulator for Handshake {
    fn consume(&mut self, packet: Packet, connection: &mut Connection) -> bool {
        let mut offset: usize = 0;

        self.protocol_version = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
            offset += result.offset_increment;
            assert_eq!(0x0a, result.result, "Unsupported protocol version");
            result.result as u8
        };

        self.server_version = {
            let result = StringNullEnc::from_bytes(&packet.body[offset..].to_vec(), None);
            offset += result.offset_increment;
            result.result
        };

        self.thread_id = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(4));
            offset += result.offset_increment;
            result.result
        };

        self.auth_plugin_data_part_1 = {
            let result = StringFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(8));
            offset += result.offset_increment;
            result.result
        };

        self.filler = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
            offset += result.offset_increment;
            assert_eq!(0x00, result.result, "Unexpected filler!");
            result.result as u8
        };

        self.capability_flags_1 = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(2));
            offset += result.offset_increment;
            result.result as u16
        };

        self.character_set = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
            offset += result.offset_increment;
            result.result as u8
        };

        self.status_flags = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(2));
            offset += result.offset_increment;
            result.result as u16
        };

        self.capability_flags_2 = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(2));
            offset += result.offset_increment;
            result.result as u16
        };

        self.capability_flags =
            ((self.capability_flags_2 as u32) << 16) + self.capability_flags_1 as u32;

        if self.capability_flags & CapabilityFlags::ClientPluginAuth as u32 != 0 {
            self.auth_plugin_data_len = {
                let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
                offset += result.offset_increment;
                result.result as u8
            }
        } else {
            assert_eq!(
                0x00,
                IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1)).result
            );
            offset += 1;
        }

        let _reserved_string: String = {
            let result = StringFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(10));
            offset += result.offset_increment;
            assert!(
                RESERVED_STRING.eq(&result.result),
                "Unexpected reserved string!"
            );
            result.result
        };

        self.auth_plugin_data_part_2 = {
            let length = max(PLUGIN_DATA_MAX_LENGTH, self.auth_plugin_data_len) - 8;
            let result =
                StringFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(length as usize));
            offset += result.offset_increment;
            result.result
        };

        if self.capability_flags & CapabilityFlags::ClientPluginAuth as u32 != 0 {
            self.auth_plugin_name = {
                let result = StringNullEnc::from_bytes(&packet.body[offset..].to_vec(), None);
                offset += result.offset_increment;
                Some(result.result)
            }
        }

        true
    }
}

enum StatusFlags {
    ClientLongPassword = 0x01,
    ClientFoundRows = 0x02,
}


#[cfg(test)]
mod tests {
    use crate::connection::{Connection, Phase};
    use crate::mysql::accumulator::Accumulator;
    use crate::mysql::accumulator::handshake::Handshake;
    use crate::mysql::packet::Packet;

    #[test]
    fn test_handshake() {
        let packet = Packet::from_bytes(&[ 0x4a,0x00,0x00,0x00,0x0a,0x38,0x2e,0x30,0x2e,0x33,0x32,0x00,0x0a,0x00,0x00,0x00,0x15,0x51,0x79,0x32,0x2c,0x6e,0x09,0x77,0x00,0xff,0xff,0xff,0x02,0x00,0xff,0xdf,0x15,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x43,0x28,0x36,0x51,0x2c,0x51,0x74,0x7c,0x62,0x08,0x60,0x22,0x00,0x63,0x61,0x63,0x68,0x69,0x6e,0x67,0x5f,0x73,0x68,0x61,0x32,0x5f,0x70,0x61,0x73,0x73,0x77,0x6f,0x72,0x64,0x00,], Phase::AuthRequest).unwrap();
        let mut connection = Connection::default();

        let mut handshake = Handshake::default();
        handshake.consume(packet, &mut connection);

        println!("{:?}", handshake);
    }
}