//! Reading and writing the text files a mod is made of.
//!
//! Two details matter for Hearts of Iron IV. Localisation `.yml` files are only
//! loaded by the game when they carry a UTF-8 BOM, and older script files are
//! sometimes Windows-1252 rather than UTF-8. Both survive a round trip: a file
//! read as Windows-1252 is written back as Windows-1252, so saving one field
//! does not rewrite every non-ASCII byte in the file.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Windows-1252 differs from Latin-1 only in the 0x80..=0x9F range.
/// `U+FFFD` marks the byte values the encoding leaves undefined.
const HIGH: [char; 32] = [
    '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}', '\u{017D}', '\u{FFFD}',
    '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Encoding {
    Utf8,
    Windows1252,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFile {
    pub path: String,
    pub content: String,
    /// The file started with a UTF-8 byte order mark.
    pub has_bom: bool,
    pub encoding: Encoding,
    pub size_bytes: u64,
}

pub fn read(path: impl AsRef<Path>) -> AppResult<TextFile> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| AppError::io(path, source))?;
    let size_bytes = bytes.len() as u64;

    let has_bom = bytes.starts_with(&BOM);
    let body = if has_bom { &bytes[3..] } else { &bytes[..] };

    let (content, encoding) = match std::str::from_utf8(body) {
        Ok(text) => (text.to_string(), Encoding::Utf8),
        Err(_) => (decode_windows_1252(body), Encoding::Windows1252),
    };

    Ok(TextFile {
        path: path.display().to_string(),
        content,
        has_bom,
        encoding,
        size_bytes,
    })
}

/// Writes `content` in `encoding`, returning the encoding actually used.
///
/// Text that no longer fits Windows-1252 — a translator pasting Japanese into
/// a legacy file, say — is written as UTF-8, which the game also reads. The
/// caller gets the result back so it can say so.
pub fn write(
    path: impl AsRef<Path>,
    content: &str,
    encoding: Encoding,
    with_bom: bool,
) -> AppResult<Encoding> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }

    let (body, used) = match encoding {
        Encoding::Windows1252 => match encode_windows_1252(content) {
            Some(bytes) => (bytes, Encoding::Windows1252),
            None => (content.as_bytes().to_vec(), Encoding::Utf8),
        },
        Encoding::Utf8 => (content.as_bytes().to_vec(), Encoding::Utf8),
    };

    let mut bytes = Vec::with_capacity(body.len() + BOM.len());
    // A byte order mark only belongs in front of UTF-8.
    if with_bom && used == Encoding::Utf8 {
        bytes.extend_from_slice(&BOM);
    }
    bytes.extend_from_slice(&body);

    fs::write(path, bytes).map_err(|source| AppError::io(path, source))?;

    Ok(used)
}

fn decode_windows_1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| match byte {
            0x80..=0x9F => HIGH[(byte - 0x80) as usize],
            other => *other as char,
        })
        .collect()
}

/// Encodes to Windows-1252, or `None` when a character has no representation.
fn encode_windows_1252(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len());

    for character in text.chars() {
        let code = character as u32;
        let byte = match code {
            0x00..=0x7F | 0xA0..=0xFF => code as u8,
            _ => {
                let index = HIGH
                    .iter()
                    .position(|candidate| *candidate == character && *candidate != '\u{FFFD}')?;
                0x80 + index as u8
            }
        };
        bytes.push(byte);
    }

    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join("hoi4ms-text-file-test");
        fs::create_dir_all(&directory).expect("temp dir");
        directory.join(name)
    }

    #[test]
    fn decodes_windows_1252_high_bytes() {
        assert_eq!(decode_windows_1252(&[0x93, 0x41, 0x94]), "\u{201C}A\u{201D}");
    }

    #[test]
    fn round_trips_a_bom() {
        let path = temp_path("l_english.yml");

        write(&path, "l_english:\n key:0 \"value\"\n", Encoding::Utf8, true).expect("write");
        let file = read(&path).expect("read");

        assert!(file.has_bom);
        assert_eq!(file.encoding, Encoding::Utf8);
        assert!(file.content.starts_with("l_english:"));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn keeps_windows_1252_bytes_across_a_save() {
        let path = temp_path("legacy.txt");
        // 0xE9 is `é` in Windows-1252 and invalid on its own in UTF-8.
        fs::write(&path, b"name = \"Caf\xE9\"\n").expect("write");

        let file = read(&path).expect("read");
        assert_eq!(file.encoding, Encoding::Windows1252);
        assert!(file.content.contains("Caf\u{E9}"));

        let used = write(&path, &file.content, file.encoding, file.has_bom).expect("write back");
        assert_eq!(used, Encoding::Windows1252);

        let bytes = fs::read(&path).expect("read back");
        assert_eq!(bytes, b"name = \"Caf\xE9\"\n");

        fs::remove_file(&path).ok();
    }

    #[test]
    fn falls_back_to_utf8_when_the_text_no_longer_fits() {
        let path = temp_path("widened.txt");
        fs::write(&path, b"name = \"Caf\xE9\"\n").expect("write");

        let file = read(&path).expect("read");
        let widened = file.content.replace("Caf\u{E9}", "\u{6771}\u{4EAC}");
        let used = write(&path, &widened, file.encoding, false).expect("write back");

        assert_eq!(used, Encoding::Utf8);
        assert_eq!(read(&path).expect("read back").content, widened);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn encodes_the_windows_1252_specials() {
        let encoded = encode_windows_1252("\u{201C}quoted\u{201D}").expect("encodable");
        assert_eq!(encoded[0], 0x93);
        assert_eq!(encoded[encoded.len() - 1], 0x94);
    }
}
