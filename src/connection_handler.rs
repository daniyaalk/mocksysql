use crate::connection::{ClientConnectionType, Phase, ServerConnectionType};
use crate::mysql::command::{Command, MySqlCommand};
use crate::mysql::packet::{OkData, Packet, PacketType};
use crate::tls::{handle_client_tls, handle_server_tls};
use crate::{connection::Connection, state_handler};
use rustls::StreamOwned;
use std::collections::HashSet;
use std::sync::atomic::AtomicU8;
use std::sync::LazyLock;
use std::{
    env,
    io::{Error, Read, Write},
    net::TcpStream,
    thread,
};

static GLOBAL_COUNTER: AtomicU8 = AtomicU8::new(100);
static SERVER_TRANSITION_PHASES: LazyLock<HashSet<Phase>> = LazyLock::new(|| {
    HashSet::from([
        Phase::HandshakeResponse,
        Phase::Command,
        Phase::AuthComplete,
        Phase::AuthSwitchResponse,
    ])
});

static CLIENT_TRANSITION_PHASES: LazyLock<HashSet<Phase>> =
    LazyLock::new(|| HashSet::from([Phase::AuthInit, Phase::PendingResponse, Phase::AuthComplete]));

fn exchange(mut connection: Connection) -> Result<(), Error> {
    let mut buf: [u8; 4096];

    loop {
        // Server Loop
        loop {
            buf = [0; 4096]; // Due to a poor design choice in state_handler.rs

            println!("Listening from server");
            let (server_connection, read_bytes) =
                read_server_bytes(connection.server_connection, &mut buf);
            connection.server_connection = server_connection;
            let read_bytes = read_bytes?;

            println!("From server: {:?}", &buf[0..read_bytes].to_vec());

            if read_bytes == 0 {
                panic!();
            }

            let packets = state_handler::process_incoming_frame(&buf, &mut connection);

            let (client_connection, _) =
                write_client_bytes(connection.client_connection, &buf[..read_bytes]);
            connection.client_connection = client_connection;

            if SERVER_TRANSITION_PHASES.contains(&connection.phase) {
                println!("Transitioning to client");
                break;
            }
        }

        // client loop

        loop {
            buf = [0; 4096]; // Due to a poor design choice in state_handler.rs
            if connection.phase == Phase::TlsExchange {
                let server_tls = handle_server_tls(&mut buf);
                let client_tls = handle_client_tls(&mut buf);

                if let (
                    ServerConnectionType::Plain(server_conn),
                    ClientConnectionType::Plain(client_conn),
                ) = (&connection.server_connection, &connection.client_connection)
                {
                    let server_tls_conn = StreamOwned::new(server_tls, server_conn.try_clone()?);
                    let client_tls_conn = StreamOwned::new(client_tls, client_conn.try_clone()?);

                    connection = connection.switch_connections(
                        ServerConnectionType::Tls(server_tls_conn),
                        ClientConnectionType::Tls(client_tls_conn),
                    );
                }

                connection.phase = Phase::HandshakeResponse;
                println!("TLS Set");
            }

            println!("Listening from client: {:?}", &connection.phase);

            let (client_connection, read_bytes) =
                read_client_bytes(connection.client_connection, &mut buf);
            connection.client_connection = client_connection;
            let read_bytes = read_bytes?;

            println!("From client: {:?}", &buf[0..read_bytes].to_vec());

            if read_bytes == 0 {
                panic!();
            }

            let packets = state_handler::process_incoming_frame(&buf, &mut connection);

            let (server_connection, _) =
                write_server_bytes(connection.server_connection, &buf[..read_bytes]);
            connection.server_connection = server_connection;

            if CLIENT_TRANSITION_PHASES.contains(&connection.phase) {
                println!("Transitioning to server");
                break;
            }
        }
    }

    Ok(())
}

