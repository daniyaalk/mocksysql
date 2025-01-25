use crate::mysql::packet::Packet;

#[derive(Default, Debug)]
pub struct ResultSet {
    columns: Vec<String>,
    rows: Vec<Vec<String>>
}


impl ResultSet {
    pub fn from_packets(packets: Vec<Packet>) -> ResultSet {
        ResultSet::default()
    }
}