use crate::connection::Phase;
use crate::mysql::command::{Command, MySqlCommand};
use crate::mysql::packet::{OkData, Packet, PacketType};
use crate::tls;
use crate::{
    connection::{Connection, Direction},
    state_handler,
};
use rustls::StreamOwned;
use std::sync::atomic::AtomicU8;
use std::time::Duration;
use std::{
    env,
    io::{Error, Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

static GLOBAL_COUNTER: AtomicU8 = AtomicU8::new(100);

fn exchange<RWS: Read + Write + Sized>(
    mut from: RWS,
    mut to: RWS,
    direction: Direction,
    _client_address: SocketAddr,
    connection: Arc<Mutex<Connection>>,
) -> Result<(), Error> {
    let mut tls_done: bool = false;
    // let mut client_connection = None;
    // let mut server_connection = None;

    loop {
        let mut buf: [u8; 4096] = [0; 4096];

        if direction == Direction::C2S && !tls_done {
            let mut connection = connection.try_lock().unwrap();
            let current_phase = &connection.phase;
            if current_phase == &Phase::TlsExchange {
                let (server_tls, client_tls) = tls::handle_tls(&mut from, &mut to, &mut buf);
                connection.phase = Phase::HandshakeResponse;
                connection.client_connection = Some(client_tls.clone());
                connection.server_connection = Some(server_tls);

                from = StreamOwned::new(client_tls, from).sock;
                to = StreamOwned::new(server_tls, to).sock;

                tls_done = true;
                continue;
            }
        } else if direction == Direction::S2C && !tls_done {
            loop {
                let connection = connection.lock().unwrap();
                if connection.phase == Phase::TlsExchange && connection.server_connection.is_none()
                {
                    thread::sleep(Duration::from_millis(100));
                } else {
                    break;
                }
            }
        }

        let read_bytes = from.read(&mut buf).expect("its joever");
        // let read_bytes = read_bytes(
        //     &mut from,
        //     &mut buf,
        //     &direction,
        //     &mut server_connection,
        //     &mut client_connection,
        // );

        if read_bytes == 0 {
            panic!();
            break;
        }

        let packets = state_handler::process_incoming_frame(&buf, &connection, &direction);

        if intercept_enabled() {
            let mut connection_mutex = connection.try_lock().unwrap();
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
        // write_bytes(
        //     &mut to,
        //     &buf[..read_bytes],
        //     &direction,
        //     &mut server_connection,
        //     &mut client_connection,
        // )?
    }

    Ok(())
}

// fn write_bytes<RWS: Read + Write + Sized>(
//     to: &mut RWS,
//     buf: &[u8],
//     direction: &Direction,
//     server_connection: &mut Option<ServerConnection>,
//     client_connection: &mut Option<ClientConnection>,
// ) -> std::io::Result<()> {
//     if server_connection.is_none() {
//         return to.write_all(buf);
//     }
//     println!("Here!");
//     let mut writer = match direction {
//         Direction::C2S => server_connection.as_mut().unwrap().write_tls(),
//         Direction::S2C => client_connection.as_mut().unwrap().wrrite_tls(),
//     };
//
//     println!("Writing {:?} {:?}", direction, buf.to_vec());
//     writer.write_all(buf)
// }
//
// fn read_bytes<RWS: Read + Write + Sized>(
//     from: &mut RWS,
//     buf: &mut [u8],
//     direction: &Direction,
//     server_connection: &mut Option<ServerConnection>,
//     client_connection: &mut Option<ClientConnection>,
// ) -> usize {
//     let bytes_read;
//
//     if server_connection.is_none() {
//         bytes_read = from.read(buf).unwrap();
//     } else {
//         println!("Here!");
//         bytes_read = match direction {
//             Direction::C2S => {
//                 println!("C2S TLS Message");
//                 client_connection
//                     .as_mut()
//                     .unwrap()
//                     .reader()
//                     .read(buf)
//                     .unwrap()
//             }
//             Direction::S2C => {
//                 println!("S2C TLS Message");
//                 server_connection
//                     .as_mut()
//                     .unwrap()
//                     .reader()
//                     .read(buf)
//                     .unwrap()
//             }
//         }
//     }
//
//     println!("Read {:?} {:?}", direction, buf[..bytes_read].to_vec());
//     bytes_read
// }

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
