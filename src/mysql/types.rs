use std::collections::VecDeque;

pub struct DecodeResult<T> {
    pub result: T,
    pub offset_increment: usize,
}

pub trait Converter<T> {
    fn from_bytes(bytes: &Vec<u8>, length: Option<usize>) -> DecodeResult<T>;

    fn encode(_value: T, _length: Option<usize>) -> Vec<u8> {
        todo!("Not implemented")
    }
}

pub trait FixedLengthConverter<T> {
    fn from_bytes(bytes: &[u8], length: usize) -> DecodeResult<T>;
    // TODO: Change implementations to use FixedLength Converter
}

pub struct IntLenEnc {}
pub struct IntFixedLen {}
pub struct StringLenEnc {}
pub struct StringNullEnc {}
pub struct StringFixedLen {}

pub struct StringEOFEnc {}

impl Converter<u64> for IntFixedLen {
    fn from_bytes(bytes: &Vec<u8>, length: Option<usize>) -> DecodeResult<u64> {
        if length.is_none() {
            panic!("IntFixedLen length should be greater than 0");
        }

        let mut buffer = [0u8; 8];
        let slice = &bytes[0..length.unwrap()];
        buffer[..slice.len()].copy_from_slice(slice);

        DecodeResult {
            result: u64::from_le_bytes(buffer),
            offset_increment: length.unwrap(),
        }
    }

    fn encode(value: u64, length: Option<usize>) -> Vec<u8> {
        let mut buffer = VecDeque::with_capacity(length.unwrap());
        let bytes = &value.to_le_bytes();

        for byte in bytes[0..length.unwrap()].iter() {
            buffer.push_back(*byte);
        }

        buffer.into()
    }
}

impl Converter<u64> for IntLenEnc {
    fn from_bytes(bytes: &Vec<u8>, length: Option<usize>) -> DecodeResult<u64> {
        if length.is_some() {
            panic!("IntLenEnc length should not be called with length parameter!");
        }

        match bytes[0] {
            0xFC => {
                let value: u64 = (bytes[1] as u64) + ((bytes[2] as u64) << 8);
                DecodeResult {
                    result: value,
                    offset_increment: 3,
                }
            }
            0xFD => {
                let value: u64 =
                    (bytes[1] as u64) + ((bytes[2] as u64) << 8) + ((bytes[3] as u64) << 16);
                DecodeResult {
                    result: value,
                    offset_increment: 4,
                }
            }
            0xFE => {
                let value: u64 = (bytes[1] as u64)
                    + ((bytes[2] as u64) << 8)
                    + ((bytes[3] as u64) << 16)
                    + ((bytes[4] as u64) << 24)
                    + ((bytes[5] as u64) << 32)
                    + ((bytes[6] as u64) << 40)
                    + ((bytes[7] as u64) << 48)
                    + ((bytes[8] as u64) << 56);
                DecodeResult {
                    result: value,
                    offset_increment: 9,
                }
            }
            _ => DecodeResult {
                result: bytes[0] as u64,
                offset_increment: 1,
            },
        }
    }

    fn encode(value: u64, length: Option<usize>) -> Vec<u8> {
        if length.is_some() {
            panic!("IntLenEnc length should not be called with length parameter!");
        }

        if value < 251 {
            // 1-byte encoding
            vec![value as u8]
        } else if value <= 0xFFFF {
            // 2-byte encoding (0xFC followed by 2 bytes)
            let mut bytes = vec![0xFC];
            bytes.extend_from_slice(&(value as u16).to_le_bytes());
            bytes
        } else if value <= 0xFFFFFF {
            // 3-byte encoding (0xFD followed by 3 bytes)
            let mut bytes = vec![0xFD];
            bytes.extend_from_slice(&(value as u32).to_le_bytes()[..3]);
            bytes
        } else {
            // 8-byte encoding (0xFE followed by 8 bytes)
            let mut bytes = vec![0xFE];
            bytes.extend_from_slice(&(value as u64).to_le_bytes());
            bytes
        }
    }
}

impl Converter<String> for StringLenEnc {
    fn from_bytes(bytes: &Vec<u8>, _length: Option<usize>) -> DecodeResult<String> {
        let length = IntLenEnc::from_bytes(bytes, None);
        let mut result: String = "".to_string();
        let offset = length.offset_increment;
        for byte in bytes[offset..offset + (length.result as usize)].iter() {
            result.push(*byte as char);
        }

        DecodeResult {
            offset_increment: length.offset_increment + (length.result as usize),
            result,
        }
    }

    fn encode(value: String, _length: Option<usize>) -> Vec<u8> {
        let mut out = vec![];

        out.append(&mut IntLenEnc::encode(value.len() as u64, None));
        out.append(&mut value.as_bytes().to_vec());
        out
    }
}

impl Converter<String> for StringNullEnc {
    fn from_bytes(bytes: &Vec<u8>, _length: Option<usize>) -> DecodeResult<String> {
        let mut null_position: Option<usize> = None;

        for i in 0..bytes.len() {
            if bytes[i] == 0x00 {
                null_position = Some(i);
                break;
            }
        }

        if null_position.is_none() {
            panic!("Could not find null byte!");
        }

        DecodeResult {
            result: String::from_utf8(bytes[0..null_position.unwrap()].to_vec()).unwrap(),
            offset_increment: null_position.unwrap() + 1,
        }
    }
}

impl Converter<String> for StringEOFEnc {
    fn from_bytes(bytes: &Vec<u8>, _length: Option<usize>) -> DecodeResult<String> {
        DecodeResult {
            result: String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| {
                println!("{:?}", bytes);
                panic!("Could not convert bytes to string!");
            }),
            offset_increment: bytes.len(),
        }
    }
}

impl Converter<String> for StringFixedLen {
    fn from_bytes(bytes: &Vec<u8>, length: Option<usize>) -> DecodeResult<String> {
        if length.is_none() {
            panic!("StringFixedLen requires length parameter!");
        }

        DecodeResult {
            result: String::from_utf8(bytes[0..length.unwrap()].to_vec()).unwrap(),
            offset_increment: length.unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_len_enc() {
        let bytes: Vec<u8> = vec![
            0x19, 0x73, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x20, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67,
            0x20, 0x77, 0x69, 0x74, 0x68, 0x20, 0x73, 0x70, 0x61, 0x63, 0x65, 0x73,
        ];
        let result = StringLenEnc::from_bytes(&bytes, None);
        assert!("sample string with spaces".eq(&result.result));
        assert_eq!(26, result.offset_increment);
    }

    #[test]
    fn test_int_len_enc() {
        let bytes: Vec<u8> = vec![0xfe, 0x3c, 0x58, 0xd7, 0xfa, 0xc2, 0x05, 0x00, 0x00];
        assert_eq!(6334990211132, IntLenEnc::from_bytes(&bytes, None).result);
    }

    #[test]
    fn test_encode() {
        let result = IntLenEnc::encode(6334990211130, None);
        // fe  3a  58  d7  fa  c2  05  00  00
        assert_eq!(
            vec![0xfe, 0x3a, 0x58, 0xd7, 0xfa, 0xc2, 0x05, 0x00, 0x00],
            result
        );
    }
}
