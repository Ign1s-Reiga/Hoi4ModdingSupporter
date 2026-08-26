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
///
/// Scripts sometimes put several assignments on one line, and deleting the
/// whole line would take the neighbours with it.
fn pair_owns_its_line(source: &str, pair: &Pair) -> bool {
    let start = line_start(source, pair.span.start);
    let leading_is_blank = source[start..pair.span.start]
        .chars()
        .all(|character| character == ' ' || character == '\t');

    // A block value can span lines, so measure from where the pair ends.
    let end = line_end(source, pair.span.end);
    let trailing = source
        .get(pair.span.end.min(end)..end)
        .unwrap_or_default()
        .trim();
    let trailing_is_blank = trailing.is_empty() || trailing.starts_with('#');

    leading_is_blank && trailing_is_blank
}

/// What setting one field resolves to against the current source.
enum Change {
    /// Rewrite the value of an assignment that is already there.
    Replace(TextEdit),
    /// Delete an assignment that is already there.
    Remove(Vec<TextEdit>),
    /// Append a new line to the block body.
    Add(String),
    Nothing,
}

fn scalar_change(source: &str, block: &Block, key: &str, value: &str) -> Change {
    let trimmed = value.trim();
    let existing = block.find(key);

    if trimmed.is_empty() {
        return match existing {
            Some(pair) => Change::Remove(remove_pair(source, pair)),
            None => Change::Nothing,
        };
    }

    let raw = if needs_quotes(trimmed) {
        quote(trimmed)
    } else {
        trimmed.to_string()
    };

    match existing {
        Some(pair) => Change::Replace(TextEdit::new(pair.value.span(), raw)),
        None => Change::Add(format!("{key} = {raw}")),
    }
}

fn block_change(source: &str, block: &Block, key: &str, content: &str) -> Change {
    let trimmed = content.trim();
    let existing = block.find(key);
    let newline = detect_newline(source);

    if trimmed.is_empty() {
        return match existing {
            Some(pair) => Change::Remove(remove_pair(source, pair)),
            None => Change::Nothing,
        };
    }

    match existing {
        Some(pair) => {
            let indent = indent_at(source, pair.span.start);
            Change::Replace(TextEdit::new(
                pair.value.span(),
                format_block(trimmed, &indent, newline),
            ))
        }
        None => {
            let indent = child_indent(source, block);
            Change::Add(format!("{key} = {}", format_block(trimmed, &indent, newline)))
        }
    }
}

/// Collects every edit made to one block.
///
/// New lines are gathered and emitted as a single insertion when the editor is
/// finished. Each insertion targets the same span just before the closing
/// brace, so appending them one at a time would leave `apply_edits` keeping
/// only the first and silently dropping the rest.
pub struct BlockEditor<'a> {
    source: &'a str,
    block: &'a Block,
    edits: Vec<TextEdit>,
    additions: Vec<String>,
}

impl<'a> BlockEditor<'a> {
    pub fn new(source: &'a str, block: &'a Block) -> Self {
        Self {
            source,
            block,
            edits: Vec::new(),
            additions: Vec::new(),
        }
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    pub fn block(&self) -> &'a Block {
        self.block
    }

    /// Sets `key = value`, adding the key when it is missing and deleting it
    /// when `value` is empty.
    pub fn set_scalar(&mut self, key: &str, value: &str) -> &mut Self {
        let change = scalar_change(self.source, self.block, key, value);
        self.apply(change)
    }

    /// Sets `key = { ... }` with `content` as the body.
    pub fn set_block(&mut self, key: &str, content: &str) -> &mut Self {
        let change = block_change(self.source, self.block, key, content);
        self.apply(change)
    }

    pub fn edit(&mut self, edit: TextEdit) -> &mut Self {
        self.edits.push(edit);
        self
    }

    pub fn remove(&mut self, pair: &Pair) -> &mut Self {
        self.edits.extend(remove_pair(self.source, pair));
        self
    }

    /// Queues a whole line to append inside the block.
    pub fn add_line(&mut self, line: String) -> &mut Self {
        self.additions.push(line);
        self
    }

    fn apply(&mut self, change: Change) -> &mut Self {
        match change {
            Change::Replace(edit) => self.edits.push(edit),
            Change::Remove(edits) => self.edits.extend(edits),
            Change::Add(line) => self.additions.push(line),
            Change::Nothing => {}
        }
        self
    }

