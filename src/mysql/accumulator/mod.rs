use crate::connection::Connection;
use crate::mysql::packet::Packet;

pub mod handshake;

/// Consumes a given packet and updates the connection state accordingly.
/// Returns true if no the accumulator expects no further packets (e.g, on the last packet
/// of a ResultSet).
pub trait Accumulator {
    fn consume(&mut self, packet: Packet, connection: &mut Connection) -> bool;
}
