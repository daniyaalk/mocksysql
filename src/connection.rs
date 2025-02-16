use crate::mysql::accumulator::handshake::HandshakeAccumulator;
use crate::mysql::accumulator::handshake_response::HandshakeResponseAccumulator;
use crate::mysql::accumulator::result_set::ResponseAccumulator;
use crate::mysql::command::Command;
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use std::io::{Read, Write};
use std::net::TcpStream;

trait RWS: Read + Write + Sized {}
pub enum ClientConnectionType {
    Plain(TcpStream),
    Tls(StreamOwned<ServerConnection, TcpStream>),
}

pub enum ServerConnectionType {
    Plain(TcpStream),
    Tls(StreamOwned<ClientConnection, TcpStream>),
}

#[allow(dead_code)]
// #[derive(Debug)]
pub struct Connection {
    pub phase: Phase,
    pub partial_bytes: Option<Vec<u8>>,
    pub last_command: Option<Command>,

    pub handshake: Option<HandshakeAccumulator>,
    pub handshake_response: Option<HandshakeResponseAccumulator>,

    pub client_connection: Box<dyn RWS>,
    pub server_connection: RWS,

    query_response: ResponseAccumulator,
}

impl Connection {
    pub fn switch_connections(
        self,
        server: ServerConnectionType,
        client: ClientConnectionType,
    ) -> Connection {
        Connection {
            client_connection: client,
            server_connection: server,
            ..self
        }
    }

    pub fn new(server: ServerConnectionType, client: ClientConnectionType) -> Connection {
        Connection {
            client_connection: client,
            server_connection: server,
            phase: Phase::Handshake,
            partial_bytes: None,
            last_command: None,
            handshake: None,
            handshake_response: None,
            query_response: ResponseAccumulator::default(),
        }
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

#[derive(Debug, PartialEq)]
pub enum Direction {
    C2S,
    S2C,
}
