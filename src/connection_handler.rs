use crate::connection::{KafkaProducerConfig, Phase, SwitchableConnection};
use crate::materialization::{ReplayLog, StateDiffLog};
use crate::mysql::command::MySqlCommand::ComQuery;
use crate::mysql::command::{Command, MySqlCommand};
use crate::mysql::packet::{OkData, Packet, PacketType};
#[cfg(feature = "tls")]
use crate::tls::{handle_client_tls, handle_server_tls};
use crate::{connection::Connection, materialization, state_handler};
use base64::Engine;
use kafka::producer::{AsBytes, Record};
use log::{debug, error};
use once_cell::sync::Lazy;
#[cfg(feature = "tls")]
use rustls::StreamOwned;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env::VarError;
use std::fs::File;
use std::str::FromStr;
use std::sync::atomic::AtomicU8;
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::sleep;
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

static INTERCEPT_WRITES: Lazy<String> =
    Lazy::new(|| env::var("INTERCEPT_WRITES").unwrap_or_else(|_| "false".to_string()));

static DELAY_VARS: Lazy<HashMap<String, String>> = Lazy::new(|| {
    env::vars()
        .filter(|(k, _)| k.starts_with("DELAY_"))
        .collect()
});

static CLIENT_TRANSITION_PHASES: LazyLock<HashSet<Phase>> =
    LazyLock::new(|| HashSet::from([Phase::AuthInit, Phase::PendingResponse, Phase::AuthComplete]));

pub fn initiate(
    client: TcpStream,
    state_difference_map: StateDiffLog,
    kafka_config: KafkaProducerConfig,
    replay_map: ReplayLog,
) {
    let target_address = env::var("TARGET_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3307".to_owned());

    let server = TcpStream::connect(target_address).expect("Fault");

    let connection = Connection::new(
        SwitchableConnection::Plain(RefCell::new(server)),
        SwitchableConnection::Plain(RefCell::new(client)),
        state_difference_map,
        replay_map,
        kafka_config,
    );

    let worker = thread::spawn(move || exchange(connection));

    worker.join().ok();
}

fn exchange(mut connection: Connection) -> Result<(), Error> {
    let mut buf: [u8; 4096] = [0; 4096];

    let mut packets;

    loop {
        // Server Loop
        loop {
            debug!("Listening from server");

            let mut bytes_count: Option<usize> = None;

            if let Some(replay_logs) = &connection.replay {
                let last_command = connection.get_last_command();
                if last_command.is_some() && last_command.unwrap().com_code == ComQuery {
                    loop {
                        {
                            let replay_logs = replay_logs.lock().unwrap();
                            let entry =
                                replay_logs.get(&connection.get_last_command().unwrap().arg);

                            if let Some(entry) = entry {
                                let base64_bytes = entry.value();
                                if let Ok(bytes) =
                                    base64::engine::general_purpose::STANDARD.decode(base64_bytes)
                                {
                                    let len = bytes.len().min(buf.len());
                                    buf[..len].copy_from_slice(&bytes[..len]);
                                    bytes_count = Some(len);
                                    break;
                                }
                            }
                        }
                        sleep(Duration::from_millis(100));
                        debug!("Timed out waiting for cache population");
                    }
                }
            }

            if None == bytes_count {
                bytes_count = Some(read_bytes(&mut connection.server_connection, &mut buf)?);
            }

            let bytes_count = bytes_count.unwrap_or(0);

            debug!("From server: {:?}", &buf[0..bytes_count].to_vec());

            if bytes_count == 0 {
                return Ok(());
            }

            packets = state_handler::process_incoming_frame(&buf, &mut connection, bytes_count);

            let encoded_bytes = state_handler::generate_outgoing_frame(&packets);

            write_bytes(&mut connection.client_connection, encoded_bytes.as_slice());

            if let Some(command) = &connection.last_command {
                if let Some((topic, producer)) = &connection.kafka_producer_config {
                    let payload = format!(
                        "{{\"last_command\": \"{}\", \"output\": \"{}\"}}\n",
                        escape_json(&command.arg),
                        base64::engine::general_purpose::STANDARD.encode(&encoded_bytes),
                    );

                    let mut handle = producer.lock().unwrap();
                    let res = handle.send(&Record::from_value(&topic, payload.as_bytes().to_vec()));
                    debug!("Kafka response: {:?}", res);
                }
            }

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

            let response_bytes: Option<&[u8]> = None;
            if let Some(replay) = &connection.replay {}

            let read_bytes = read_bytes(&mut connection.client_connection, &mut buf)?;

            debug!("From client: {:?}", &buf[0..read_bytes].to_vec());

            if read_bytes == 0 {
                return Ok(());
            }

            packets = state_handler::process_incoming_frame(&buf, &mut connection, read_bytes);

            let encoded_bytes = state_handler::generate_outgoing_frame(&packets);

            if !DELAY_VARS.is_empty() {
                delay_if_required(&connection.last_command, &DELAY_VARS);
            }

            if intercept_enabled() && intercept_command(&mut connection, &packets) {
                // Connection returns to command phase if the query is intercepted, so the client loop needs to be started again.
                continue;
            }

            if connection.replay.is_none()
                || connection.get_last_command().is_none()
                || connection.get_last_command().unwrap().com_code != ComQuery
            {
                write_bytes(&mut connection.server_connection, encoded_bytes.as_slice());
            }

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
    *INTERCEPT_WRITES == "true"
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

fn escape_json(user_input: &str) -> String {
    user_input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
