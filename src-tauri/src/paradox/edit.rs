//! Byte-range rewrites over a parsed script file.
//!
//! Edits are collected against the *original* source and applied back to front,
//! so offsets gathered during parsing stay valid until the very last one lands.

use super::lexer::{needs_quotes, quote};
use super::parser::{Block, Items, Pair, Value};
use super::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub span: Span,
    pub replacement: String,
}

impl TextEdit {
    pub fn new(span: Span, replacement: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
        }
    }
}

/// Applies edits to `source`. Overlapping edits are resolved by keeping the
/// first one seen for a given range start, which keeps a caller mistake from
/// producing garbled output.
pub fn apply_edits(source: &str, mut edits: Vec<TextEdit>) -> String {
    edits.sort_by_key(|edit| edit.span.start);

    let mut result = String::with_capacity(source.len());
    let mut cursor = 0usize;

    for edit in edits {
        if edit.span.start < cursor {
            continue;
        }
        result.push_str(&source[cursor..edit.span.start]);
        result.push_str(&edit.replacement);
        cursor = edit.span.end.max(edit.span.start);
    }

    if cursor < source.len() {
        result.push_str(&source[cursor..]);
    }

    result
}

/// The line separator the file already uses.
pub fn detect_newline(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

pub fn line_start(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0)
}

/// End of the line containing `offset`, excluding the line break itself.
pub fn line_end(source: &str, offset: usize) -> usize {
    let start = offset.min(source.len());
    source[start..]
        .find('\n')
        .map(|index| {
            let end = start + index;
            if end > start && source.as_bytes()[end - 1] == b'\r' {
                end - 1
            } else {
                end
            }
        })
        .unwrap_or(source.len())
}

/// Leading whitespace of the line containing `offset`.
pub fn indent_at(source: &str, offset: usize) -> String {
    let start = line_start(source, offset);
    source[start..offset.min(source.len())]
        .chars()
        .take_while(|character| *character == ' ' || *character == '\t')
        .collect()
}

/// Indentation to use for children of `block`: copied from an existing child
/// when there is one, otherwise one tab deeper than the block itself.
pub fn child_indent(source: &str, block: &Block) -> String {
    if let Some(first) = block.items.first() {
        let offset = match first {
            super::parser::Item::Pair(pair) => pair.span.start,
            super::parser::Item::Value(value) => value.span().start,
        };
        // Only trust the indentation when the child really starts its own line.
        let start = line_start(source, offset);
        if source[start..offset]
            .chars()
            .all(|character| character == ' ' || character == '\t')
        {
            return indent_at(source, offset);
        }
    }

    let base = indent_at(source, block.span.start);
    format!("{base}\t")
}

/// True when nothing but `pair` (and optionally a comment) sits on its line.
fn pair_owns_its_line(source: &str, pair: &Pair) -> bool {
    let start = line_start(source, pair.span.start);
    source[start..pair.span.start]
        .chars()
        .all(|character| character == ' ' || character == '\t')
        && pair.span.end >= line_end(source, pair.span.end).min(pair.span.end)
}

/// Sets `key = value` inside `block`, inserting the key when it is missing and
/// deleting it when `value` is empty.
pub fn set_scalar(source: &str, block: &Block, key: &str, value: &str) -> Vec<TextEdit> {
    let existing = block.find(key);
    let trimmed = value.trim();

    match (existing, trimmed.is_empty()) {
        (Some(pair), true) => remove_pair(source, pair),
        (Some(pair), false) => {
            let raw = if needs_quotes(trimmed) {
                quote(trimmed)
            } else {
                trimmed.to_string()
            };
            vec![TextEdit::new(pair.value.span(), raw)]
        }
        (None, true) => Vec::new(),
        (None, false) => {
            let raw = if needs_quotes(trimmed) {
                quote(trimmed)
            } else {
                trimmed.to_string()
            };
            vec![insert_line(source, block, &format!("{key} = {raw}"))]
        }
    }
}

/// Sets `key = { ... }` inside `block` with `content` as the block body.
pub fn set_block(source: &str, block: &Block, key: &str, content: &str) -> Vec<TextEdit> {
    let existing = block.find(key);
    let trimmed = content.trim();

    match (existing, trimmed.is_empty()) {
        (Some(pair), true) => remove_pair(source, pair),
        (Some(pair), false) => {
            let indent = indent_at(source, pair.span.start);
            let newline = detect_newline(source);
            let formatted = format_block(trimmed, &indent, newline);
            vec![TextEdit::new(pair.value.span(), formatted)]
        }
        (None, true) => Vec::new(),
        (None, false) => {
            let indent = child_indent(source, block);
            let newline = detect_newline(source);
            let formatted = format_block(trimmed, &indent, newline);
            vec![insert_line(source, block, &format!("{key} = {formatted}"))]
        }
    }
}

/// Removes a pair together with the line it sits on when it owns that line.
pub fn remove_pair(source: &str, pair: &Pair) -> Vec<TextEdit> {
    if !pair_owns_its_line(source, pair) {
        return vec![TextEdit::new(pair.span, String::new())];
    }

    let start = line_start(source, pair.span.start);
    let mut end = line_end(source, pair.span.end);
    // Swallow the line break so no blank line is left behind.
    if source[end..].starts_with("\r\n") {
        end += 2;
    } else if source[end..].starts_with('\n') {
        end += 1;
    }

    vec![TextEdit::new(Span::new(start, end), String::new())]
}

/// An edit inserting `text` as a new last line inside `block`.
pub fn insert_line(source: &str, block: &Block, text: &str) -> TextEdit {
    insert_lines(source, block, std::slice::from_ref(&text.to_string()))
}

