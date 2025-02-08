use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::CapabilityFlags;
use crate::mysql::accumulator::{AccumulationDelta, Accumulator};
use crate::mysql::packet::Packet;
use crate::mysql::types::{Converter, IntFixedLen, StringFixedLen, StringNullEnc};
use std::cmp::max;

const RESERVED_STRING: &str = "\0\0\0\0\0\0\0\0\0\0";
const PLUGIN_DATA_MAX_LENGTH: u8 = 21;

/// https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase_packets_protocol_handshake_v10.html
#[derive(Debug, Default, Clone)]
pub struct HandshakeAccumulator {
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

impl Accumulator for HandshakeAccumulator {
    fn consume(&mut self, packet: &Packet, _connection: &Connection) -> Phase {
        let mut offset: usize = 0;

        let protocol_version = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
            offset += result.offset_increment;
            assert_eq!(0x0a, result.result, "Unsupported protocol version");
            result.result as u8
        };

        let server_version = {
            let result = StringNullEnc::from_bytes(&packet.body[offset..].to_vec(), None);
            offset += result.offset_increment;
            result.result
        };

        let thread_id = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(4));
            offset += result.offset_increment;
            result.result
        };

        let auth_plugin_data_part_1 = {
            let result = StringFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(8));
            offset += result.offset_increment;
            result.result
        };

        let filler = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
            offset += result.offset_increment;
            assert_eq!(0x00, result.result, "Unexpected filler!");
            result.result as u8
        };

        let capability_flags_1 = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(2));
            offset += result.offset_increment;
            result.result as u16
        };

        let character_set = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
            offset += result.offset_increment;
            result.result as u8
        };

        let status_flags = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(2));
            offset += result.offset_increment;
            result.result as u16
        };

        let capability_flags_2 = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(2));
            offset += result.offset_increment;
            result.result as u16
        };

        let capability_flags = ((capability_flags_2 as u32) << 16) + capability_flags_1 as u32;
        let auth_plugin_data_len;

        if capability_flags & CapabilityFlags::ClientPluginAuth as u32 != 0 {
            auth_plugin_data_len = {
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
            auth_plugin_data_len = 0;
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

        let auth_plugin_data_part_2 = {
            let length = max(PLUGIN_DATA_MAX_LENGTH, auth_plugin_data_len) - 8;
            let result =
                StringFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(length as usize));
            offset += result.offset_increment;
            result.result
        };

        let auth_plugin_name = if capability_flags & CapabilityFlags::ClientPluginAuth as u32 != 0 {
            let result = StringNullEnc::from_bytes(&packet.body[offset..].to_vec(), None);
            offset += result.offset_increment;
            Some(result.result)
        } else {
            None
        };

        assert_eq!(offset, packet.body.len());
        self.accumulation_complete = true;
        self.protocol_version = protocol_version;
        self.server_version = server_version;
        self.thread_id = thread_id;
        self.auth_plugin_data_part_1 = auth_plugin_data_part_1;
        self.filler = filler;
        self.capability_flags_1 = capability_flags_1;
        self.character_set = character_set;
        self.status_flags = status_flags;
        self.capability_flags_2 = capability_flags_2;
        self.auth_plugin_data_len = auth_plugin_data_len;
        self.auth_plugin_data_part_2 = auth_plugin_data_part_2;
        self.auth_plugin_name = auth_plugin_name;
        self.capability_flags = capability_flags;

        Phase::HandshakeResponse
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }

    fn get_accumulation_delta(&self) -> Option<AccumulationDelta> {
        Some(AccumulationDelta {
            handshake: Some(self.clone()),
            ..AccumulationDelta::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::{Connection, Phase};
    use crate::mysql::accumulator::handshake::HandshakeAccumulator;
    use crate::mysql::accumulator::Accumulator;
    use crate::mysql::packet::Packet;

    #[test]
    fn test_handshake() {
        let packet = Packet::from_bytes(
            &[
                0x4a, 0x00, 0x00, 0x00, 0x0a, 0x38, 0x2e, 0x30, 0x2e, 0x33, 0x32, 0x00, 0x0a, 0x00,
                0x00, 0x00, 0x15, 0x51, 0x79, 0x32, 0x2c, 0x6e, 0x09, 0x77, 0x00, 0xff, 0xff, 0xff,
                0x02, 0x00, 0xff, 0xdf, 0x15, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x43, 0x28, 0x36, 0x51, 0x2c, 0x51, 0x74, 0x7c, 0x62, 0x08, 0x60, 0x22, 0x00,
                0x63, 0x61, 0x63, 0x68, 0x69, 0x6e, 0x67, 0x5f, 0x73, 0x68, 0x61, 0x32, 0x5f, 0x70,
                0x61, 0x73, 0x73, 0x77, 0x6f, 0x72, 0x64, 0x00,
            ],
            Phase::AuthInit,
        )
        .unwrap();
        let connection = Connection::default();

        let mut handshake = HandshakeAccumulator::default();
        handshake.consume(&packet, &connection);

        println!("{:?}", handshake);
        // TODO: Add assertions
    }
}
