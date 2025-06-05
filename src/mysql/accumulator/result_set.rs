use crate::connection::{Connection, Phase};
use crate::materialization::evaluator::{Parse, ParseResult, Parser};
use crate::materialization::StateDifference;
use crate::mysql::accumulator::{AccumulationDelta, Accumulator, CapabilityFlags};
use crate::mysql::command::MySqlCommand;
use crate::mysql::packet::{
    EofData, ErrorData, OkData, Packet, PacketHeader, PacketType, ServerStatusFlags,
};
use crate::mysql::types::{Converter, IntFixedLen, IntLenEnc, StringLenEnc};
use sqlparser::ast::Statement;
use sqlparser::dialect::MySqlDialect;
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct ResponseAccumulator {
    state: State,
    metadata_follows: bool,
    columns: Vec<ColumnDefinition>,
    #[allow(dead_code)]
    rows: Vec<Vec<String>>,
    #[allow(dead_code)]
    row_count: usize,
    status: Option<PacketType>,
    column_count: usize,
    accumulation_complete: bool,
    error: Option<ErrorData>,
    parsed_sql: Option<Vec<Statement>>,
    skipped_packets: usize,
}

impl Accumulator for ResponseAccumulator {
    fn consume(&mut self, packet: &mut Packet, connection: &Connection) -> Phase {
        let mut next_phase = connection.phase.clone();

        if connection.get_last_command().is_none() {
            panic!("Attempt to populate Result")
        }

        if packet.p_type == PacketType::Error {
            self.state = State::Complete;
            self.error = Some(ErrorData::from_packet(packet, connection));
        }

        if packet.p_type == PacketType::Ok {
            let ok_data = OkData::from_packet(packet, connection);
            self.state = State::Complete;
            println!("{:?}", ok_data);
        }

        match self.state {
            State::Initiated => {
                if connection.get_last_command().unwrap().com_code == MySqlCommand::ComQuery {
                    if connection.get_handshake_response().unwrap().client_flag
                        & CapabilityFlags::ClientOptionalResultSetMetadata as u32
                        != 0
                    {
                        self.state = State::MetaExchange;
                    } else {
                        self.state = State::ColumnCount;
                    }
                } else if connection.get_last_command().unwrap().com_code
                    == MySqlCommand::ComFieldList
                {
                    self.state = State::HydrateColumns;
                } else {
                    self.state = State::Complete;
                }

                self.parsed_sql = sqlparser::parser::Parser::parse_sql(
                    &MySqlDialect {},
                    &connection.get_last_command().unwrap().arg,
                )
                .ok();

                next_phase = self.consume(packet, connection)
            }
            State::MetaExchange => {
                self.metadata_follows = {
                    let result = IntLenEnc::from_bytes(&packet.body, Some(1));
                    result.result == 1
                };
                self.state = State::ColumnCount;
            }
            State::ColumnCount => {
                self.column_count = IntLenEnc::from_bytes(&packet.body, None).result as usize;
                self.state = State::ColumnsHydrated;
                if (connection.get_handshake_response().unwrap().client_flag
                    & CapabilityFlags::ClientOptionalResultSetMetadata as u32
                    == 0)
                    || self.metadata_follows
                {
                    self.state = State::HydrateColumns;
                }
            }
            State::HydrateColumns => {
                if connection.get_last_command().unwrap().com_code == MySqlCommand::ComFieldList
                    && packet.p_type == PacketType::Eof
                {
                    self.state = State::ColumnsHydrated;
                    next_phase = self.consume(packet, connection);
                } else {
                    self.columns.push(ColumnDefinition::from_packet(packet));
                    if self.column_count == self.columns.len() {
                        self.state = State::ColumnsHydrated;
                    }
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
                    next_phase = self.consume(packet, connection);
                }
            }
            State::HydrateRows => {
                let mut status_flags: Option<u16> = None;

                match packet.get_packet_type() {
                    PacketType::Ok => {
                        status_flags = OkData::from_packet(packet, connection).status_flags
                    }
                    PacketType::Eof => {
                        status_flags = EofData::from_packet(packet, connection).status_flags
                    }
                    PacketType::Other => {
                        if let Some(diff) = &connection
                            .diff
                            .get(&self.columns.first().unwrap().org_table)
                        {
                            self.override_row(packet, diff);
                        }
                    }
                    _ => {
                        panic!("Unexpected packet type")
                    }
                }
                packet.header.seq -= self.skipped_packets as u8;

                if packet.p_type == PacketType::Error || status_flags.is_some() {
                    // No further data in this result set
                    self.state = State::Complete;

                    // Query returns multiple Result sets
                    if status_flags.is_some()
                        && status_flags.unwrap() & ServerStatusFlags::ServerMoreResultsExist as u16
                            != 0
                    {
                        self.state = State::Initiated;
                    }

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

    fn get_accumulation_delta(&self) -> Option<AccumulationDelta> {
        Some(AccumulationDelta {
            response: Some(self.clone()), // yuck
            ..AccumulationDelta::default()
        })
    }
}

impl ResponseAccumulator {
    fn parse_row(&self, packet: &Packet) -> HashMap<String, Option<String>> {
        let mut row = HashMap::new();

        let mut i = 0;
        let mut column_index = 0;
        let bytes = packet.body.as_slice();

        while i < packet.body.len() {
            if bytes[i] == 0xfb {
                row.insert(
                    self.columns.get(column_index).unwrap().org_name.clone(),
                    None,
                );
                i += 1;
            } else {
                let field = StringLenEnc::from_bytes(&bytes[i..].to_vec(), None);
                row.insert(
                    self.columns.get(column_index).unwrap().org_name.clone(),
                    Some(field.result),
                );
                i += field.offset_increment;
            }

            column_index += 1;
        }

        row
    }

    fn override_row(&mut self, packet: &mut Packet, diff: &StateDifference) {
        let mut row = self.parse_row(packet);
        let mut new_body: Vec<u8> = Vec::new();
        let mut override_state = None;

        for state_changes in diff {
            if state_changes.0.is_none() {
                override_state = Some(&state_changes.1)
            } else if let Ok(ParseResult::Boolean(true)) =
                Parse::evaluate(&row, &Box::new(state_changes.0.clone().unwrap()))
            {
                override_state = Some(&state_changes.1)
            }
        }

        for i in 0..row.len() {
            let column_name = &self.columns.get(i).unwrap().org_name;
            let mut value = row.get(column_name).unwrap();

            if override_state.is_some() && override_state.unwrap().contains_key(column_name) {
                value = override_state.unwrap().get(column_name).unwrap();
                // Updating original hashmap to decide if row needs to be omitted in select queries based on new state.
                row.insert(column_name.clone(), value.clone());
            }

            new_body.extend(match value {
                None => vec![0xfbu8],
                Some(value) => StringLenEnc::encode(value.clone(), None),
            })
        }

        if let Some(statements) = &self.parsed_sql {
            if let Some(Statement::Query(query_box)) = statements.last() {
                let query = query_box.body.as_select();

                if let Some(query) = query {
                    if query.selection.is_some() {
                        if let Ok(ParseResult::Boolean(b)) =
                            Parse::evaluate(&row, &Box::new(query.clone().selection.unwrap()))
                        {
                            if !b {
                                self.skipped_packets += 1;
                                packet.skip = true;
                            }
                        }
                    }
                }
            }
        }

        packet.header = PacketHeader {
            size: new_body.len(),
            seq: packet.header.seq, // Will be decremented by caller based on `self.skipped_packets`
        };
        packet.body = new_body;
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
#[allow(dead_code)]
struct ColumnDefinition {
    catalog: String,
    schema: String,
    table: String,
    org_table: String,
    name: String,
    org_name: String,
    fixed_length_fields: u64,
    character_set: u16,
    column_length: u32,
    field_type: FieldTypes,
    flags: u16,
    decimals: u8,
    reserved: u16,
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
            character_set: {
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(2));
                offset += result.offset_increment;
                result.result as u16
            },
            column_length: {
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(4));
                offset += result.offset_increment;
                result.result as u32
            },
            field_type: {
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(1));
                offset += result.offset_increment;
                FieldTypes::try_from(result.result as u16)
                    .expect("Invalid column type encountered!")
            },
            flags: {
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(2));
                offset += result.offset_increment;
                result.result as u16
            },
            decimals: {
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(1));
                offset += result.offset_increment;
                result.result as u8
            },
            reserved: {
                let result = IntFixedLen::from_bytes(&body[offset..].to_vec(), Some(2));
                offset += result.offset_increment;
                // assert_eq!(offset, body.len());
                result.result as u16
            },
        }
    }
}

