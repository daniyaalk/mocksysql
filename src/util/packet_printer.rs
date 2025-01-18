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

    while offset < bytes.len() {
        print!(" {:02x} ", bytes.get(offset).unwrap());
        offset += 1;

        if offset % 16 == 0 || offset == bytes.len() {

            if offset == bytes.len() {
                for _ in 0 .. 16-(bytes.len()%16 ) {
                    print!("    ");
                }       
            }

            if offset >= 16 && offset-1 < bytes.len() {
                print!(" | {} \n", get_text(&bytes[offset-16 .. offset-1]));                
            }
        }

    }
    print!("\n");

}

fn get_text(bytes: &[u8]) -> String {  
    
    let mut ret: String = String::new();

    for byte in bytes {
        match byte.is_ascii_graphic() {
            true => ret.push(*byte as char),
            false => ret.push('.'),
        }
    }

    ret

}