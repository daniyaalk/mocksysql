use crate::connection::{Connection, Phase};
use crate::mysql::packet::Packet;
use crate::mysql::protocol::Accumulator;
use crate::mysql::types::{Converter, IntFixedLen, StringEOFEnc, StringNullEnc};
use std::fs::read_to_string;

#[derive(Debug, Default)]
pub struct AuthSwitchResponse {
    data: Vec<u8>,
    accumulation_complete: bool,
}

impl Accumulator for AuthSwitchResponse {
    fn consume(&mut self, packet: &Packet, connection: &Connection) -> Phase {
        let data = packet.body.to_vec();

        self.accumulation_complete = true;

        self.data = data;
        Phase::AuthComplete
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }
}
