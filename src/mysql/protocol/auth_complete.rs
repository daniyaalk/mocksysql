use crate::connection::{Connection, Phase};
use crate::mysql::packet::{Packet, PacketType};
use crate::mysql::protocol::Accumulator;

#[derive(Default)]
pub struct AuthComplete {
    accumulation_complete: bool,
}

impl Accumulator for AuthComplete {
    fn consume(&mut self, packet: &Packet, connection: &mut Connection) {
        if PacketType::Ok == packet.p_type {
            connection.phase = Phase::Command;
        } else if PacketType::Error == packet.p_type {
            connection.phase = Phase::AuthFailed;
        }
        self.accumulation_complete = true;
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }
}
