pub struct DecodeResult<T> {
    pub result: T,
    pub offset_increment: usize,
}

pub trait Converter<T> {
    fn from_bytes(bytes: &Vec<u8>, length: Option<usize>) -> DecodeResult<T>;
}

pub trait FixedLengthConverter<T> {
    fn from_bytes(bytes: &[u8], length: usize) -> DecodeResult<T>;
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
}

impl Converter<u128> for IntLenEnc {
    fn from_bytes(bytes: &Vec<u8>, _length: Option<usize>) -> DecodeResult<u128> {
        match bytes[0] {
            0xFC => {
                let value: u128 = 0xFC + ((bytes[1] as u128) << 8) + ((bytes[2] as u128) << 16);
                DecodeResult {
                    result: value,
                    offset_increment: 3,
                }
            }
            0xFE => {
                let value: u128 = 0xFC
                    + ((bytes[1] as u128) << 8)
                    + ((bytes[2] as u128) << 16)
                    + ((bytes[3] as u128) << 24)
                    + ((bytes[4] as u128) << 32)
                    + ((bytes[5] as u128) << 40)
                    + ((bytes[6] as u128) << 48)
                    + ((bytes[7] as u128) << 56);
                DecodeResult {
                    result: value,
                    offset_increment: 9,
                }
            }
            0xFD => {
                let value: u128 = 0xFC
                    + ((bytes[1] as u128) << 8)
                    + ((bytes[2] as u128) << 16)
                    + ((bytes[3] as u128) << 24);
                DecodeResult {
                    result: value,
                    offset_increment: 4,
                }
            }
            _ => DecodeResult {
                result: bytes[0] as u128,
                offset_increment: 1,
            },
        }
    }
}

impl Converter<String> for StringLenEnc {
    fn from_bytes(bytes: &Vec<u8>, _length: Option<usize>) -> DecodeResult<String> {
        let length = IntLenEnc::from_bytes(bytes, None);
        let mut result: String = "".to_string();
        let offset = length.offset_increment;
        for i in offset..offset + (length.result as usize) {
            result.push(bytes[i] as char);
        }

        DecodeResult {
            offset_increment: length.offset_increment + (length.result as usize),
            result,
        }
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
            0x18, 0x73, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x20, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67,
            0x20, 0x77, 0x69, 0x74, 0x68, 0x20, 0x73, 0x70, 0x61, 0x63, 0x65, 0x73,
        ];
        let result = StringLenEnc::from_bytes(&bytes, None);
        assert_eq!(true, "sample string with spaces".eq(&result.result));
        assert_eq!(26, result.offset_increment);
    }
}
