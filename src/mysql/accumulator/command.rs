use crate::connection::{Connection, Phase};
use crate::mysql::accumulator::result_set::ResponseAccumulator;
use crate::mysql::accumulator::{AccumulationDelta, Accumulator};
use crate::mysql::command::Command;
use crate::mysql::packet::Packet;
use std::any::Any;

#[derive(Debug, Clone, Default)]
pub struct CommandAccumulator {
    pub command: Option<Command>,
    accumulation_complete: bool,
}

impl Accumulator for CommandAccumulator {
    fn consume(&mut self, packet: &Packet, _connection: &Connection) -> Phase {
        self.command = Some(Command::from_bytes(&packet.body));
        println!("Command details: {:?}", self.command);
        Phase::PendingResponse
    }

    fn accumulation_complete(&self) -> bool {
        self.accumulation_complete
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn get_accumulation_delta(&self) -> Option<AccumulationDelta> {
        Some(AccumulationDelta {
            last_command: self.command.clone(),
            response: Some(ResponseAccumulator::default()),
            ..AccumulationDelta::default()
        })
    }
}
