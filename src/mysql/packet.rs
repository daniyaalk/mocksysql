use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::CapabilityFlags;
use crate::mysql::types::{Converter, IntFixedLen, IntLenEnc, StringEOFEnc, StringFixedLen};
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
pub struct ErrorData {
    #[allow(dead_code)]
    pub error_code: u16,
    #[allow(dead_code)]
    pub sql_state: Option<SQLState>,
    #[allow(dead_code)]
    pub error_message: String,
}

impl ErrorData {
    pub fn from_packet(packet: &Packet, connection: &Connection) -> ErrorData {
        assert_eq!(packet.p_type, PacketType::Error);
        let body = &packet.body;

        let mut offset = 1;

        ErrorData {
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
    #[allow(dead_code)]
    state_marker: String,
    #[allow(dead_code)]
    state: String,
}

#[derive(Debug)]
pub struct SessionState {
    #[allow(dead_code)]
    type_: u8,
    #[allow(dead_code)]
    data: String,
}

#[repr(u16)]
pub enum ServerStatusFlags {
    ServerMoreResultsExist = 0x08,
    #[allow(dead_code)]
    ServerSessionStateChanged = 0x01 << 14,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct OkData {
    pub header: u8,
    pub affected_rows: u64,
    pub last_insert_id: u64,
    pub status_flags: Option<u16>,
    pub warnings: Option<u16>,
    pub info: Option<String>,
    pub session_state_info: Option<SessionState>,
}

impl OkData {
    pub fn from_packet(packet: &Packet, connection: &Connection) -> OkData {
        assert_eq!(packet.p_type, PacketType::Ok);

        let mut offset = 0;
        let body = &packet.body;

        let header = {
            let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(1));
            assert!(result.result == 0x00 || result.result == 0xFE);
            offset += result.offset_increment;
            result.result as u8
        };

        let affected_rows = {
            let result = IntLenEnc::from_bytes(&body[offset..].to_vec(), None);
            offset += result.offset_increment;
            result.result
        };

        let last_insert_id = {
            let result = IntLenEnc::from_bytes(&body[offset..].to_vec(), None);
            offset += result.offset_increment;
            result.result
        };

        let mut status_flags = None;
        let mut warnings = None;
        if connection.get_handshake_response().unwrap().client_flag
            & CapabilityFlags::ClientProtocol41 as u32
            != 0
        {
            status_flags = Some({
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(2));
                offset += result.offset_increment;
                result.result as u16
            });
            warnings = Some({
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(2));
                offset += result.offset_increment;
                result.result as u16
            })
        } else if connection.get_handshake_response().unwrap().client_flag
            & CapabilityFlags::ClientTransactions as u32
            != 0
        {
            status_flags = Some({
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(2));
                offset += result.offset_increment;
                result.result as u16
            })
        }

        let info = None;
        if connection.get_handshake_response().unwrap().client_flag
            & CapabilityFlags::ClientSessionTrack as u32
            != 0
        {
            // The documentation is not clear about the condition below, how do we infer if status is not empty??
            // https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_basic_ok_packet.html
            // if (status_flags.unwrap() & ServerStatusFlags::ServerSessionStateChanged as u16) != 0 {
            //     info = {
            //         let result = StringLenEnc::from_bytes(&body[offset..].to_vec(), None);
            //         offset += result.offset_increment;
            //         Some(result.result)
            //     }
            // }
        }

        OkData {
            header,
            affected_rows,
            last_insert_id,
            status_flags,
            warnings,
            info,
            session_state_info: None,
        }
    }

    pub fn to_packet(&self, sequence: u8, client_flag: u32) -> Packet {
        let mut body: Vec<u8> = Vec::new();

        body.push(self.header);
        body.extend(IntLenEnc::encode(self.affected_rows, None));
        body.extend(IntLenEnc::encode(self.last_insert_id, None));

        if client_flag & CapabilityFlags::ClientProtocol41 as u32 != 0 {
            body.extend(self.status_flags.unwrap_or(0).to_ne_bytes());
            body.extend(self.warnings.unwrap_or(0).to_ne_bytes());
        } else if client_flag & CapabilityFlags::ClientTransactions as u32 != 0 {
            body.extend(self.status_flags.unwrap().to_le_bytes());
        }

        if client_flag & CapabilityFlags::ClientSessionTrack as u32 != 0 {
            // TODO: Add session track data
        }

        Packet {
            header: PacketHeader {
                size: body.len(),
                seq: sequence,
            },
            body,
            p_type: PacketType::Ok,
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct EofData {
    pub status_flags: Option<u16>,
    warnings: Option<u16>,
}

impl EofData {
    pub fn from_packet(packet: &Packet, connection: &Connection) -> EofData {
        assert_eq!(packet.p_type, PacketType::Eof);

        let mut offset = 1;
        let body = &packet.body;

        let warnings;
        let status_flags;

        if connection.get_handshake_response().unwrap().client_flag
            & CapabilityFlags::ClientProtocol41 as u32
            != 0
        {
            warnings = Some({
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(2));
                offset += result.offset_increment;
                result.result as u16
            });

            status_flags = Some({
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(2));
                offset += result.offset_increment;
                result.result as u16
            });

            assert_eq!(offset + 2, body.len());

            return EofData {
                status_flags,
                warnings,
            };
        }

        EofData {
            status_flags: None,
            warnings: None,
        }
    }
}
