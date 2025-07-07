use crate::mysql::packet::Packet;
use log::{debug, log_enabled};

pub fn print_packet(packet: &Packet) {
    if !log_enabled!(log::Level::Debug) {
        return;
    }

    let mut print_buffer = String::new();
    // Print bounds
    print_buffer.push_str(
        "----------------------------------------------------------------------------------\n",
    );
    print_buffer.push_str("  0   1   2   3   4   5   6   7   8   9   a   b   c   d   e   f\n");
    print_buffer.push_str(
        "----------------------------------------------------------------------------------\n",
    );

    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&packet.header.to_bytes());
    bytes.extend_from_slice(&packet.body);

    let mut offset = 0;
    let mut row_buffer: String = String::new();

    while offset < bytes.len() {
        let byte = bytes.get(offset).unwrap();
        print_buffer.push_str(format!(" {:02x} ", byte).as_str());

        match byte.is_ascii_graphic() {
            true => row_buffer.push(*byte as char),
            false => row_buffer.push('.'),
        }

        offset += 1;

        if offset % 16 == 0 || offset == bytes.len() {
            if offset == bytes.len() {
                while offset % 16 != 0 {
                    print_buffer.push_str("    ");
                    offset += 1;
                }
            }

            print_buffer.push_str(format!(" | {} \n", row_buffer).as_str());
            row_buffer = "".to_string();
        }
    }
    print_buffer.push('\n');
    print_buffer.push_str(format!("Packet type: {:?} \n", packet.p_type).as_str());
    debug!("{}", print_buffer);
}
