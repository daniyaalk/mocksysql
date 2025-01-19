use std::sync::{Arc, Mutex};
use util::packet_printer;

use crate::{
    connection::{Connection, Direction, State},
    mysql::packet::Packet,
    util,
};

enum PacketParseResult {
    Packet(Packet),
    PartialData(Vec<u8>),
    None,
}

pub fn process_frame(
    buf: &[u8],
    connection: &Arc<Mutex<Connection>>,
    #[allow(unused_variables)] direction: &Direction,
) {
    let packets = make_packets(buf, &mut connection.clone());

    for packet in &packets {
        let mut connection = connection.try_lock().expect("");
        match &connection.get_state() {
            State::Initiated => {
                if packet.is_ok().is_some() {
                    connection.mark_auth_done();
                    println!("Auth Done!")
                }
            }
            _ => (),
        }
    }
}

fn make_packets(buf: &[u8], connection: &Arc<Mutex<Connection>>) -> Vec<Packet> {
    let mut ret: Vec<Packet> = Vec::new();

    let mut offset: usize = 0;

    loop {
        let mut connection = connection.lock().expect("");

        let mut buffer_vec: Vec<u8> = connection.partial_data.clone().unwrap_or(Vec::new());
        buffer_vec.extend_from_slice(buf);

        match parse_buffer(&buffer_vec, &mut offset) {
            PacketParseResult::Packet(p) => {
                verify_packet_order(&ret, &p);
                ret.push(p);
            }
            PacketParseResult::None => {
                // Processing concluded for current buffer as well as packet list, reset partial data.
                connection.unset_partial_data();
                break;
            }
            PacketParseResult::PartialData(pd) => {
                connection.set_partial_data(pd);
                break;
            }
        }
    }

    ret
}

fn verify_packet_order(ret: &[Packet], p: &Packet) {
    if ret.len() > 0 {
        if ret.get(ret.len() - 1).unwrap().header.seq != p.header.seq - 1 {
            panic!("Out of order packet, something is wrong with the decoding!");
        }
    }
}

fn parse_buffer(buf: &Vec<u8>, start_offset: &mut usize) -> PacketParseResult {
    let offset = start_offset.clone();

    if offset == buf.len() || buf[offset] == 0 {
        // If previous processing exhausted the full buffer and the packet bounds coincided with the end of the buffer
        // or if the previous transmission was smaller than the buffer, return None so that the packet list can be finalized.
        return PacketParseResult::None;
    }

    let packet = Packet::from_bytes(&buf[offset..]);

    if packet.is_err() {
        // Unable to parse packet due to buffer going out of bounds.
        return PacketParseResult::PartialData(buf[offset..].to_vec());
    }

    let header = &packet.as_ref().unwrap().header;

    *start_offset = offset + 4 + header.size as usize;

    let packet = packet.unwrap();
    packet_printer::print_packet(&packet);

    PacketParseResult::Packet(packet)
}
