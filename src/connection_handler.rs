use crate::connection::{Phase, SwitchableConnection};
use crate::materialization::StateDiffLog;
use crate::mysql::command::{Command, MySqlCommand};
use crate::mysql::packet::{OkData, Packet, PacketType};
#[cfg(feature = "tls")]
use crate::tls::{handle_client_tls, handle_server_tls};
use crate::{connection::Connection, materialization, state_handler};
use log::{debug, error};
#[cfg(feature = "tls")]
use rustls::StreamOwned;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::str::FromStr;
use std::sync::atomic::AtomicU8;
use std::sync::LazyLock;
use std::time::Duration;
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
        Phase::AuthSwitchResponse,
    ])
});

static CLIENT_TRANSITION_PHASES: LazyLock<HashSet<Phase>> =
    LazyLock::new(|| HashSet::from([Phase::AuthInit, Phase::PendingResponse, Phase::AuthComplete]));

pub fn initiate(client: TcpStream, state_difference_map: StateDiffLog) {
    let target_address = env::var("TARGET_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3307".to_owned());

    let server = TcpStream::connect(target_address).expect("Fault");

    let connection = Connection::new(
        SwitchableConnection::Plain(RefCell::new(server)),
        SwitchableConnection::Plain(RefCell::new(client)),
        state_difference_map,
    );

    let worker = thread::spawn(move || exchange(connection));

    worker.join().ok();
}

fn exchange(mut connection: Connection) -> Result<(), Error> {
    let mut buf: [u8; 4096] = [0; 4096];

    let mut packets;
    let delay_vars: HashMap<String, String> = env::vars()
        .filter(|(k, _)| k.starts_with("DELAY_"))
        .collect();
    loop {
        // Server Loop
        loop {
            debug!("Listening from server");

            let read_bytes = read_bytes(&mut connection.server_connection, &mut buf)?;

            debug!("From server: {:?}", &buf[0..read_bytes].to_vec());

            if read_bytes == 0 {
                return Ok(());
            }

            packets = state_handler::process_incoming_frame(&buf, &mut connection, read_bytes);

            let encoded_bytes = state_handler::generate_outgoing_frame(&packets);

            write_bytes(&mut connection.client_connection, encoded_bytes.as_slice());

            if SERVER_TRANSITION_PHASES.contains(&connection.phase) {
                debug!("Transitioning to client");
                break;
            }
        }

        // client loop
        loop {
            #[cfg(feature = "tls")]
            if connection.phase == Phase::TlsExchange {
                connection = switch_to_tls(connection);
            }

            debug!("Listening from client: {:?}", &connection.phase);

            let read_bytes = read_bytes(&mut connection.client_connection, &mut buf)?;

            debug!("From client: {:?}", &buf[0..read_bytes].to_vec());

            if read_bytes == 0 {
                return Ok(());
            }

            packets = state_handler::process_incoming_frame(&buf, &mut connection, read_bytes);

            let encoded_bytes = state_handler::generate_outgoing_frame(&packets);

            if !delay_vars.is_empty() {
                delay_if_required(&connection.last_command, &delay_vars);
            }

            if intercept_enabled() && intercept_command(&mut connection, &packets) {
                // Connection returns to command phase if the query is intercepted, so the client loop needs to be started again.
                continue;
            }

            write_bytes(&mut connection.server_connection, encoded_bytes.as_slice());

            if CLIENT_TRANSITION_PHASES.contains(&connection.phase) {
                debug!("Transitioning to server");
                break;
            }
        }
    }
}

fn delay_if_required(last_command: &Option<Command>, delay_vars: &HashMap<String, String>) {
    let last_command = last_command.clone();
    if last_command.is_some() {
        if let Some(command_type) = last_command.unwrap().arg.split_whitespace().next() {
            let key = "DELAY_".to_string() + &*command_type.to_uppercase();
            if delay_vars.contains_key(&key) {
                match env::var(&key) {
                    Ok(_) => {
                        let delay = u64::from_str(delay_vars.get(&key).unwrap());

                        match delay {
                            Ok(delay) => {
                                thread::sleep(Duration::from_millis(delay));
                                debug!("Delaying for {}", delay);
                            }
                            Err(_) => {
                                error!("Delaying for {} failed", key);
                            }
                        }
                    }
                    Err(_) => {
                        error!("DELAY_{key}: environment variable not found");
                    }
                }
            }
        }
    }
}

