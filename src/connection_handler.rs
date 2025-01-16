use std::{io::{Error, Read, Write}, net::TcpStream, thread, time::Duration};

use crate::connection::Direction;

fn exchange(mut from: TcpStream, mut to: TcpStream, mut direction: Direction) -> Result<(), Error> {

    let mut buf: [u8; 4096] = [0; 4096];


    loop {
        

        let read_bytes = from.read(&mut buf)?;

        if read_bytes == 0 {
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        println!("From {:?}: {:?}", from.peer_addr(), &buf);
        println!("{}", String::from_utf8_lossy(&buf));


        to.write_all(&buf[..read_bytes])?;

    }

}

pub fn initiate(client: TcpStream) {

    let target_address: &str = "127.0.0.1:3307";

    let server = TcpStream::connect(target_address).expect("Fault");

    let server_clone = server.try_clone().expect("Fault");
    let client_clone = client.try_clone().expect("Fault");

    let client_to_server_channel = thread::spawn(move || exchange(client, server_clone, Direction::C2S));
    let server_to_client_channel = thread::spawn(move || exchange(server, client_clone, Direction::S2C));

    client_to_server_channel.join().ok();
    server_to_client_channel.join().ok();
}