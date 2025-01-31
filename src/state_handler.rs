use std::sync::{Arc, Mutex};
use util::packet_printer;

use crate::mysql::accumulator::auth_complete::AuthCompleteAccumulator;
use crate::mysql::accumulator::auth_init::AuthInitAccumulator;
use crate::mysql::accumulator::auth_switch_request::AuthSwitchRequestAccumulator;
use crate::mysql::accumulator::auth_switch_response::AuthSwitchResponseAccumulator;
use crate::mysql::accumulator::result_set::ResultSetAccumulator;
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
    let mut pending_response = ResultSetAccumulator::default();

    for packet in &packets {
        let mut connection = connection
            .try_lock()
            .expect("Transmission race condition / lock failed.");
        // get_accumulator()
        match &connection.get_state().clone() {
            Phase::Handshake => {
                let mut handshake = HandshakeAccumulator::default();
                connection.phase = handshake.consume(packet, &connection);
                connection.handshake = Some(handshake);
            }
            Phase::HandshakeResponse => {
                let mut handshake_response = HandshakeResponseAccumulator::default();
                connection.phase = handshake_response.consume(packet, &connection);
                connection.handshake_response = Some(handshake_response);
            }
            Phase::AuthInit => {
                if packet.body[0] == 0xfe {
                    // AuthSwitchRequest
                    connection.phase = Phase::AuthSwitchRequest;
                    let mut auth_switch_request = AuthSwitchRequestAccumulator::default();
                    connection.phase = auth_switch_request.consume(packet, &connection);
                } else {
                    // AuthComplete
                }
            }
            Phase::AuthSwitchRequest => {
                // AuthSwitchRequest is not intended to be transitioned into organically,
                // it should be inferred by 0xfe packet after HandshakeResponse
                panic!("Untracked state transition");
            }
            Phase::AuthSwitchResponse => {
                let mut auth_switch_response = AuthSwitchResponseAccumulator::default();
                connection.phase = auth_switch_response.consume(packet, &connection);
            }
            Phase::AuthComplete => {
                let mut auth_complete_accumulation = AuthCompleteAccumulator::default();
                connection.phase = auth_complete_accumulation.consume(packet, &connection);
            }
            Phase::AuthFailed => {
                panic!(
                    "Untracked state transition, no transmissions should occur after auth failure."
                );
            }
            Phase::Command => {
                println!("Entered command phase!");
                connection.phase = Phase::PendingResponse;
                connection.last_command =
                    Some(crate::mysql::command::Command::from_bytes(&packet.body));
            }
            Phase::PendingResponse => {
                connection.phase = pending_response.consume(packet, &connection);
            }
        }

        println!("{:?}", connection.get_state());
    }
    if pending_response.accumulation_complete() {
        println!("{:?}", pending_response);
    }

    packets
}

fn get_accumulator(phase: Phase) -> Box<dyn Accumulator> {
    match phase {
        Phase::Handshake => Box::from(HandshakeAccumulator::default()),
        Phase::HandshakeResponse => Box::from(HandshakeResponseAccumulator::default()),
        Phase::AuthInit => Box::from(AuthInitAccumulator::default()),
        Phase::AuthSwitchRequest => Box::from(AuthSwitchRequestAccumulator::default()),
        Phase::AuthSwitchResponse => Box::from(AuthSwitchResponseAccumulator::default()),
        Phase::AuthFailed => {
            panic!("Untracked state transition, no transmissions should occur after auth failure.");
        }
        Phase::AuthComplete => Box::from(AuthCompleteAccumulator::default()),
        Phase::Command => {
            todo!()
        }
        Phase::PendingResponse => Box::from(ResultSetAccumulator::default()),
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
    packet_printer::print_packet(&packet);

    PacketParseResult::Packet(packet)
}
