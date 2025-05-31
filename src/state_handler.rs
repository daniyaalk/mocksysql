use util::packet_printer;

use crate::mysql::accumulator::auth_complete::AuthCompleteAccumulator;
use crate::mysql::accumulator::auth_init::AuthInitAccumulator;
use crate::mysql::accumulator::auth_switch_response::AuthSwitchResponseAccumulator;
use crate::mysql::accumulator::command::CommandAccumulator;
use crate::mysql::accumulator::result_set::ResponseAccumulator;
use crate::mysql::accumulator::Accumulator;
use crate::mysql::accumulator::{
    handshake::HandshakeAccumulator, handshake_response::HandshakeResponseAccumulator,
};
use crate::{
    connection::{Connection, Phase},
    mysql::packet::Packet,
    util,
};

enum PacketParseResult {
    Packet(Packet),
    PartialData(Vec<u8>),
    None,
}

pub fn process_incoming_frame(
    buf: &[u8],
    connection: &mut Connection,
    read_bytes: usize,
) -> Vec<Packet> {
    let in_packets = make_packets(buf, connection, read_bytes);
    let mut out_packets = vec![];

    for mut packet in in_packets {
        let mut accumulator = get_accumulator(
            connection.phase.clone(),
            connection.get_response_accumulator(),
        );

        packet_printer::print_packet(&packet);

        connection.phase = accumulator.consume(&mut packet, connection);
        out_packets.push(packet);

        sync_connection_state(connection, accumulator);

        println!("{:?}", connection.get_state());
    }

    out_packets
}

fn sync_connection_state(connection: &mut Connection, accumulator: Box<dyn Accumulator>) {
    if accumulator.accumulation_complete() && accumulator.get_accumulation_delta().is_some() {
        let delta = accumulator.get_accumulation_delta().unwrap();

        if delta.handshake.is_some() {
            connection.handshake = delta.handshake
        }
        if delta.handshake_response.is_some() {
            connection.handshake_response = delta.handshake_response
        }
        if delta.last_command.is_some() {
            connection.last_command = delta.last_command
        }
        if delta.response.is_some() {
            connection.set_response_accumulator(delta.response.unwrap());
        }
    }
}

fn get_accumulator(
    phase: Phase,
    response_accumulator: ResponseAccumulator,
) -> Box<dyn Accumulator> {
    match phase {
        Phase::Handshake => Box::from(HandshakeAccumulator::default()),
        Phase::TlsExchange => unreachable!(),
        Phase::HandshakeResponse => Box::from(HandshakeResponseAccumulator::default()),
        Phase::AuthInit => Box::from(AuthInitAccumulator::default()),
        Phase::AuthSwitchResponse => Box::from(AuthSwitchResponseAccumulator::default()),
        Phase::AuthFailed => {
            panic!("Untracked state transition, no transmissions should occur after auth failure.");
        }
        Phase::AuthComplete => Box::from(AuthCompleteAccumulator::default()),
        Phase::Command => Box::from(CommandAccumulator::default()),
        Phase::PendingResponse => Box::from(response_accumulator), // yuck!
    }
}

fn make_packets(buf: &[u8], connection: &mut Connection, read_bytes: usize) -> Vec<Packet> {
    let mut ret: Vec<Packet> = Vec::new();

    let mut offset: usize = 0;

    loop {
        let mut buffer_vec: Vec<u8> = connection.partial_bytes.clone().unwrap_or_default();
        buffer_vec.extend_from_slice(&buf[..read_bytes]);

        match parse_buffer(&buffer_vec, &mut offset, connection.phase.clone()) {
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
    if !ret.is_empty() {
        let cur_seq = p.header.seq;
        let prev_seq = ret.last().unwrap().header.seq;

        // Check if current packet seq is previous packet seq + 1.
        // Special handling for seq 0, as it can occur upon rollover from 255.
        if (cur_seq != 0 && prev_seq != cur_seq - 1) || (cur_seq == 0 && prev_seq != 255) {
            panic!("Out of order packet, something is wrong with the decoding!");
        }
    }
}

fn parse_buffer(buf: &[u8], start_offset: &mut usize, phase: Phase) -> PacketParseResult {
    let offset = *start_offset;

    if offset == buf.len() {
        // If previous processing exhausted the full buffer and the packet bounds coincided with the end of the buffer
        // or if the previous transmission was smaller than the buffer, return None so that the packet list can be finalized.
        return PacketParseResult::None;
    }

    let packet = Packet::from_bytes(&buf[offset..], phase);

    if packet.is_err() {
        // Unable to parse packet due to buffer going out of bounds.
        return PacketParseResult::PartialData(buf[offset..].to_vec());
    }

    let header = &packet.as_ref().unwrap().header;

    *start_offset = offset + 4 + header.size;

    let packet = packet.unwrap();

    PacketParseResult::Packet(packet)
}

pub fn generate_outgoing_frame(packet: &Vec<Packet>) -> Vec<u8> {
    let mut ret: Vec<u8> = Vec::new();

    for packet in packet.iter() {
        ret.append(&mut packet.header.to_bytes().to_vec());
        ret.append(&mut packet.body.to_vec());
    }

    ret
}
