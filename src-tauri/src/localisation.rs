//! Hearts of Iron IV localisation files.
//!
//! The format looks like YAML but is not: a language header (`l_english:`)
//! followed by ` KEY:version "text"` lines. Files must be UTF-8 with a BOM or
//! the game silently ignores them, so a BOM is always written back.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paradox::edit::detect_newline;
use crate::text_file;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalisationEntry {
    pub key: String,
    /// The number after the colon, kept as text because it is optional.
    pub version: String,
    pub value: String,
    /// Zero-based line in the source file, `null` for an entry added in the app.
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalisationFile {
    pub path: String,
    /// Language header without the trailing colon, e.g. `l_english`.
    pub language: String,
    pub entries: Vec<LocalisationEntry>,
    pub has_bom: bool,
    pub encoding: text_file::Encoding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalisationFileInfo {
    pub path: String,
    pub relative_path: String,
    pub language: String,
    pub entry_count: usize,
    /// Missing BOM means the game will not load the file.
    pub has_bom: bool,
}

pub fn read(path: &str) -> AppResult<LocalisationFile> {
    let file = text_file::read(Path::new(path))?;
    let (language, entries) = parse(&file.content);

    Ok(LocalisationFile {
        path: path.replace('\\', "/"),
        language,
        entries,
        has_bom: file.has_bom,
        encoding: file.encoding,
    })
}

/// Rewrites the entries of a file, leaving comments and unknown lines in place.
pub fn write(path: &str, language: &str, entries: &[LocalisationEntry]) -> AppResult<LocalisationFile> {
    let target = Path::new(path);
    let existing = if target.exists() {
        text_file::read(target)?
    } else {
        text_file::TextFile {
            path: path.to_string(),
            content: String::new(),
            has_bom: true,
            encoding: text_file::Encoding::Utf8,
            size_bytes: 0,
        }
    };

    let newline = detect_newline(&existing.content);
    let mut lines: Vec<String> = existing
        .content
        .lines()
        .map(|line| line.to_string())
        .collect();

    let (existing_language, existing_entries) = parse(&existing.content);

    // Index both sides by line: the original so an untouched entry can keep the
    // exact text it had, trailing comment and spacing included, and the
    // incoming set so deletions can be spotted.
    let mut original: Vec<Option<&LocalisationEntry>> = vec![None; lines.len()];
    for entry in &existing_entries {
        if let Some(line) = entry.line {
            if line < original.len() {
                original[line] = Some(entry);
            }
        }
    }

    let mut kept: Vec<Option<&LocalisationEntry>> = vec![None; lines.len()];
    for entry in entries {
        if let Some(line) = entry.line {
            if line < kept.len() {
                kept[line] = Some(entry);
            }
        }
    }

    let mut rewritten: Vec<String> = Vec::with_capacity(lines.len() + entries.len());
    for (index, line) in lines.drain(..).enumerate() {
        match (original[index], kept[index]) {
            // An entry the user actually changed.
            (Some(before), Some(after)) if !same_entry(before, after) => {
                rewritten.push(format_entry(after));
            }
            // An entry line nobody touched: keep it byte for byte.
            (Some(_), Some(_)) => rewritten.push(line),
            // An entry the user deleted.
            (Some(_), None) => {}
            // A comment, blank line or the language header.
            (None, _) => rewritten.push(line),
        }
    }

    // Make sure the file starts with a language header.
    let header = if language.trim().is_empty() {
        existing_language
    } else {
        language.trim().trim_end_matches(':').to_string()
    };
    let header = if header.is_empty() {
        "l_english".to_string()
    } else {
        header
    };

    match rewritten.iter().position(|line| is_language_header(line)) {
        Some(index) => rewritten[index] = format!("{header}:"),
        None => rewritten.insert(0, format!("{header}:")),
    }

    for entry in entries.iter().filter(|entry| entry.line.is_none()) {
        rewritten.push(format_entry(entry));
    }

    let mut content = rewritten.join(newline);
    if !content.ends_with(newline) {
        content.push_str(newline);
    }

    // Localisation is always UTF-8 with a byte order mark, whatever the file
    // looked like before: without one the game ignores it entirely.
    text_file::write(target, &content, text_file::Encoding::Utf8, true)?;

    read(path)
}

fn parse(content: &str) -> (String, Vec<LocalisationEntry>) {
    let mut language = String::new();
    let mut entries = Vec::new();

    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if is_language_header(trimmed) {
            if language.is_empty() {
                language = trimmed.trim_end_matches(':').trim().to_string();
            }
            continue;
        }

        if let Some(entry) = parse_entry(line, index) {
            entries.push(entry);
        }
    }

    (language, entries)
}

fn is_language_header(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.to_lowercase().starts_with("l_") && trimmed.ends_with(':') && !trimmed.contains('"')
}

fn parse_entry(line: &str, index: usize) -> Option<LocalisationEntry> {
    let without_comment = strip_comment(line);
    let colon = without_comment.find(':')?;

    let key = without_comment[..colon].trim().to_string();
    if key.is_empty() || key.to_lowercase().starts_with("l_") {
        return None;
    }

    let remainder = &without_comment[colon + 1..];
    let quote = remainder.find('"')?;
    let version = remainder[..quote].trim().to_string();
    let value = remainder[quote..].trim();

    Some(LocalisationEntry {
        key,
        version,
        value: unescape(trim_quotes(value)),
        line: Some(index),
    })
}

/// Removes a trailing `#` comment that is not inside the quoted value.
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_quotes = false;

    for (index, character) in line.char_indices() {
        match character {
            '"' if index == 0 || bytes[index - 1] != b'\\' => in_quotes = !in_quotes,
            '#' if !in_quotes => return &line[..index],
            _ => {}
        }
    }

    line
}

