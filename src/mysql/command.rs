#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Command {
    pub com_code: MySqlCommand,
    pub arg: String,
}

impl Command {
    pub fn from_bytes(bytes: &[u8]) -> Command {
        let com_code = MySqlCommand::from_byte(bytes[0]).unwrap();
        let arg =
            String::from_utf8(bytes[1..].to_vec()).expect("Unable to convert bytes to string");
        Command { com_code, arg: arg }
    }
}

#[allow(dead_code, clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MySqlCommand {
    ComSleep = 0x00,
    ComQuit = 0x01,
    ComInitDb = 0x02,
    ComQuery = 0x03,
    ComFieldList = 0x04,
    ComCreateDb = 0x05,
    ComDropDb = 0x06,
    ComRefresh = 0x07,
    ComShutdown = 0x08,
    ComStatistics = 0x09,
    ComProcessInfo = 0x0A,
    ComConnect = 0x0B,
    ComProcessKill = 0x0C,
    ComDebug = 0x0D,
    ComPing = 0x0E,
    ComTime = 0x0F,
    ComDelayedInsert = 0x10,
    ComChangeUser = 0x11,
    ComBinlogDump = 0x12,
    ComTableDump = 0x13,
    ComConnectOut = 0x14,
    ComRegisterSlave = 0x15,
    ComStmtPrepare = 0x16,
    ComStmtExecute = 0x17,
    ComStmtSendLongData = 0x18,
    ComStmtClose = 0x19,
    ComStmtReset = 0x1A,
    ComSetOption = 0x1B,
    ComStmtFetch = 0x1C,
    ComDaemon = 0x1D,
    ComBinlogDumpGtid = 0x1E,
    ComResetConnection = 0x1F,
}

#[allow(dead_code)]
impl MySqlCommand {
    /// Try to convert a u8 value to a MySqlCommand
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(MySqlCommand::ComSleep),
            0x01 => Some(MySqlCommand::ComQuit),
            0x02 => Some(MySqlCommand::ComInitDb),
            0x03 => Some(MySqlCommand::ComQuery),
            0x04 => Some(MySqlCommand::ComFieldList),
            0x05 => Some(MySqlCommand::ComCreateDb),
            0x06 => Some(MySqlCommand::ComDropDb),
            0x07 => Some(MySqlCommand::ComRefresh),
            0x08 => Some(MySqlCommand::ComShutdown),
            0x09 => Some(MySqlCommand::ComStatistics),
            0x0A => Some(MySqlCommand::ComProcessInfo),
            0x0B => Some(MySqlCommand::ComConnect),
            0x0C => Some(MySqlCommand::ComProcessKill),
            0x0D => Some(MySqlCommand::ComDebug),
            0x0E => Some(MySqlCommand::ComPing),
            0x0F => Some(MySqlCommand::ComTime),
            0x10 => Some(MySqlCommand::ComDelayedInsert),
            0x11 => Some(MySqlCommand::ComChangeUser),
            0x12 => Some(MySqlCommand::ComBinlogDump),
            0x13 => Some(MySqlCommand::ComTableDump),
            0x14 => Some(MySqlCommand::ComConnectOut),
            0x15 => Some(MySqlCommand::ComRegisterSlave),
            0x16 => Some(MySqlCommand::ComStmtPrepare),
            0x17 => Some(MySqlCommand::ComStmtExecute),
            0x18 => Some(MySqlCommand::ComStmtSendLongData),
            0x19 => Some(MySqlCommand::ComStmtClose),
            0x1A => Some(MySqlCommand::ComStmtReset),
            0x1B => Some(MySqlCommand::ComSetOption),
            0x1C => Some(MySqlCommand::ComStmtFetch),
            0x1D => Some(MySqlCommand::ComDaemon),
            0x1E => Some(MySqlCommand::ComBinlogDumpGtid),
            0x1F => Some(MySqlCommand::ComResetConnection),
            _ => None,
        }
    }
}
