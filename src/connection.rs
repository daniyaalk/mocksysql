use std::cell::{Cell, RefCell};
use crate::mysql::command::Command;
use crate::mysql::protocol::handshake_response::HandshakeResponse;
use std::rc::Rc;
use crate::mysql::protocol::result_set::ResultSet;

#[allow(dead_code)]
#[derive(Default)]
pub struct Connection {
    pub phase: Phase,
    pub partial_bytes: Option<Vec<u8>>,
    pub last_command: Option<Command>,

    pub handshake: Option<crate::mysql::protocol::handshake::Handshake>,
    pub handshake_response: Option<HandshakeResponse>,
    pub result_set: RefCell<ResultSet>,
}

impl Connection {
    pub fn get_state(&self) -> &Phase {
        &self.phase
    }

    pub fn get_last_command(&self) -> Option<&Command> {
        self.last_command.as_ref()
    }

    pub fn get_handshake_response(&self) -> Option<&HandshakeResponse> {
        self.handshake_response.as_ref()
    }
    
    pub fn get_result_set(&self) -> &RefCell<ResultSet> {
        &self.result_set
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
