use std::{arch::x86_64::_CMP_NEQ_US, sync::{Arc, Mutex, MutexGuard}};
use util::packet_printer;

use crate::{connection::{self, Connection, Direction, State}, mysql::packet::{self, Packet, PacketHeader}, util};


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

        let mut header_bytes: [u8; 4] = [0; 4];
        header_bytes.copy_from_slice(&buf[offset .. offset+4]);
        let header: PacketHeader = PacketHeader::from_bytes(&header_bytes);

        if header.size == 0 || offset + header.size > buf.len() {
            return None;
        }

        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&buf[offset+4 .. offset + 4 + header.size]);

        *start_offset += 4 + header.size;
        return Some(Packet{
            header, body
        });
    }

    None
}