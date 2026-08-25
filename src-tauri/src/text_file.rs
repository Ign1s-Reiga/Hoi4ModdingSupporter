//! Reading and writing the text files a mod is made of.
//!
//! Two details matter for Hearts of Iron IV. Localisation `.yml` files are only
//! loaded by the game when they carry a UTF-8 BOM, and older script files are
//! sometimes Windows-1252 rather than UTF-8. Both are preserved on a round trip.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFile {
    pub path: String,
    pub content: String,
    /// The file started with a UTF-8 byte order mark.
    pub has_bom: bool,
    /// The bytes were not valid UTF-8 and were decoded as Windows-1252.
    pub is_legacy_encoding: bool,
    pub size_bytes: u64,
}

pub fn read(path: impl AsRef<Path>) -> AppResult<TextFile> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| AppError::io(path, source))?;
    let size_bytes = bytes.len() as u64;

    let has_bom = bytes.starts_with(&BOM);
    let body = if has_bom { &bytes[3..] } else { &bytes[..] };

    let (content, is_legacy_encoding) = match std::str::from_utf8(body) {
        Ok(text) => (text.to_string(), false),
        Err(_) => (decode_windows_1252(body), true),
    };

    Ok(TextFile {
        path: path.display().to_string(),
        content,
        has_bom,
        is_legacy_encoding,
        size_bytes,
    })
}

pub fn write(path: impl AsRef<Path>, content: &str, with_bom: bool) -> AppResult<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }

    let mut bytes = Vec::with_capacity(content.len() + 3);
    if with_bom {
        bytes.extend_from_slice(&BOM);
    }
    bytes.extend_from_slice(content.as_bytes());

    fs::write(path, bytes).map_err(|source| AppError::io(path, source))
}

/// Windows-1252 maps 1:1 onto Unicode except for the 0x80..=0x9F range.
fn decode_windows_1252(bytes: &[u8]) -> String {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}',
        '\u{017D}', '\u{FFFD}', '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
    ];

    bytes
        .iter()
        .map(|byte| match byte {
            0x80..=0x9F => HIGH[(byte - 0x80) as usize],
            other => *other as char,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_windows_1252_high_bytes() {
        assert_eq!(decode_windows_1252(&[0x93, 0x41, 0x94]), "\u{201C}A\u{201D}");
    }

    #[test]
    fn round_trips_a_bom() {
        let dir = std::env::temp_dir().join("hoi4ms-text-file-test");
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("l_english.yml");

        write(&path, "l_english:\n key:0 \"value\"\n", true).expect("write");
        let file = read(&path).expect("read");

        assert!(file.has_bom);
        assert!(!file.is_legacy_encoding);
        assert!(file.content.starts_with("l_english:"));

        fs::remove_file(&path).ok();
    }
}
