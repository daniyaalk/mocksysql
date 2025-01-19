use crate::mysql::command::Command;

#[allow(dead_code)]
pub struct Connection {
    pub state: State,
    pub partial_data: Option<Vec<u8>>,
    pub last_command: Option<Box<Command>>,
}

impl Connection {
    pub fn new() -> Connection {
        Connection {
            state: State::Initiated,
            partial_data: None,
            last_command: None,
        }
    }

    pub fn get_state(&self) -> &State {
        &self.state
    }

    pub fn mark_auth_done(&mut self) {
        self.state = State::AuthDone
    }
}

#[allow(dead_code)]
impl Connection {
    pub fn unset_partial_data(&mut self) {
        self.partial_data = None;
    }

    pub fn set_partial_data(&mut self, bytes: Vec<u8>) {
        let mut temp: Vec<u8> = Vec::new();
        temp.extend(bytes);
        self.partial_data = Some(temp);
    }
}

pub enum State {
    Initiated,
    AuthDone,
    #[allow(dead_code)]
    PendingResponse,
}

#[derive(Debug, PartialEq)]
pub enum Direction {
    C2S,
    S2C,
}