pub fn read_server_bytes(
    mut conn: ServerConnectionType,
    buf: &mut [u8],
) -> (ServerConnectionType, Result<usize, Error>) {
    let result = match &mut conn {
        ServerConnectionType::Plain(stream) => stream.read(buf),
        ServerConnectionType::Tls(stream) => stream.read(buf),
        #[cfg(test)]
        ServerConnectionType::None => unreachable!(),
    };
    (conn, result)
}

pub fn write_server_bytes(
    conn: ServerConnectionType,
    buf: &[u8],
) -> (ServerConnectionType, std::io::Result<usize>) {
    match conn {
        ServerConnectionType::Plain(mut stream) => {
            let p = stream.write(buf);
            (ServerConnectionType::Plain(stream), p)
        }
        ServerConnectionType::Tls(mut stream) => {
            let p = stream.write(buf);
            (ServerConnectionType::Tls(stream), p)
        }
        #[cfg(test)]
        ServerConnectionType::None => unreachable!(),
    }
}

pub fn read_client_bytes(
    mut conn: ClientConnectionType,
    buf: &mut [u8],
) -> (ClientConnectionType, Result<usize, Error>) {
    let result = match &mut conn {
        ClientConnectionType::Plain(stream) => stream.read(buf),
        ClientConnectionType::Tls(stream) => stream.read(buf),
        #[cfg(test)]
        ClientConnectionType::None => unreachable!(),
    };
    (conn, result)
}

pub fn write_client_bytes(
    conn: ClientConnectionType,
    buf: &[u8],
) -> (ClientConnectionType, std::io::Result<usize>) {
    match conn {
        ClientConnectionType::Plain(mut stream) => {
            let p = stream.write(buf);
            (ClientConnectionType::Plain(stream), p)
        }
        ClientConnectionType::Tls(mut stream) => {
            let p = stream.write(buf);
            (ClientConnectionType::Tls(stream), p)
        }
        #[cfg(test)]
        ClientConnectionType::None => unreachable!(),
    }
}

fn get_write_response(
    last_command: &Option<Command>,
    sequence: &u8,
    client_flag: u32,
) -> Option<Vec<u8>> {
    let count = GLOBAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ok_data = OkData {
        header: 0x00,
        affected_rows: 1,
        last_insert_id: count as u64,
        status_flags: None,
        warnings: None,
        info: None,
        session_state_info: None,
    };

    Some(ok_data.to_packet(sequence + 1, client_flag).to_bytes())
}

fn is_write_query(last_command: &Option<Command>, packet: &Packet) -> bool {
    if last_command.is_none() {
        return false;
    }

    let last_command = last_command.as_ref().unwrap();
    let last_command_arg = &last_command.arg.to_lowercase();

    packet.p_type.eq(&PacketType::Command)
        && last_command.com_code.eq(&MySqlCommand::ComQuery)
        && (last_command_arg.starts_with("insert")
            || last_command_arg.starts_with("update")
            || last_command_arg.starts_with("delete"))
}

pub fn initiate(client: TcpStream) {
    let target_address: &str = "127.0.0.1:3307";

    let server = TcpStream::connect(target_address).expect("Fault");

    let connection = Connection::new(
        ServerConnectionType::Plain(server),
        ClientConnectionType::Plain(client),
    );

    let worker = thread::spawn(move || exchange(connection));

    worker.join().ok();
}

fn intercept_enabled() -> bool {
    env::var("INTERCEPT_INSERT").is_ok() && env::var("INTERCEPT_INSERT").unwrap() == "true"
}

// if intercept_enabled() {
// let last_command = connection.last_command.clone();
// let client_flag = connection
// .handshake_response
// .as_ref()
// .map(|hr| hr.client_flag);
//
// if packets.len() == 1 && is_write_query(&last_command, packets.first().unwrap()) {
// if let Some(response) = get_write_response(
// &last_command,
// &packets.first().unwrap().header.seq,
// client_flag.unwrap(),
// ) {
// println!("{:?}", response);
// connection.phase = Phase::Command;
// from.write_all(&response)?;
// continue;
// }
// }
// }
