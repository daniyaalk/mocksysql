use crate::connection::State::Initiated;
use crate::mysql::command::Command;

#[allow(dead_code)]
#[derive(Default)]
pub struct Connection {
    pub state: State,
    pub partial_bytes: Option<Vec<u8>>,
    pub partial_result_set: Option<crate::mysql::result_set::ResultSet>,
    pub last_command: Option<Box<Command>>,
}

impl Connection {
    pub fn get_state(&self) -> &State {
        &self.state
    }

    pub fn mark_auth_done(&mut self) {
        self.state = State::AuthDone
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

pub enum State {
    Initiated,
    AuthDone,
    #[allow(dead_code)]
    PendingResponse,
}

impl Default for State {
    fn default() -> State {
        Initiated
    }
}

#[derive(Debug, PartialEq)]
pub enum Direction {
    C2S,
    S2C,
}
