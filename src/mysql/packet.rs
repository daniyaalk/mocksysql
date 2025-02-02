use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::CapabilityFlags;
use crate::mysql::types::{Converter, IntFixedLen, StringEOFEnc, StringFixedLen};
use std::{fmt::Error, usize};

#[derive(Debug)]
pub struct Packet {
    pub header: PacketHeader,
    pub body: Vec<u8>,
    pub p_type: PacketType,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum PacketType {
    Other,
    Command,
    Ok,
    Eof,
    Error,
}

#[derive(Debug)]
pub struct PacketHeader {
    pub size: usize,
    pub seq: u8,
}

impl PacketHeader {
    pub fn from_bytes(bytes: &[u8; 4]) -> Self {
        let mut temp: [u8; 8] = [0; 8];
        temp[..3].clone_from_slice(&bytes[0..3]);

        let seq = bytes[3];
        let size = usize::from_le_bytes(temp);

        Self { size, seq }
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        let mut ret: [u8; 4] = [0x00; 4];
        ret.clone_from_slice(&usize::to_le_bytes(self.size)[0..4]);
        ret[3] = self.seq;

        ret
    }
}

impl Packet {
    pub fn from_bytes(bytes: &[u8], phase: Phase) -> Result<Packet, Error> {
        if bytes.len() < 4 {
            return Err(Error {});
        }

        let raw_header: [u8; 4] = bytes[0..4].try_into().expect("Slice with incorrect length");
        let header = PacketHeader::from_bytes(&raw_header);

        if bytes.len() < 4 + header.size {
            return Err(Error {});
        }
        let body = bytes[4..4 + header.size].to_vec();

        let p_type: PacketType = get_packet_type(&body, phase);

        Ok(Packet {
            header,
            body,
            p_type,
        })
    }

    #[allow(dead_code)]
    pub fn to_bytes(self) -> Vec<u8> {
        let mut ret: Vec<u8> = Vec::new();

        ret.extend(self.header.to_bytes());
        ret.extend(self.body.to_vec());
        ret
    }
}

fn get_packet_type(body: &[u8], phase: Phase) -> PacketType {
    // TODO: Look into this, the mysql documentation suggests a packet size of 7 and 9 respectively.
    if body.len() >= 7 && body[0] == 0x00 {
        return PacketType::Ok;
    }

    if body.len() <= 9 && body[0] == 0xfe {
        return PacketType::Eof;
    }

    if body[0] == 0xff {
        return PacketType::Error;
    }

    if Phase::Command == phase {
        return PacketType::Command;
    }

    PacketType::Other
}

impl Packet {
    pub fn get_packet_type(&self) -> PacketType {
        self.p_type.clone() // TODO: Change this to a Cell
    }
}

#[derive(Debug, Clone)]
pub struct ErrorPacket {
    pub error_code: u16,
    pub sql_state: Option<SQLState>,
    pub error_message: String,
}

impl ErrorPacket {
    pub fn from_packet(packet: &Packet, connection: &Connection) -> ErrorPacket {
        assert_eq!(packet.p_type, PacketType::Error);
        let body = &packet.body;

        let mut offset = 0;

        ErrorPacket {
            error_code: {
                let result = IntFixedLen::from_bytes(body, Some(2));
                offset += result.offset_increment;
                result.result as u16
            },
            sql_state: {
                let sql_state = get_sql_state(packet, connection, &offset);
                offset += 6;
                sql_state
            },
            error_message: {
                let result = StringEOFEnc::from_bytes(&body[offset..].to_vec(), None);
                offset += result.offset_increment;
                assert_eq!(offset, body.len());
                result.result
            },
        }
    }
}

fn get_sql_state(packet: &Packet, connection: &Connection, offset: &usize) -> Option<SQLState> {
    if connection.get_handshake_response().unwrap().client_flag
        & CapabilityFlags::ClientProtocol41 as u32
        == 0
    {
        return None;
    }

    let mut state_offset = *offset;
    Some(SQLState {
        state_marker: {
            let result = StringFixedLen::from_bytes(&packet.body[state_offset..].to_vec(), Some(1));
            state_offset += result.offset_increment;
            result.result
        },
        state: {
            let result = StringFixedLen::from_bytes(&packet.body[state_offset..].to_vec(), Some(5));
            state_offset += result.offset_increment;
            assert_eq!(state_offset - offset, 6);
            result.result
        },
    })
}

#[derive(Debug, Clone)]
pub struct SQLState {
    state_marker: String,
    state: String,
}

#[allow(dead_code)]
pub struct OkData {
    rows_len: usize,
    id: u64,
    server_status: [u8; 2],
    warning_count: [u8; 2],
    message: Vec<u8>,
}
