use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::Accumulator;
use crate::mysql::packet::Packet;
use std::any::Any;

#[derive(Debug, Default)]
pub struct AuthSwitchResponseAccumulator {
    data: Vec<u8>,
    accumulation_complete: bool,
}

impl Accumulator for AuthSwitchResponseAccumulator {
    fn consume(&mut self, packet: &Packet, _connection: &Connection) -> Phase {
        let data = packet.body.to_vec();

        self.accumulation_complete = true;

        self.data = data;
        Phase::AuthComplete
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
