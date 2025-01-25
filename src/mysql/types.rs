pub struct DecodeResult <T> {
    pub result: T,
    pub offset_increment: usize,
}

pub trait Converter <T> {
    fn from_bytes(bytes: &Vec<u8>) -> DecodeResult<T>;

    // fn to_bytes(&self) -> Vec<u8>;
}

pub struct IntLenEnc {}
impl Converter<u64> for IntLenEnc {
    fn from_bytes(bytes: &Vec<u8>) -> DecodeResult<u64> {
        match bytes[0] {
            0xFC => {
                let value: u64 = 0xFC + ((bytes[1] as u64) << 8) + ((bytes[2] as u64) << 16);
                DecodeResult {
                    result: value,
                    offset_increment: 3,
                }
            }
            0xFE => {
                let value: u64 = 0xFC
                    + ((bytes[1] as u64) << 8)
                    + ((bytes[2] as u64) << 16)
                    + ((bytes[3] as u64) << 24)
                    + ((bytes[4] as u64) << 32)
                    + ((bytes[5] as u64) << 40)
                    + ((bytes[6] as u64) << 48)
                    + ((bytes[7] as u64) << 56);
                DecodeResult {
                    result: value,
                    offset_increment: 9,
                }
            }
            0xFD => {
                let value: u64 = 0xFC
                    + ((bytes[1] as u64) << 8)
                    + ((bytes[2] as u64) << 16)
                    + ((bytes[3] as u64) << 24);
                DecodeResult {
                    result: value,
                    offset_increment: 4,
                }
            }
            _ => DecodeResult {
                result: bytes[0] as u64,
                offset_increment: 1,
            },
        }
    }
}


pub struct StringLenEnc {}

impl Converter<String> for StringLenEnc {
    fn from_bytes(bytes: &Vec<u8>) -> DecodeResult<String> {
        let length = IntLenEnc::from_bytes(bytes);
        let mut result: String = "".to_string();
        let offset = length.offset_increment;
        for i in offset..offset + length.result as usize {
            result.push(bytes[i] as char);
        }

        DecodeResult {
            offset_increment: length.offset_increment + result.len(),
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_len_enc() {
      let bytes: Vec<u8> = vec![0x18, 0x73, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x20, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67, 0x20, 0x77, 0x69, 0x74, 0x68, 0x20, 0x73, 0x70, 0x61, 0x63, 0x65, 0x73];
        let result = StringLenEnc::from_bytes(&bytes);
        assert_eq!(true, "sample string with spaces".eq(&result.result));
        assert_eq!(26, result.offset_increment);
    }
}