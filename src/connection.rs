use crate::mysql::command::Command;
use std::cell::Cell;
use crate::mysql::protocol::handshake_response::HandshakeResponse;

#[allow(dead_code)]
#[derive(Default)]
pub struct Connection {
    pub phase: Phase,
    pub partial_bytes: Option<Vec<u8>>,
    pub partial_result_set: Option<crate::mysql::protocol::result_set::ResultSet>,
    pub last_command: Option<Command>,

    pub handshake: Option<crate::mysql::protocol::handshake::Handshake>,
    pub handshake_response: Option<HandshakeResponse>,
}

impl Connection {
    pub fn get_state(&self) -> &Phase {
        &self.phase
    }

}

impl Connection {
    pub fn unset_partial_data(&mut self) {
        self.partial_bytes = None;
    }

    pub fn set_partial_data(&mut self, bytes: Vec<u8>) {
        let mut temp: Vec<u8> = Vec::new();
        temp.extend(bytes);
        self.partial_bytes = Some(temp);
    }
}

#[derive(Clone, Debug)]
pub enum Phase {
    Handshake,
    HandshakeResponse,
    AuthInit,
    AuthSwitchRequest,
    AuthSwitchResponse,
    AuthFailed,
    AuthComplete,
    Command,
    #[allow(dead_code)]
    PendingResponse,
}

impl Default for Phase {
    fn default() -> Phase {
        Phase::Handshake
    }
}

#[derive(Debug, PartialEq)]
pub enum Direction {
    C2S,
    S2C,
}