    pub fn finish(mut self) -> Vec<TextEdit> {
        if !self.additions.is_empty() {
            let insertion = insert_lines(self.source, self.block, &self.additions);
            self.edits.push(insertion);
        }
        self.edits
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
    use crate::paradox::parse;

    const SOURCE: &str = "focus_tree = {\n\tid = tree\n\tfocus = {\n\t\tid = alpha\n\t\tx = 1\n\t\tcost = 10\n\t}\n}\n";

    /// Runs `apply` against the first `focus` block in `source` and returns the
    /// rewritten file.
    fn edit_focus(source: &str, apply: impl FnOnce(&mut BlockEditor<'_>)) -> String {
        let document = parse(source).expect("parses");
        let block = document
            .find("focus_tree")
            .and_then(|tree| tree.value.as_block())
            .and_then(|tree| tree.find("focus"))
            .or_else(|| document.find("focus"))
            .and_then(|focus| focus.value.as_block())
            .expect("the sample has a focus");

        let mut editor = BlockEditor::new(source, block);
        apply(&mut editor);
        apply_edits(source, editor.finish())
    }

    #[test]
    fn replaces_an_existing_scalar() {
        let result = edit_focus(SOURCE, |editor| {
            editor.set_scalar("x", "7");
        });

        assert!(result.contains("x = 7"));
        assert!(result.contains("id = alpha"));
    }

    #[test]
    fn inserts_a_missing_scalar_with_matching_indentation() {
        let result = edit_focus(SOURCE, |editor| {
            editor.set_scalar("y", "4");
        });

        assert!(result.contains("\n\t\ty = 4\n"), "unexpected output:\n{result}");
        assert!(result.contains("cost = 10"));
    }

    #[test]
    fn adds_every_missing_field_rather_than_only_the_first() {
        let result = edit_focus(SOURCE, |editor| {
            editor.set_scalar("y", "4");
            editor.set_scalar("relative_position_id", "beta");
            editor.set_block("completion_reward", "add_political_power = 50");
        });

        // Each of these targets the same span before the closing brace, so all
        // three have to land in one insertion.
        assert!(result.contains("y = 4"), "missing y:\n{result}");
        assert!(
            result.contains("relative_position_id = beta"),
            "missing relative_position_id:\n{result}"
        );
        assert!(
            result.contains("completion_reward = { add_political_power = 50 }"),
            "missing completion_reward:\n{result}"
        );
        assert!(result.contains("id = alpha"));
    }

    #[test]
    fn mixes_additions_with_replacements() {
        let result = edit_focus(SOURCE, |editor| {
            editor.set_scalar("cost", "25");
            editor.set_scalar("y", "9");
        });

        assert!(result.contains("cost = 25"));
        assert!(result.contains("y = 9"));
    }

    #[test]
    fn removes_a_scalar_when_the_value_is_cleared() {
        let result = edit_focus(SOURCE, |editor| {
            editor.set_scalar("cost", "");
        });

        assert!(!result.contains("cost"));
        assert!(result.contains("x = 1"));
    }

    #[test]
    fn quotes_values_that_need_it() {
        let result = edit_focus(SOURCE, |editor| {
            editor.set_scalar("id", "two words");
        });

        assert!(result.contains("id = \"two words\""));
    }

    #[test]
    fn writes_a_multiline_block() {
        let result = edit_focus(SOURCE, |editor| {
            editor.set_block(
                "completion_reward",
                "add_political_power = 120\nadd_stability = 0.05",
            );
        });

        assert!(result.contains("completion_reward = {"));
        assert!(result.contains("\t\t\tadd_political_power = 120"));
        assert!(result.contains("\t\t\tadd_stability = 0.05"));
    }

    #[test]
    fn removing_a_key_keeps_its_line_mates() {
        let result = edit_focus("focus = { id = alpha x = 1 y = 2 }\n", |editor| {
            editor.set_scalar("x", "");
        });

        assert!(result.contains("id = alpha"));
        assert!(result.contains("y = 2"));
        assert!(!result.contains("x = 1"));
    }

    #[test]
    fn removing_a_key_that_owns_its_line_drops_the_line() {
        let result = edit_focus(SOURCE, |editor| {
            editor.set_scalar("x", "");
        });

        assert!(!result.contains("x = 1"));
        // No blank line is left where the assignment used to be.
        assert!(!result.contains("\n\n\t\tcost"));
    }

    #[test]
    fn keeps_unrelated_comments_and_spacing() {
        let source = "focus = {\n\tid = alpha # keep me\n\n\t# a note\n\tx = 1\n}\n";
        let result = edit_focus(source, |editor| {
            editor.set_scalar("x", "9");
        });

        assert!(result.contains("# keep me"));
        assert!(result.contains("# a note"));
        assert!(result.contains("x = 9"));
    }

    #[test]
    fn later_edits_do_not_shift_earlier_ones() {
        let result = edit_focus(SOURCE, |editor| {
            editor.set_scalar("id", "renamed");
            editor.set_scalar("cost", "25");
        });

        assert!(result.contains("id = renamed"));
        assert!(result.contains("cost = 25"));
    }
}