#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
#[repr(u8)]
enum FieldTypes {
    MysqlTypeDecimal,
    MysqlTypeTiny,
    MysqlTypeShort,
    MysqlTypeLong,
    MysqlTypeFloat,
    MysqlTypeDouble,
    MysqlTypeNull,
    MysqlTypeTimestamp,
    MysqlTypeLongLong,
    MysqlTypeInt24,
    MysqlTypeDate,
    MysqlTypeTime,
    MysqlTypeDatetime,
    MysqlTypeYear,
    MysqlTypeNewDate,
    MysqlTypeVarchar,
    MysqlTypeBit,
    MysqlTypeTimestamp2,
    MysqlTypeDatetime2,
    MysqlTypeTime2,
    MysqlTypeTypedArray,
    MysqlTypeVector = 242,
    MysqlTypeInvalid = 243,
    MysqlTypeBool = 244,
    MysqlTypeJson = 245,
    MysqlTypeNewDecimal = 246,
    MysqlTypeEnum = 247,
    MysqlTypeSet = 248,
    MysqlTypeTinyBlob = 249,
    MysqlTypeMediumBlob = 250,
    MysqlTypeLongBlob = 251,
    MysqlTypeBlob = 252,
    MysqlTypeVarString = 253,
    #[default]
    MysqlTypeString = 254,
    MysqlTypeGeometry = 255,
}

