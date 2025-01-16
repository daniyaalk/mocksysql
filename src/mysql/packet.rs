pub struct Packet {
    header: [u8; 4],
    body: Box<[u8]>
}

impl Packet {

    pub fn from_bytes(bytes: &[u8]) -> Packet {

        let header: [u8; 4] = bytes[0 .. 4].try_into().expect("Slice with incorrect length");
        let body = bytes[4 .. ].to_vec().into_boxed_slice();

        Packet { header: header, body: body }

    }

}
