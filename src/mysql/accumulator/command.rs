use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::result_set::ResponseAccumulator;
use crate::mysql::accumulator::{AccumulationDelta, Accumulator, CapabilityFlags};
use crate::mysql::command::{Command, MySqlCommand};
use crate::mysql::packet::Packet;
use crate::mysql::types::{Converter, IntLenEnc};
use log::debug;

#[derive(Debug, Clone, Default)]
pub struct CommandAccumulator {
    pub command: Option<Command>,
    parameter_count: Option<usize>,
    parameter_set_count: Option<usize>,
    #[allow(dead_code)]
    null_bitmap: Option<Vec<u8>>,
    new_params_bind_flag: u8,
    #[allow(dead_code)]
    parameters: Option<Vec<Param>>,
    accumulation_complete: bool,
    #[allow(dead_code)]
    parameter_values: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Default)]
struct Param {
    #[allow(dead_code)]
    param_type_and_flag: u16,
    #[allow(dead_code)]
    parameter_name: String,
}

impl Accumulator for CommandAccumulator {
    fn consume(&mut self, packet: &mut Packet, connection: &Connection) -> Phase {
        let mut offset = 0;
        let body = &packet.body;

        if *body.first().unwrap() == 0x03 {
            // COM_QUERY

            offset = 1;

            if CapabilityFlags::ClientQueryAttributes as u32
                & connection.get_handshake_response().unwrap().client_flag
                != 0
            {
                self.parameter_count = {
                    let result = IntLenEnc::from_bytes(&body[offset..].to_vec(), None);
                    offset += result.offset_increment;
                    Some(result.result as usize)
                };

                self.parameter_set_count = {
                    let result = IntLenEnc::from_bytes(&body[offset..].to_vec(), None);
                    offset += result.offset_increment;
                    Some(result.result as usize)
                };
                assert_eq!(self.parameter_set_count.unwrap(), 0x01);

                if self.parameter_count.unwrap() > 0 {
                    // FIXME: Binary decoding not implemented
                    offset += self.parameter_count.unwrap().div_ceil(8);

                    self.new_params_bind_flag = {
                        let result = IntLenEnc::from_bytes(&body[offset..].to_vec(), Some(1));
                        offset += result.offset_increment;
                        let _ = offset; // For future use
                        result.result as u8
                    };

                    assert_eq!(self.parameter_set_count.unwrap(), 0x01);
                    todo!("Implementing changes for binary protocol");
                }
            }
        }

        let command = Command::from_bytes(
            MySqlCommand::from_byte(packet.body[0]).unwrap(),
            &packet.body[offset..],
        );

        let next_phase = match command.com_code {
            MySqlCommand::ComStmtClose => Phase::Command,
            _ => Phase::PendingResponse,
        };

        self.command = Some(command);
        debug!("Command details: {:?}", self.command);
        self.accumulation_complete = true;
        next_phase
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }

    fn get_accumulation_delta(&self) -> Option<AccumulationDelta> {
        Some(AccumulationDelta {
            last_command: self.command.clone(),
            response: Some(ResponseAccumulator::default()),
            ..AccumulationDelta::default()
        })
    }
}
