use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::auth_switch_request::AuthSwitchRequestAccumulator;
use crate::mysql::accumulator::{auth_complete, Accumulator};
use crate::mysql::packet::Packet;
use auth_complete::AuthCompleteAccumulator;

#[derive(Debug, Clone, Default)]
pub struct AuthInitAccumulator {
    accumulation_complete: bool,
}

impl Accumulator for AuthInitAccumulator {
    fn consume(&mut self, packet: &Packet, connection: &Connection) -> Phase {
        // https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase_packets_protocol_auth_switch_request.html
        if packet.body[0] == 0xfe {
            self.accumulation_complete = true;
            return AuthSwitchRequestAccumulator::default().consume(packet, connection);
        } else if packet.body[0] == 0x00 {
            // AuthSwitch is not required if the credentials sent in HandshakeResponse were sufficient.
            return AuthCompleteAccumulator::default().consume(packet, connection);
        }

        Phase::AuthInit
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }
}
