use crate::connection::Phase;
use crate::mysql::command::{Command, MySqlCommand};
use crate::mysql::packet::{OkData, Packet, PacketType};
use crate::{
    connection::{Connection, Direction},
    state_handler,
};
use std::sync::atomic::AtomicU8;
use std::{
    env,
    io::{Error, Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

static GLOBAL_COUNTER: AtomicU8 = AtomicU8::new(100);

fn exchange(
    mut from: TcpStream,
    mut to: TcpStream,
    direction: Direction,
    _client_addess: SocketAddr,
    connection: Arc<Mutex<Connection>>,
) -> Result<(), Error> {
    loop {
        let mut buf: [u8; 4096] = [0; 4096];

        let read_bytes = from.read(&mut buf).expect("its joever");

        if read_bytes == 0 {
            break;
        }

        let current_phase = { connection.lock().unwrap().phase.clone() };
        if current_phase == Phase::TlsExchange {
            handle_tls(connection.clone(), from.try_clone()?, &mut buf, &direction);
        }

        let packets = state_handler::process_incoming_frame(&buf, &connection, &direction);

        if intercept_enabled() {
            let mut connection_mutex = connection.lock().unwrap();
            let last_command = connection_mutex.last_command.clone();
            let client_flag = connection_mutex
                .handshake_response
                .as_ref()
                .map(|hr| hr.client_flag);

            if packets.len() == 1 && is_write_query(&last_command, packets.first().unwrap()) {
                if let Some(response) = get_write_response(
                    &last_command,
                    &packets.first().unwrap().header.seq,
                    client_flag.unwrap(),
                ) {
                    println!("{:?}", response);
                    connection_mutex.phase = Phase::Command;
                    from.write_all(&response)?;
                    continue;
                }
            }
        }

        to.write_all(&buf[..read_bytes])?;
    }

    Ok(())
}

fn handle_tls(
    connection: Arc<Mutex<Connection>>,
    from: TcpStream,
    buf: &mut [u8; 4096],
    direction: &Direction,
) {
    let mut connection = connection.lock().unwrap();
    connection.phase = Phase::HandshakeResponse;
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
    let connection: Arc<Mutex<Connection>> = Arc::new(Mutex::new(Connection::default()));
    let connection2 = Arc::clone(&connection);

    let target_address: &str = "127.0.0.1:3307";
    let client_address = client.peer_addr().unwrap();

    let server = TcpStream::connect(target_address).expect("Fault");

    let server_clone = server.try_clone().expect("Fault");
    let client_clone = client.try_clone().expect("Fault");

    let client_to_server_channel = thread::spawn(move || {
        exchange(
            client,
            server_clone,
            Direction::C2S,
            client_address,
            connection,
        )
    });
    let server_to_client_channel = thread::spawn(move || {
        exchange(
            server,
            client_clone,
            Direction::S2C,
            client_address,
            connection2,
        )
    });

    client_to_server_channel.join().ok();
    server_to_client_channel.join().ok();
}

fn intercept_enabled() -> bool {
    env::var("INTERCEPT_INSERT").is_ok() && env::var("INTERCEPT_INSERT").unwrap() == "true"
}
