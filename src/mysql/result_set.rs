use crate::mysql::packet::Packet;
use crate::mysql::types::{Converter, IntLenEnc, StringLenEnc};

#[derive(Debug, Default)]
pub struct ResultSet {
    state: State,
    columns: Vec<ColumnDefinition>,
    rows: Vec<Vec<String>>,
    row_count: usize,
    column_count: usize,
}

impl ResultSet {
    pub fn consume(self: &mut Self, packet: &Packet) -> &mut Self {
        match self.state {
            State::Initiated => {
                self.column_count =
                    crate::mysql::types::IntLenEnc::from_bytes(&packet.body).result as usize;
                self.state = State::HydrateColumns
            }
            State::HydrateColumns => {}
            State::HydrateRows => {}
            State::Complete => {}
        }

        self
    }
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Initiated,
    HydrateColumns,
    HydrateRows,
    Complete,
}

#[derive(Debug, Default)]
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
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec());
                assert_eq!("def", result.result);
                offset += result.offset_increment;
                result.result
            },
            schema: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec());
                offset += result.offset_increment;
                result.result
            },
            table: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec());
                offset += result.offset_increment;
                result.result
            },
            org_table: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec());
                offset += result.offset_increment;
                result.result
            },
            name: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec());
                offset += result.offset_increment;
                result.result
            },
            org_name: {
                let result = StringLenEnc::from_bytes(&body[offset..].to_vec());
                offset += result.offset_increment;
                result.result
            },
            fixed_length_fields: {
                let result = IntLenEnc::from_bytes(&body[offset..].to_vec());
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
    use crate::mysql::result_set::*;

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
