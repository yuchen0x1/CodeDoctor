#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_and_utf16_with_bom() {
        assert_eq!(decode_bytes(b"hello\n").unwrap().encoding, "utf-8");
        let mut utf16 = vec![0xFF, 0xFE];
        for unit in "中文".encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        let decoded = decode_bytes(&utf16).unwrap();
        assert_eq!(decoded.encoding, "utf-16le");
        assert_eq!(decoded.text, "中文");
    }

    #[test]
    fn binary_content_is_excluded() {
        let decoded = decode_bytes(&[0, 1, 2, 0, 255]).unwrap();
        assert!(decoded.is_binary);
    }
}
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedText {
    pub encoding: String,
    pub text: String,
    pub is_binary: bool,
}

pub fn decode_bytes(bytes: &[u8]) -> Result<DecodedText, AppError> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return decode_utf8(&bytes[3..], "utf-8");
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Ok(DecodedText {
            encoding: "utf-16le".to_owned(),
            text: decode_utf16(&bytes[2..], true),
            is_binary: false,
        });
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Ok(DecodedText {
            encoding: "utf-16be".to_owned(),
            text: decode_utf16(&bytes[2..], false),
            is_binary: false,
        });
    }
    match std::str::from_utf8(bytes) {
        Ok(text) if !looks_binary(text.as_bytes()) => Ok(DecodedText {
            encoding: "utf-8".to_owned(),
            text: text.to_owned(),
            is_binary: false,
        }),
        Ok(_) => Ok(DecodedText {
            encoding: "binary".to_owned(),
            text: String::new(),
            is_binary: true,
        }),
        Err(_) => Ok(DecodedText {
            encoding: "binary".to_owned(),
            text: String::new(),
            is_binary: true,
        }),
    }
}

fn decode_utf8(bytes: &[u8], encoding: &str) -> Result<DecodedText, AppError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| AppError::Parser("text encoding is invalid".to_owned()))?;
    Ok(DecodedText {
        encoding: encoding.to_owned(),
        text: text.to_owned(),
        is_binary: false,
    })
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        units.push(if little_endian {
            u16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], chunk[1]])
        });
    }
    String::from_utf16_lossy(&units)
}

fn looks_binary(bytes: &[u8]) -> bool {
    if bytes.contains(&0) {
        return true;
    }
    let controls = bytes
        .iter()
        .filter(|byte| matches!(**byte, 0x01..=0x08 | 0x0B..=0x0C | 0x0E..=0x1F))
        .count();
    controls > bytes.len().saturating_div(20).max(1)
}
