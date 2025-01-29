use crate::connection::Connection;
use crate::mysql::packet::Packet;

pub mod handshake;
pub mod handshake_response;
pub mod result_set;
pub mod auth_switch_request;
pub mod auth_switch_response;
pub mod auth_complete;

#[repr(u32)]
pub enum CapabilityFlags {
    ClientLongPassword = 0x01,
    ClientConnectWithDB = 0x01 << 4,
    ClientProtocol41 = 0x01 << 10,
    ClientPluginAuth = 0x01 << 19,
    ClientConnectAttrs = 0x01 << 20,
    ClientPluginAuthLenEncClientData = 0x1 << 21,
    ClientZSTDCompressionAlgorithm = 0x1 << 26,
}

/// Consumes a given packet and updates the connection state accordingly.
/// Returns true if no the accumulator expects no further packets (e.g, on the last packet
/// of a ResultSet).
pub trait Accumulator {
    fn consume(&mut self, packet: &Packet, connection: &mut Connection);

    fn accumulation_complete(&self) -> bool;
}
