use std::cell::Cell;
use crate::connection::Connection;
use crate::mysql::accumulator::Accumulator;
use crate::mysql::packet::Packet;
use crate::mysql::types::{Converter, IntFixedLen, StringNullEnc};
use std::thread::ThreadId;

/// https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase_packets_protocol_handshake_v10.html
#[derive(Default)]
pub struct Handshake {
    accumulation_complete: bool,
    protocol_version: u8,
    server_version: String,
    thread_id: Option<ThreadId>,
    auth_plugin_data_part_1: [char; 8],
    filler: u8,
    capability_flags_1: [u8; 2],
    character_set: u8,
    status_flags: [u8; 8],
    capability_flags_2: [u8; 2],
}

impl Accumulator for Handshake {
    fn consume(&mut self, packet: Packet, connection: &mut Connection) -> bool {
        let mut offset: usize = 0;

        self.protocol_version = {
            let result = IntFixedLen::from_bytes(&packet.body, Some(8));
            offset += result.offset_increment;
            result.result as u8
        };

        if self.protocol_version != 0x0a {
            panic!("Unsupported protocol version");
        }

        self.server_version = {
            let result = StringNullEnc::from_bytes(&packet.body, None);
            offset += result.offset_increment;
            result.result
        };

        true
    }
}

enum StatusFlags {
    ClientLongPassword = 0x01,
    ClientFoundRows = 0x02,
}

