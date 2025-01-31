use crate::connection::{Connection, Phase};
use crate::mysql::packet::Packet;

pub mod auth_complete;
pub mod auth_init;
pub mod auth_switch_request;
pub mod auth_switch_response;
pub mod handshake;
pub mod handshake_response;
pub mod result_set;

#[repr(u32)]
pub enum CapabilityFlags {
    ClientLongPassword = 0x01,
    ClientConnectWithDB = 0x01 << 4,
    ClientProtocol41 = 0x01 << 10,
    ClientPluginAuth = 0x01 << 19,
    ClientConnectAttrs = 0x01 << 20,
    ClientPluginAuthLenEncClientData = 0x1 << 21,
    ClientDeprecateEof = 0x1 << 24,
    ClientOptionalResultSetMetadata = 0x1 << 25,
    ClientZSTDCompressionAlgorithm = 0x1 << 26,
}

/// Consumes a given packet and returns the phase to transition the connection into.
/// Returns true if no the accumulator expects no further packets (e.g, on the last packet
/// of a ResultSet).
pub trait Accumulator {
    fn consume(&mut self, packet: &Packet, connection: &Connection) -> Phase;

    fn accumulation_complete(&self) -> bool;
}
