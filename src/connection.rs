use crate::mysql::accumulator::handshake_response::HandshakeResponseAccumulator;
use crate::mysql::accumulator::result_set::ResponseAccumulator;
use crate::mysql::command::Command;

#[allow(dead_code)]
#[derive(Default)]
pub struct Connection {
    pub phase: Phase,
    pub partial_bytes: Option<Vec<u8>>,
    pub last_command: Option<Command>,

    pub handshake: Option<crate::mysql::accumulator::handshake::HandshakeAccumulator>,
    pub handshake_response: Option<HandshakeResponseAccumulator>,
    query_response: ResponseAccumulator,
}

impl Connection {
    pub fn get_state(&self) -> &Phase {
        &self.phase
    }

    pub fn get_last_command(&self) -> Option<&Command> {
        self.last_command.as_ref()
    }

    pub fn get_handshake_response(&self) -> Option<&HandshakeResponseAccumulator> {
        self.handshake_response.as_ref()
    }

    pub fn get_response_accumulator(&self) -> ResponseAccumulator {
        self.query_response.clone()
    }

    pub fn set_response_accumulator(&mut self, accumulator: ResponseAccumulator) {
        self.query_response = accumulator;
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

#[derive(Clone, Debug, Default, PartialEq)]
#[repr(u8)]
pub enum Phase {
    #[default]
    Handshake,
    HandshakeResponse,
    AuthInit,
    AuthSwitchResponse,
    AuthFailed,
    AuthComplete,
    Command,
    #[allow(dead_code)]
    PendingResponse,
}

#[derive(Debug, PartialEq)]
pub enum Direction {
    C2S,
    S2C,
}