impl TryFrom<u16> for FieldTypes {
    type Error = String;

    fn try_from(value: u16) -> Result<FieldTypes, Self::Error> {
        match value {
            0 => Ok(FieldTypes::MysqlTypeDecimal),
            1 => Ok(FieldTypes::MysqlTypeTiny),
            2 => Ok(FieldTypes::MysqlTypeShort),
            3 => Ok(FieldTypes::MysqlTypeLong),
            4 => Ok(FieldTypes::MysqlTypeFloat),
            5 => Ok(FieldTypes::MysqlTypeDouble),
            6 => Ok(FieldTypes::MysqlTypeNull),
            7 => Ok(FieldTypes::MysqlTypeTimestamp),
            8 => Ok(FieldTypes::MysqlTypeLongLong),
            9 => Ok(FieldTypes::MysqlTypeInt24),
            10 => Ok(FieldTypes::MysqlTypeDate),
            11 => Ok(FieldTypes::MysqlTypeTime),
            12 => Ok(FieldTypes::MysqlTypeDatetime),
            13 => Ok(FieldTypes::MysqlTypeYear),
            14 => Ok(FieldTypes::MysqlTypeNewDate),
            15 => Ok(FieldTypes::MysqlTypeVarchar),
            16 => Ok(FieldTypes::MysqlTypeBit),
            17 => Ok(FieldTypes::MysqlTypeTimestamp2),
            18 => Ok(FieldTypes::MysqlTypeDatetime2),
            19 => Ok(FieldTypes::MysqlTypeTime2),
            20 => Ok(FieldTypes::MysqlTypeTypedArray),
            242 => Ok(FieldTypes::MysqlTypeVector),
            243 => Ok(FieldTypes::MysqlTypeInvalid),
            244 => Ok(FieldTypes::MysqlTypeBool),
            245 => Ok(FieldTypes::MysqlTypeJson),
            246 => Ok(FieldTypes::MysqlTypeNewDecimal),
            247 => Ok(FieldTypes::MysqlTypeEnum),
            248 => Ok(FieldTypes::MysqlTypeSet),
            249 => Ok(FieldTypes::MysqlTypeTinyBlob),
            250 => Ok(FieldTypes::MysqlTypeMediumBlob),
            251 => Ok(FieldTypes::MysqlTypeLongBlob),
            252 => Ok(FieldTypes::MysqlTypeBlob),
            253 => Ok(FieldTypes::MysqlTypeVarString),
            254 => Ok(FieldTypes::MysqlTypeString),
            255 => Ok(FieldTypes::MysqlTypeGeometry),
            _ => Err(format!("Invalid MySQL type value: {}", value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::Phase;
    use crate::mysql::accumulator::result_set::*;

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
        let c_def = ColumnDefinition::from_packet(&packet);
        println!("{:?}", c_def);
    }
}
