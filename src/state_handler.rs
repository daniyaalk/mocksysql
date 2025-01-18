use std::sync::{Arc, Mutex};
use util::packet_printer;

use crate::{connection::{Connection, Direction, State}, mysql::packet::Packet, util};


pub fn process_frame(buf: &[u8], connection: &Arc<Mutex<Connection>>, direction: &Direction) {
    
    let mut connection = connection.try_lock().expect("Error when trying to acquire lock");

    let packets = make_packets(buf, &connection.partial_data);

    for packet in &packets {
        println!("{:?} {:?}", direction, packet);
        // println!("{:X?}", buf);
        packet_printer::print_packet(&packet);

        match &connection.get_state() {
            State::Initiated => {
                if packet.is_ok().is_some() {
                    connection.mark_auth_done();
                    println!("Auth Done!")
                }
            }
            _ => ()
        }

    }

}


fn make_packets(buf: &[u8], partial_data: &Option<Vec<u8>>) -> Vec<Packet> {

    let mut ret: Vec<Packet> = Vec::new();

    let mut offset: usize = 0;

    loop {

        let buffer_vec:Vec<u8> = partial_data
                                    .clone()
                                    .unwrap_or(Vec::new())
                                    .into_iter()
                                    .chain(buf.to_vec())
                                    .collect();

        match parse_buffer(buffer_vec, &mut offset) {

            Some(packet) => {
                ret.push(packet);
            },
            
            None => {
                // if offset > buf.len() {
                //     break;
                // }
                break;
            }

        }
    }

    ret
}

fn parse_buffer(buf: Vec<u8>, start_offset: &mut usize) -> Option<Packet> {

    let offset = start_offset.clone();

    if offset + 4 <=  buf.len() {

        let packet =  Packet::from_bytes(&buf[offset..]);

        if packet.is_err() {
            return None;
        }

        let header = &packet.as_ref().unwrap().header;

        if header.size == 0 || offset + header.size > buf.len() {
            return None;
        }

        *start_offset += 4 + header.size;

        return Some(packet.unwrap());
    }

    None
}