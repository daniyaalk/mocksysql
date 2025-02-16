use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::Accumulator;
use crate::mysql::packet::Packet;
use std::io::{Read, Write};

#[derive(Debug, Default)]
pub struct AuthSwitchResponseAccumulator {
    data: Vec<u8>,
    accumulation_complete: bool,
}

impl Accumulator for AuthSwitchResponseAccumulator {
    fn consume<RWS: Read + Write + Sized>(
        &mut self,
        packet: &Packet,
        _connection: &Connection<RWS>,
    ) -> Phase {
        let data = packet.body.to_vec();

        self.accumulation_complete = true;

        self.data = data;
        Phase::AuthComplete
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }
}
