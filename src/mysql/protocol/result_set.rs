use crate::connection::{Connection, Phase};
use crate::mysql::command::MySqlCommand;
use crate::mysql::packet::{Packet, PacketType};
use crate::mysql::protocol::{Accumulator, CapabilityFlags};
use crate::mysql::types::{Converter, IntLenEnc, StringLenEnc};
use std::rc::Rc;

#[derive(Debug, Default, Clone)]
pub struct ResultSet {
    state: State,
    metadata_follows: bool,
    columns: Vec<ColumnDefinition>,
    rows: Vec<Vec<String>>,
    row_count: usize,
    status: Option<PacketType>,
    column_count: usize,
    accumulation_complete: bool,
}

impl Accumulator for ResultSet {
    fn consume(&mut self, packet: &Packet, connection: &Connection) -> Phase {
        let mut next_phase = connection.phase.clone();

        match self.state {
            State::Initiated => {
                if connection.get_last_command().is_some()
                    && connection.get_last_command().unwrap().com_code == MySqlCommand::ComQuery
                {
                    self.metadata_follows = true;
                    if connection.get_handshake_response().unwrap().client_flag
                        & CapabilityFlags::ClientOptionalResultSetMetadata as u32
                        != 0
                    {
                        self.state = State::MetaExchange;
                    } else {
                        self.state = State::ColumnCount;
                    }
                } else {
                    self.state = State::Complete;
                }
                next_phase = self.consume(packet, connection)
            }
            State::MetaExchange => {
                self.metadata_follows = {
                    let result = IntLenEnc::from_bytes(&packet.body, Some(1));
                    result.result == 0
                };
                self.state = State::ColumnCount;
            }
            State::ColumnCount => {
                self.column_count = IntLenEnc::from_bytes(&packet.body, None).result as usize;
                self.state = State::HydrateColumns;
            }
            State::HydrateColumns => {
                self.columns.push(ColumnDefinition::from_packet(packet));
                if self.column_count == self.columns.len() {
                    self.state = State::ColumnsHydrated;
                }
            }
            State::ColumnsHydrated => {
                if !connection.get_handshake_response().unwrap().client_flag
                    & CapabilityFlags::ClientDeprecateEof as u32
                    != 0
                {
                    assert_eq!(PacketType::Eof, packet.get_packet_type());
                    self.state = State::HydrateRows;
                } else {
                    self.state = State::HydrateRows;
                    next_phase = self.consume(&packet, connection);
                }
            }
            State::HydrateRows => {
                if packet.get_packet_type() != PacketType::Other {
                    self.state = State::Complete;
                    next_phase = self.consume(packet, connection);
                }
            }
            State::Complete => {
                self.status = Some(packet.get_packet_type());
                self.accumulation_complete = true;
                next_phase = Phase::Command
            }
        }
        next_phase
    }

    fn accumulation_complete(&self) -> bool {
        true
    }
}

#[derive(Debug, Default, Clone)]
enum State {
    #[default]
    Initiated,
    MetaExchange,
    ColumnCount,
    HydrateColumns,
    ColumnsHydrated,
    HydrateRows,
    Complete,
}

#[derive(Debug, Default, Clone)]
struct ColumnDefinition {
    catalog: String,
    schema: String,
    table: String,
    org_table: String,
    name: String,
    org_name: String,
    fixed_length_fields: u64,
    character_set: u8,
    // type_: FieldTypes,
    // decimals: Decimals
}

impl ColumnDefinition {
    fn from_packet(packet: &Packet) -> ColumnDefinition {
        let body = &packet.body;
        let mut offset = 0;

        ColumnDefinition {
            catalog: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec(), None);
                assert_eq!("def", result.result);
                offset += result.offset_increment;
                result.result
            },
            schema: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec(), None);
                offset += result.offset_increment;
                result.result
            },
            table: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec(), None);
                offset += result.offset_increment;
                result.result
            },
            org_table: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec(), None);
                offset += result.offset_increment;
                result.result
            },
            name: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec(), None);
                offset += result.offset_increment;
                result.result
            },
            org_name: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec(), None);
                offset += result.offset_increment;
                result.result
            },
            fixed_length_fields: {
                let result = IntLenEnc::from_bytes(&body[offset..].to_vec(), None);
                offset += result.offset_increment;
                result.result
            },
            character_set: 0x00, // TODO
        }
    }
}

#[derive(Debug)]
enum Decimals {}

#[derive(Debug)]
enum FieldTypes {}

#[cfg(test)]
mod tests {
    use crate::connection::Phase;
    use crate::mysql::protocol::result_set::*;
    use std::cell::RefCell;

    #[test]
    fn test_column_definition_decode() {
        let packet: Packet = Packet::from_bytes(
            &[
                0x49u8, 0x00u8, 0x00u8, 0x02u8, 0x03u8, 0x64u8, 0x65u8, 0x66u8, 0x0cu8, 0x73u8,
                0x77u8, 0x69u8, 0x74u8, 0x63u8, 0x68u8, 0x72u8, 0x6fu8, 0x75u8, 0x74u8, 0x65u8,
                0x72u8, 0x0cu8, 0x74u8, 0x78u8, 0x6eu8, 0x70u8, 0x61u8, 0x72u8, 0x74u8, 0x69u8,
                0x63u8, 0x69u8, 0x30u8, 0x5fu8, 0x10u8, 0x74u8, 0x78u8, 0x6eu8, 0x5fu8, 0x70u8,
                0x61u8, 0x72u8, 0x74u8, 0x69u8, 0x63u8, 0x69u8, 0x70u8, 0x61u8, 0x6eu8, 0x74u8,
                0x73u8, 0x09u8, 0x69u8, 0x64u8, 0x31u8, 0x5fu8, 0x38u8, 0x35u8, 0x5fu8, 0x30u8,
                0x5fu8, 0x02u8, 0x69u8, 0x64u8, 0x0cu8, 0x3fu8, 0x00u8, 0x14u8, 0x00u8, 0x00u8,
                0x00u8, 0x08u8, 0x03u8, 0x42u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8,
            ],
            Phase::PendingResponse,
        )
        .unwrap();
        println!("{:?}", ColumnDefinition::from_packet(&packet));
    }
}