fn trim_quotes(value: &str) -> &str {
    let start = match value.find('"') {
        Some(index) => index,
        None => return value,
    };
    match value.rfind('"') {
        Some(end) if end > start => &value[start + 1..end],
        _ => &value[start + 1..],
    }
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn unescape(value: &str) -> String {
    value.replace("\\\"", "\"").replace("\\\\", "\\")
}

/// Whether two entries say the same thing, ignoring incidental whitespace.
fn same_entry(left: &LocalisationEntry, right: &LocalisationEntry) -> bool {
    left.key.trim() == right.key.trim()
        && left.version.trim() == right.version.trim()
        && left.value == right.value
}

fn format_entry(entry: &LocalisationEntry) -> String {
    let version = entry.version.trim();
    format!(
        " {}:{} \"{}\"",
        entry.key.trim(),
        version,
        escape(&entry.value)
    )
}

/// Reads just enough of every `.yml` under the localisation folders to list them.
pub fn scan(folder: &str) -> AppResult<Vec<LocalisationFileInfo>> {
    let root = Path::new(folder);
    if !root.is_dir() {
        return Err(AppError::message(format!("{folder} is not a folder")));
    }

    let mut files = Vec::new();

    for directory in ["localisation", "localization"] {
        let localisation_root = root.join(directory);
        if !localisation_root.is_dir() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&localisation_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("yml") {
                continue;
            }

            let Ok(file) = text_file::read(path) else {
                continue;
            };
            let (language, entries) = parse(&file.content);

            files.push(LocalisationFileInfo {
                path: crate::project::normalise_path(path),
                relative_path: path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/"),
                language,
                entry_count: entries.len(),
                has_bom: file.has_bom,
            });
        }
    }

    files.sort_by(|left, right| {
        left.relative_path
            .to_lowercase()
            .cmp(&right.relative_path.to_lowercase())
    });

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "l_english:\n # a comment\n GREETING:0 \"Hello\"\n FAREWELL:1 \"Goodbye \\\"friend\\\"\" # trailing\n\n EMPTY_VERSION: \"No version\"\n";

    fn write_temp(name: &str, content: &str) -> String {
        let directory = std::env::temp_dir().join("hoi4ms-localisation-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join(name);
        std::fs::write(&path, content).expect("write");
        path.display().to_string()
    }

    #[test]
    fn parses_entries_versions_and_escapes() {
        let (language, entries) = parse(SAMPLE);

        assert_eq!(language, "l_english");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].key, "GREETING");
        assert_eq!(entries[0].version, "0");
        assert_eq!(entries[1].value, "Goodbye \"friend\"");
        assert_eq!(entries[2].version, "");
    }

    #[test]
    fn keeps_comments_when_saving() {
        let path = write_temp("keep.yml", SAMPLE);
        let file = read(&path).expect("reads");

        let mut entries = file.entries.clone();
        entries[0].value = "Hi there".to_string();

        write(&path, &file.language, &entries).expect("writes");
        let saved = std::fs::read_to_string(&path).expect("read back");

        assert!(saved.contains("# a comment"));
        assert!(saved.contains("GREETING:0 \"Hi there\""));
    }

    #[test]
    fn leaves_untouched_entry_lines_byte_for_byte() {
        let path = write_temp(
            "untouched.yml",
            "l_english:\n GREETING:0 \"Hello\"\n FAREWELL:0 \"Bye\" # translator note\n  SPACED:0    \"Odd spacing\"\n",
        );
        let file = read(&path).expect("reads");

        let mut entries = file.entries.clone();
        entries[0].value = "Hi".to_string();
        write(&path, &file.language, &entries).expect("writes");

        let saved = std::fs::read_to_string(&path).expect("read back");

        // The edited entry is rebuilt.
        assert!(saved.contains(" GREETING:0 \"Hi\""));
        // The others keep their comment and their spacing.
        assert!(
            saved.contains(" FAREWELL:0 \"Bye\" # translator note"),
            "comment lost:\n{saved}"
        );
        assert!(
            saved.contains("  SPACED:0    \"Odd spacing\""),
            "spacing lost:\n{saved}"
        );
    }

    #[test]
    fn writes_a_byte_order_mark() {
        let path = write_temp("bom.yml", "l_english:\n KEY:0 \"value\"\n");
        let file = read(&path).expect("reads");
        write(&path, &file.language, &file.entries).expect("writes");

        let bytes = std::fs::read(&path).expect("read back");
        assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);
    }

    #[test]
    fn adds_and_removes_entries() {
        let path = write_temp("mutate.yml", SAMPLE);
        let file = read(&path).expect("reads");

        let mut entries: Vec<LocalisationEntry> = file
            .entries
            .iter()
            .filter(|entry| entry.key != "FAREWELL")
            .cloned()
            .collect();
        entries.push(LocalisationEntry {
            key: "NEW_KEY".into(),
            version: "0".into(),
            value: "Added".into(),
            line: None,
        });

        let saved = write(&path, &file.language, &entries).expect("writes");

        assert!(saved.entries.iter().all(|entry| entry.key != "FAREWELL"));
        assert!(saved.entries.iter().any(|entry| entry.key == "NEW_KEY"));
        assert_eq!(saved.entries.len(), 3);
    }

    #[test]
    fn escapes_quotes_on_the_way_out() {
        let path = write_temp("escape.yml", "l_english:\n KEY:0 \"plain\"\n");
        let file = read(&path).expect("reads");

        let mut entries = file.entries.clone();
        entries[0].value = "say \"hello\"".to_string();
        let saved = write(&path, &file.language, &entries).expect("writes");

        assert_eq!(saved.entries[0].value, "say \"hello\"");
    }
}
