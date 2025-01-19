use std::{
    io::{Error, Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use crate::{
    connection::{Connection, Direction},
    state_handler,
};

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

        state_handler::process_frame(&buf, &connection, &direction);

        to.write_all(&buf[..read_bytes])?;
    }

    Ok(())
}

pub fn initiate(client: TcpStream) {
    let connection: Arc<Mutex<Connection>> = Arc::new(Mutex::new(Connection::new()));
    let connection2 = Arc::clone(&connection);

    let target_address: &str = "127.0.0.1:3307";
    let client_address = client.peer_addr().unwrap().clone();

    let server = TcpStream::connect(target_address).expect("Fault");

    let server_clone = server.try_clone().expect("Fault");
    let client_clone = client.try_clone().expect("Fault");

    let client_to_server_channel = thread::spawn(move || {
        exchange(
            client,
            server_clone,
            Direction::C2S,
            client_address.clone(),
            connection,
        )
    });
    let server_to_client_channel = thread::spawn(move || {
        exchange(
            server,
            client_clone,
            Direction::S2C,
            client_address.clone(),
            connection2,
        )
    });

    client_to_server_channel.join().ok();
    server_to_client_channel.join().ok();
}
