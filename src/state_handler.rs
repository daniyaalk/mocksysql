use std::sync::{Arc, Mutex, MutexGuard};
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
    connection::{Connection, Direction, Phase},
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
    connection: &Arc<Mutex<Connection>>,
    #[allow(unused_variables)] direction: &Direction,
) -> Vec<Packet> {
    let packets = make_packets(buf, connection.clone());

    for packet in &packets {
        let mut connection = connection
            .try_lock()
            .expect("Transmission race condition / lock failed.");

        let mut accumulator = get_accumulator(
            connection.phase.clone(),
            connection.get_response_accumulator(),
        );

        packet_printer::print_packet(packet);
        connection.phase = accumulator.consume(packet, &connection);

        sync_connection_state(&mut connection, accumulator);

        println!("{:?}", connection.get_state());
    }

    packets
}

fn sync_connection_state(
    connection: &mut MutexGuard<Connection>,
    accumulator: Box<dyn Accumulator>,
) {
    if accumulator.get_accumulation_delta().is_some() {
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
        Phase::HandshakeResponse => Box::from(HandshakeResponseAccumulator::default()),
        Phase::AuthInit => Box::from(AuthInitAccumulator::default()),
        Phase::AuthSwitchRequest => panic!("Auth switch request should be transitioned into via the accumulator for AuthInit, not by packet processing loop."),
        Phase::AuthSwitchResponse => Box::from(AuthSwitchResponseAccumulator::default()),
        Phase::AuthFailed => {
            panic!("Untracked state transition, no transmissions should occur after auth failure.");
        }
        Phase::AuthComplete => Box::from(AuthCompleteAccumulator::default()),
        Phase::Command => Box::from(CommandAccumulator::default()),
        Phase::PendingResponse => Box::from(response_accumulator), // yuck!
    }
}

pub fn process_outgoing_packets(
    packets: Vec<Packet>,
    connection: Arc<Mutex<Connection>>,
    direction: Direction,
) -> Option<Vec<u8>> {
    None
}

fn make_packets(buf: &[u8], connection: Arc<Mutex<Connection>>) -> Vec<Packet> {
    let mut ret: Vec<Packet> = Vec::new();

    let mut offset: usize = 0;

    loop {
        let mut connection = connection
            .lock()
            .expect("Connection lock failed when reading packet.");

        let mut buffer_vec: Vec<u8> = connection.partial_bytes.clone().unwrap_or_default();
        buffer_vec.extend_from_slice(buf);

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
    if ret.len() > 0 {
        let cur_seq = p.header.seq;
        let prev_seq = ret.get(ret.len() - 1).unwrap().header.seq;

        // Check if current packet seq is previous packet seq + 1.
        // Special handling for seq 0, as it can occur upon rollover from 255.
        if (cur_seq != 0 && prev_seq != cur_seq - 1) || (cur_seq == 0 && prev_seq != 255) {
            panic!("Out of order packet, something is wrong with the decoding!");
        }
    }
}

fn parse_buffer(buf: &Vec<u8>, start_offset: &mut usize, phase: Phase) -> PacketParseResult {
    let offset = start_offset.clone();

    if offset == buf.len() || buf[offset] == 0 {
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

    *start_offset = offset + 4 + header.size as usize;

    let packet = packet.unwrap();

    PacketParseResult::Packet(packet)
}
