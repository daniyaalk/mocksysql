use crate::materialization::StateDiffLog;
use crate::mysql::accumulator::handshake::HandshakeAccumulator;
use crate::mysql::accumulator::handshake_response::HandshakeResponseAccumulator;
use crate::mysql::accumulator::result_set::ResponseAccumulator;
use crate::mysql::command::Command;
use dashmap::DashMap;
#[cfg(feature = "tls")]
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::net::TcpStream;
use std::sync::Arc;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Connection {
    pub phase: Phase,
    pub partial_bytes: Option<Vec<u8>>,
    pub last_command: Option<Command>,

    pub handshake: Option<HandshakeAccumulator>,
    pub handshake_response: Option<HandshakeResponseAccumulator>,

    pub client_connection: SwitchableConnection,
    pub server_connection: SwitchableConnection,

    query_response: ResponseAccumulator,
    pub diff: StateDiffLog,
}

impl Connection {
    pub fn new(
        server: SwitchableConnection,
        client: SwitchableConnection,
        state_difference_map: StateDiffLog,
    ) -> Connection {
        Connection {
            client_connection: client,
            server_connection: server,
            phase: Phase::Handshake,
            partial_bytes: None,
            last_command: None,
            handshake: None,
            handshake_response: None,
            query_response: ResponseAccumulator::default(),
            diff: state_difference_map,
        }
    }

    #[cfg(test)]
    pub fn default() -> Connection {
        let map = Arc::new(DashMap::<BTreeSet<(String, String)>, String>::new());

        Connection::new(
            SwitchableConnection::None,
            SwitchableConnection::None,
            StateDiffLog::default(),
        )
    }

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

#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
#[derive(Hash)]
pub enum Phase {
    Handshake,
    HandshakeResponse,
    TlsExchange,
    AuthInit,
    AuthSwitchResponse,
    AuthFailed,
    AuthComplete,
    Command,
    #[allow(dead_code)]
    PendingResponse,
}

#[derive(Debug)]
pub enum SwitchableConnection {
    Plain(RefCell<TcpStream>),
    #[cfg(feature = "tls")]
    ClientTls(RefCell<StreamOwned<ServerConnection, TcpStream>>),
    #[cfg(feature = "tls")]
    ServerTls(RefCell<StreamOwned<ClientConnection, TcpStream>>),
    #[cfg(test)]
    None,
}
impl SwitchableConnection {
    pub fn take(self) -> TcpStream {
        match self {
            SwitchableConnection::Plain(stream) => stream.into_inner(),
            _ => panic!("SwitchableConnection::take() called on a non-PlainConnection"),
        }
    }
}
