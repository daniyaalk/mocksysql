use crate::connection::{Connection, Phase};
use crate::mysql::packet::Packet;
use crate::mysql::protocol::Accumulator;
use crate::mysql::types::{Converter, IntFixedLen, StringEOFEnc, StringNullEnc};
use std::fs::read_to_string;

#[derive(Debug, Default)]
pub struct AuthSwitchRequest {
    status_tag: u8,
    plugin_name: String,
    plugin_provided_data: String,
    accumulation_complete: bool,
}

impl Accumulator for AuthSwitchRequest {
    fn consume(&mut self, packet: &Packet, connection: &mut Connection) {
        let mut offset: usize = 0;

        let status_tag = {
            let result = IntFixedLen::from_bytes(&packet.body[offset..].to_vec(), Some(1));
            offset += result.offset_increment;
            assert_eq!(0xfe, result.result);
            result.result as u8
        };

        let plugin_name = {
            let result = StringNullEnc::from_bytes(&packet.body[offset..].to_vec(), None);
            offset += result.offset_increment;
            result.result
        };

        let plugin_provided_data = {
            let result = StringEOFEnc::from_bytes(&packet.body[offset..].to_vec(), None);
            offset += result.offset_increment;
            result.result
        };

        connection.phase = Phase::AuthSwitchResponse;
        self.accumulation_complete = true;
        self.status_tag = status_tag;
        self.plugin_name = plugin_name;
        self.plugin_provided_data = plugin_provided_data;
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }
}
