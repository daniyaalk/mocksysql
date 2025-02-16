use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::Accumulator;
use crate::mysql::packet::{Packet, PacketType};

#[derive(Default)]
pub struct AuthCompleteAccumulator {
    accumulation_complete: bool,
}

impl Accumulator for AuthCompleteAccumulator {
    fn consume(&mut self, packet: &Packet, _connection: &Connection) -> Phase {
        let phase;
        if PacketType::Ok == packet.p_type {
            phase = Phase::Command;
        } else if PacketType::Error == packet.p_type {
            phase = Phase::AuthFailed;
        } else {
            panic!("Unexpected packet type: {:?}", packet.p_type)
        }
        self.accumulation_complete = true;
        phase
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }
}