/// An edit appending several lines inside `block`, as one contiguous rewrite so
/// the caller never has to merge overlapping insertions at the same offset.
pub fn insert_lines(source: &str, block: &Block, lines: &[String]) -> TextEdit {
    let newline = detect_newline(source);
    let indent = child_indent(source, block);
    let closing_indent = indent_at(source, block.span.start);

    let body = lines
        .iter()
        .map(|line| format!("{newline}{indent}{line}"))
        .collect::<String>();

    // Insert just before the closing brace.
    let insert_at = block.inner.end;
    let before = &source[block.inner.start..insert_at];

    if before.trim().is_empty() {
        // Empty block: expand it onto its own lines.
        return TextEdit::new(
            Span::new(block.inner.start, insert_at),
            format!("{body}{newline}{closing_indent}"),
        );
    }

    let trailing_start = source[..insert_at]
        .rfind(|character: char| !character.is_whitespace())
        .map(|index| index + 1)
        .unwrap_or(insert_at);

    TextEdit::new(
        Span::new(trailing_start, insert_at),
        format!("{body}{newline}{closing_indent}"),
    )
}

/// Renders `content` as a brace block indented relative to `indent`.
pub fn format_block(content: &str, indent: &str, newline: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return "{ }".to_string();
    }

    let lines = dedent(trimmed);
    if lines.len() == 1 && lines[0].len() <= 60 && !lines[0].contains('{') {
        return format!("{{ {} }}", lines[0]);
    }

    let inner_indent = format!("{indent}\t");
    let body = lines
        .iter()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{inner_indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join(newline);

    format!("{{{newline}{body}{newline}{indent}}}")
}

/// Strips the indentation shared by every non-empty line.
pub fn dedent(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(|line| line.trim_end()).collect();

    let shared = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line[shared.min(line.len())..].to_string()
            }
        })
        .collect()
}

/// Text of a block body with shared indentation removed, for showing a nested
/// block such as `completion_reward` in a plain editor.
pub fn block_body(source: &str, value: &Value) -> String {
    match value {
        Value::Scalar(scalar) => scalar.value(),
        Value::Block(block) => {
            let inner = block.inner.text(source);
            dedent(inner.trim_matches(|character| character == '\n' || character == '\r'))
                .join("\n")
                .trim()
                .to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paradox::parser::Document;
    use crate::paradox::parse;

    /// The first `focus` block inside the first `focus_tree` of a document.
    fn focus_block(document: &Document) -> &Block {
        document
            .find("focus_tree")
            .and_then(|tree| tree.value.as_block())
            .and_then(|tree| tree.find("focus"))
            .and_then(|focus| focus.value.as_block())
            .expect("the sample has a focus")
    }

    const SOURCE: &str = "focus_tree = {\n\tid = tree\n\tfocus = {\n\t\tid = alpha\n\t\tx = 1\n\t\tcost = 10\n\t}\n}\n";

    #[test]
    fn replaces_an_existing_scalar() {
        let document = parse(SOURCE).expect("parses");
        let block = focus_block(&document);

        let edits = set_scalar(SOURCE, block, "x", "7");
        let result = apply_edits(SOURCE, edits);

        assert!(result.contains("x = 7"));
        assert!(result.contains("id = alpha"));
    }

    #[test]
    fn inserts_a_missing_scalar_with_matching_indentation() {
        let document = parse(SOURCE).expect("parses");
        let block = focus_block(&document);

        let edits = set_scalar(SOURCE, block, "y", "4");
        let result = apply_edits(SOURCE, edits);

        assert!(result.contains("\n\t\ty = 4\n"), "unexpected output:\n{result}");
        assert!(result.contains("cost = 10"));
    }

    #[test]
    fn removes_a_scalar_when_the_value_is_cleared() {
        let document = parse(SOURCE).expect("parses");
        let block = focus_block(&document);

        let edits = set_scalar(SOURCE, block, "cost", "");
        let result = apply_edits(SOURCE, edits);

        assert!(!result.contains("cost"));
        assert!(result.contains("x = 1"));
    }

    #[test]
    fn quotes_values_that_need_it() {
        let document = parse(SOURCE).expect("parses");
        let block = focus_block(&document);

        let result = apply_edits(SOURCE, set_scalar(SOURCE, block, "id", "two words"));
        assert!(result.contains("id = \"two words\""));
    }

    #[test]
    fn writes_a_multiline_block() {
        let document = parse(SOURCE).expect("parses");
        let block = focus_block(&document);

        let edits = set_block(
            SOURCE,
            block,
            "completion_reward",
            "add_political_power = 120\nadd_stability = 0.05",
        );
        let result = apply_edits(SOURCE, edits);

        assert!(result.contains("completion_reward = {"));
        assert!(result.contains("\t\t\tadd_political_power = 120"));
        assert!(result.contains("\t\t\tadd_stability = 0.05"));
    }

    #[test]
    fn keeps_unrelated_comments_and_spacing() {
        let source = "focus = {\n\tid = alpha # keep me\n\n\t# a note\n\tx = 1\n}\n";
        let document = parse(source).expect("parses");
        let focus = document.find("focus").expect("focus");
        let block = focus.value.as_block().expect("block");

        let result = apply_edits(source, set_scalar(source, block, "x", "9"));

        assert!(result.contains("# keep me"));
        assert!(result.contains("# a note"));
        assert!(result.contains("x = 9"));
    }

    #[test]
    fn later_edits_do_not_shift_earlier_ones() {
        let document = parse(SOURCE).expect("parses");
        let block = focus_block(&document);

        let mut edits = set_scalar(SOURCE, block, "id", "renamed");
        edits.extend(set_scalar(SOURCE, block, "cost", "25"));
        let result = apply_edits(SOURCE, edits);

        assert!(result.contains("id = renamed"));
        assert!(result.contains("cost = 25"));
    }
}
