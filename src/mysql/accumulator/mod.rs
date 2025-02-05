use crate::connection::{Connection, Phase};
use crate::mysql::packet::Packet;
use std::any::Any;

pub mod auth_complete;
pub mod auth_init;
pub mod auth_switch_request;
pub mod auth_switch_response;
pub mod command;
pub mod handshake;
pub mod handshake_response;
pub mod result_set;

#[repr(u32)]
pub enum CapabilityFlags {
    #[allow(dead_code)]
    ClientLongPassword = 0x01,
    ClientConnectWithDB = 0x08,
    ClientTransactions = 8192,
    ClientProtocol41 = 0x01 << 9,
    ClientPluginAuth = 0x01 << 19,
    ClientConnectAttrs = 0x01 << 20,
    ClientPluginAuthLenEncClientData = 0x01 << 21,
    ClientSessionTrack = 0x01 << 23,
    ClientDeprecateEof = 0x01 << 24,
    ClientOptionalResultSetMetadata = 0x01 << 25,
    ClientZSTDCompressionAlgorithm = 0x01 << 26,
}

#[derive(Default)]
pub struct AccumulationDelta {
    pub handshake: Option<handshake::HandshakeAccumulator>,
    pub handshake_response: Option<handshake_response::HandshakeResponseAccumulator>,
    pub last_command: Option<crate::mysql::command::Command>,
    pub response: Option<result_set::ResponseAccumulator>,
}

/// Consumes a given packet and returns the phase to transition the connection into.
/// Returns true if no the accumulator expects no further packets (e.g, on the last packet
/// of a ResultSet).
pub trait Accumulator: Any {
    fn consume(&mut self, packet: &Packet, connection: &Connection) -> Phase;

    fn accumulation_complete(&self) -> bool;

    fn as_any(&self) -> &dyn Any;

    fn get_accumulation_delta(&self) -> Option<AccumulationDelta> {
        None
    }
}
