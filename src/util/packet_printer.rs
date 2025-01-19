use crate::mysql::packet::Packet;

pub fn print_packet(packet: &Packet) {
    // Print bounds
    println!("----------------------------------------------------------------------------------");
    println!("  0   1   2   3   4   5   6   7   8   9   a   b   c   d   e   f");
    println!("----------------------------------------------------------------------------------");

    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&packet.header.to_bytes());
    bytes.extend_from_slice(&packet.body);

    let mut offset = 0;
    let mut text_buf: String = String::new();

    while offset < bytes.len() {
        let byte = bytes.get(offset).unwrap();
        print!(" {:02x} ", byte);

        match byte.is_ascii_graphic() {
            true => text_buf.push(*byte as char),
            false => text_buf.push('.'),
        }

        offset += 1;

        if offset % 16 == 0 || offset == bytes.len() {
            if offset == bytes.len() {
                while offset % 16 != 0 {
                    print!("    ");
                    offset += 1;
                }
            }

            print!(" | {} \n", text_buf);
            text_buf = "".to_owned();
        }
    }
    print!("\n");
}
