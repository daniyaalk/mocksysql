use crate::connection::Connection;
use crate::mysql::packet::Packet;

pub mod result_set;
pub mod handshake;
mod handshake_response;

#[repr(u32)]
pub enum CapabilityFlags {
    ClientLongPassword = 0x01,
    ClientPluginAuth = 0x1 << 19,
}

/// Consumes a given packet and updates the connection state accordingly.
/// Returns true if no the accumulator expects no further packets (e.g, on the last packet
/// of a ResultSet).
pub trait Accumulator {
    fn consume(&mut self, packet: &Packet, connection: &mut Connection) -> Self;
}