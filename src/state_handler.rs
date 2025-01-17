use std::sync::{Arc, Mutex, MutexGuard};

use crate::{connection::{Connection, Direction, State}, mysql::packet::{self, Packet, PacketHeader}};


pub fn process_frame(buf: &[u8], connection: &Arc<Mutex<Connection>>, direction: &Direction) {
    
    let mut connection = connection.try_lock().expect("Error when trying to acquire lock");

    let packets = make_packets(buf, &mut connection);

    for packet in &packets {
        println!("{:?} {:?}", direction, packet);
        println!("{:X?}", packet.body);
        println!("{}", String::from_utf8_lossy(&packet.body));
        match &connection.get_state() {
            State::Initiated => {
                if packet.is_ok().is_some() {
                    connection.state = State::AuthDone;
                    println!("Auth Done!")
                }
            }
            _ => ()
        }

    }

}


fn make_packets(buf: &[u8], connection: &mut MutexGuard<'_, Connection>) -> Vec<Packet> {

    let mut ret: Vec<Packet> = Vec::new();
    
    match &connection.partial_data {
        None => {

            let mut offset = 0;

            loop {

                match parse_buffer(&buf, offset) {

                    Some(packet) => {
                        offset += 4 + packet.header.size;
                        ret.push(packet);
                    },
                    None => break

                }
            }
        },
        Some(data) => ()
    }

    ret
}

fn parse_buffer(buf: &[u8], offset: usize) -> Option<Packet> {

    // let offset = offset.clone();

    if offset + 4 <=  buf.len() {

        let mut header_bytes: [u8; 4] = [0; 4];
        header_bytes.copy_from_slice(&buf[offset .. offset+4]);
        let header: PacketHeader = PacketHeader::from_bytes(&header_bytes);

        if header.size == 0 || offset + header.size > buf.len() {
            return None;
        }

        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&buf[offset+4 .. offset + 4 + header.size]);


        return Some(Packet{
            header, body
        });
    }
    None
}