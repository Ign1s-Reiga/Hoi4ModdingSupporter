//! Tokenizer for the Clausewitz script format used by Hearts of Iron IV
//! (`common/`, `events/`, `history/` and friends).
//!
//! Every token keeps the byte range it came from so the editor can rewrite a
//! single value without reformatting the rest of the file.

use super::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// A bare word, number or date such as `focus`, `10`, `1936.1.1`.
    Word,
    /// A double quoted string. The span covers the quotes.
    Quoted,
    /// One of `=`, `==`, `>`, `<`, `>=`, `<=`, `!=`, `?=`.
    Operator,
    OpenBrace,
    CloseBrace,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// The literal source text of the token, quotes included.
    pub raw: String,
}

impl Token {
    /// The token text with surrounding quotes and escapes removed.
    pub fn value(&self) -> String {
        if self.kind == TokenKind::Quoted {
            unquote(&self.raw)
        } else {
            self.raw.clone()
        }
    }
}

pub fn unquote(raw: &str) -> String {
    let trimmed = raw
        .strip_prefix('"')
        .map(|rest| rest.strip_suffix('"').unwrap_or(rest))
        .unwrap_or(raw);

    let mut out = String::with_capacity(trimmed.len());
    let mut characters = trimmed.chars();

    while let Some(character) = characters.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }

        match characters.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(escaped) => out.push(escaped),
            None => out.push('\\'),
        }
    }

    out
}

pub fn quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// True when a value has to be written back as a quoted string.
pub fn needs_quotes(value: &str) -> bool {
    value.is_empty()
        || value.chars().any(|character| {
            character.is_whitespace() || matches!(character, '"' | '{' | '}' | '#' | '=')
        })
}

fn is_operator_start(character: char) -> bool {
    matches!(character, '=' | '<' | '>' | '!' | '?')
}

fn is_token_boundary(character: char) -> bool {
    character.is_whitespace()
        || is_operator_start(character)
        || matches!(character, '{' | '}' | '#' | '"')
}

pub fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut index = 0usize;

    while index < source.len() {
        let rest = &source[index..];
        let Some(character) = rest.chars().next() else {
            break;
        };

        if character.is_whitespace() {
            index += character.len_utf8();
            continue;
        }

        // Comments run to the end of the line.
        if character == '#' {
            index += rest.find('\n').unwrap_or(rest.len());
            continue;
        }

        if character == '{' || character == '}' {
            let kind = if character == '{' {
                TokenKind::OpenBrace
            } else {
                TokenKind::CloseBrace
            };
            tokens.push(Token {
                kind,
                span: Span::new(index, index + 1),
                raw: character.to_string(),
            });
            index += 1;
            continue;
        }

        if character == '"' {
            let end = scan_quoted(source, index);
            tokens.push(Token {
                kind: TokenKind::Quoted,
                span: Span::new(index, end),
                raw: source[index..end].to_string(),
            });
            index = end;
            continue;
        }

        if is_operator_start(character) {
            let mut end = index + character.len_utf8();
            if rest[character.len_utf8()..].starts_with('=') {
                end += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Operator,
                span: Span::new(index, end),
                raw: source[index..end].to_string(),
            });
            index = end;
            continue;
        }

        let end = scan_word(source, index);
        tokens.push(Token {
            kind: TokenKind::Word,
            span: Span::new(index, end),
            raw: source[index..end].to_string(),
        });
        index = end;
    }

    tokens
}

/// Returns the byte index just past the end of the bare word starting at `start`.
///
/// A bracketed reference such as `[?variable]` or `[Root.GetName]` is one
/// word: the `?` inside would otherwise be read as an operator.
fn scan_word(source: &str, start: usize) -> usize {
    let mut in_brackets = false;
    let mut end = source.len();

    for (offset, character) in source[start..].char_indices() {
        let boundary = match character {
            '[' => {
                in_brackets = true;
                false
            }
            ']' if in_brackets => {
                in_brackets = false;
                false
            }
            // A brace, a quote or a line end closes the word whatever the
            // bracket state: a reference never holds one, and a stray `[`
            // must not swallow the brace that ends the block.
            '{' | '}' | '"' | '\n' => true,
            _ if in_brackets => false,
            _ => is_token_boundary(character),
        };
        if boundary {
            end = start + offset;
            break;
        }
    }

    // Whitespace before a brace that ended an unclosed reference is not
    // part of the word.
    source[start..end].trim_end().len() + start
}

/// Returns the byte index just past the closing quote of the string starting at
/// `start`. An unterminated string stops at the end of its line.
fn scan_quoted(source: &str, start: usize) -> usize {
    let mut escaped = false;

    for (offset, character) in source[start + 1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match character {
            '\\' => escaped = true,
            '"' => return start + 1 + offset + 1,
            '\n' => return start + 1 + offset,
            _ => {}
        }
    }

    source.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_comments_and_whitespace() {
        let tokens = tokenize("# a comment\nid = alpha\n");
        let kinds: Vec<&TokenKind> = tokens.iter().map(|token| &token.kind).collect();

        assert_eq!(
            kinds,
            vec![&TokenKind::Word, &TokenKind::Operator, &TokenKind::Word]
        );
        assert_eq!(tokens[2].raw, "alpha");
    }

    #[test]
    fn keeps_a_bracketed_reference_as_one_word() {
        let tokens = tokenize("VALUE = [?idea_cost]\nname = [Root.GetName]\n");
        let words: Vec<&str> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Word)
            .map(|token| token.raw.as_str())
            .collect();

        assert_eq!(
            words,
            vec!["VALUE", "[?idea_cost]", "name", "[Root.GetName]"]
        );
        assert_eq!(
            tokens
                .iter()
                .filter(|token| token.kind == TokenKind::Operator)
                .count(),
            2
        );
    }

    #[test]
    fn a_stray_bracket_does_not_swallow_the_closing_brace() {
        let tokens = tokenize("a = { name = [Root.GetName }\n");
        let kinds: Vec<&TokenKind> = tokens.iter().map(|token| &token.kind).collect();

        assert_eq!(kinds.last(), Some(&&TokenKind::CloseBrace));
        assert_eq!(tokens[5].raw, "[Root.GetName");
    }

    #[test]
    fn reads_quoted_strings_with_escapes() {
        let tokens = tokenize(r#"name = "say \"hi\"""#);
        assert_eq!(tokens[2].kind, TokenKind::Quoted);
        assert_eq!(tokens[2].value(), r#"say "hi""#);
    }

    #[test]
    fn a_hash_inside_a_string_is_not_a_comment() {
        let tokens = tokenize(r#"icon = "GFX_#_odd""#);
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[2].value(), "GFX_#_odd");
    }

    #[test]
    fn reads_two_character_operators() {
        let tokens = tokenize("threat >= 2");
        assert_eq!(tokens[1].raw, ">=");
    }

    #[test]
    fn spans_cover_the_original_text() {
        let source = "x = 12";
        let tokens = tokenize(source);
        assert_eq!(tokens[2].span.text(source), "12");
    }

    #[test]
    fn quotes_only_values_that_need_it() {
        assert!(!needs_quotes("GFX_goal_generic"));
        assert!(needs_quotes("two words"));
        assert_eq!(quote(r#"a "b""#), r#""a \"b\"""#);
    }
}
