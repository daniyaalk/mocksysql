use crate::connection::Phase::Auth;
use crate::mysql::command::Command;

#[allow(dead_code)]
#[derive(Default)]
pub struct Connection {
    pub phase: Phase,
    pub partial_bytes: Option<Vec<u8>>,
    pub partial_result_set: Option<crate::mysql::result_set::ResultSet>,
    pub last_command: Option<Command>,
}

impl Connection {
    pub fn get_state(&self) -> &Phase {
        &self.phase
    }

    pub fn mark_auth_done(&mut self) {
        self.phase = Phase::Command
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

#[derive(Clone)]
pub enum Phase {
    Auth,
    Command,
    #[allow(dead_code)]
    PendingResponse,
}

impl Default for Phase {
    fn default() -> Phase {
        Auth
    }
}

#[derive(Debug, PartialEq)]
pub enum Direction {
    C2S,
    S2C,
}