#[cfg(feature = "tls")]
fn switch_to_tls(mut connection: Connection) -> Connection {
    let server_tls = handle_server_tls();
    let client_tls = handle_client_tls();

    if let (SwitchableConnection::Plain(_), SwitchableConnection::Plain(_)) =
        (&connection.server_connection, &connection.client_connection)
    {
        connection.server_connection = SwitchableConnection::ServerTls(RefCell::new(
            StreamOwned::new(server_tls, connection.server_connection.take()),
        ));
        connection.client_connection = SwitchableConnection::ClientTls(RefCell::new(
            StreamOwned::new(client_tls, connection.client_connection.take()),
        ));
    }

    connection.phase = Phase::HandshakeResponse;
    debug!("TLS Set");
    connection
}

pub fn read_bytes(conn: &mut SwitchableConnection, buf: &mut [u8]) -> Result<usize, Error> {
    match conn {
        SwitchableConnection::Plain(stream) => stream.get_mut().read(buf),
        #[cfg(feature = "tls")]
        SwitchableConnection::ClientTls(stream_owned) => stream_owned.get_mut().read(buf),
        #[cfg(feature = "tls")]
        SwitchableConnection::ServerTls(stream_owned) => stream_owned.get_mut().read(buf),
        #[cfg(test)]
        SwitchableConnection::None => unreachable!(),
    }
}

pub fn write_bytes(conn: &mut SwitchableConnection, buf: &[u8]) {
    let _ = match conn {
        SwitchableConnection::Plain(stream) => stream.get_mut().write_all(buf),
        #[cfg(feature = "tls")]
        SwitchableConnection::ClientTls(stream_owned) => stream_owned.get_mut().write_all(buf),
        #[cfg(feature = "tls")]
        SwitchableConnection::ServerTls(stream_owned) => stream_owned.get_mut().write_all(buf),
        #[cfg(test)]
        SwitchableConnection::None => unreachable!(),
    };
}

fn get_write_response(last_command: Command, sequence: &u8, client_flag: u32) -> Option<Vec<u8>> {
    let count = GLOBAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ok_data = OkData {
        header: 0x00,
        affected_rows: 1,
        last_insert_id: match last_command.arg.to_lowercase().starts_with("insert ") {
            true => count as u64,
            false => 0,
        },
        status_flags: None,
        warnings: None,
        info: None,
        session_state_info: None,
    };

    Some(ok_data.to_packet(sequence + 1, client_flag).to_bytes())
}

fn is_write_query(
    last_command: &Option<Command>,
    packet: &Packet,
    diff: &mut StateDiffLog,
) -> bool {
    if last_command.is_none() {
        return false;
    }

    let last_command = last_command.as_ref().unwrap();
    let last_command_arg = &last_command.arg.to_lowercase();

    let ret = packet.p_type.eq(&PacketType::Command)
        && last_command.com_code.eq(&MySqlCommand::ComQuery)
        && (last_command_arg.starts_with("insert")
            || last_command_arg.starts_with("update")
            || last_command_arg.starts_with("delete"));

    materialization::get_diff(diff, &last_command.arg);

    ret
}

fn intercept_enabled() -> bool {
    env::var("INTERCEPT_WRITES").is_ok() && env::var("INTERCEPT_WRITES").unwrap() == "true"
}

fn intercept_command(connection: &mut Connection, packets: &[Packet]) -> bool {
    if connection.phase != Phase::PendingResponse {
        return false;
    }

    let last_command = connection.last_command.clone();
    let client_flag = connection
        .handshake_response
        .as_ref()
        .map(|hr| hr.client_flag);

    if packets.len() == 1
        && is_write_query(
            &last_command,
            packets.first().unwrap(),
            &mut connection.diff,
        )
    {
        let last_command = last_command.unwrap();

        if let Some(response) = get_write_response(
            last_command,
            &packets.first().unwrap().header.seq,
            client_flag.unwrap(),
        ) {
            debug!("{:?}", response);
            connection.phase = Phase::Command;
            write_bytes(&mut connection.client_connection, &response);
            return true;
        }
    }

    false
}
