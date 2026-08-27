//! Recursive-descent parser producing a span-annotated tree.
//!
//! Nothing is normalised: every node points back at the byte range it was read
//! from, which is what lets `crate::paradox::edit` patch one value and leave
//! the author formatting, comments and ordering untouched.

use super::lexer::{tokenize, unquote, Token, TokenKind};
use super::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} (at byte {})", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone)]
pub struct Scalar {
    pub raw: String,
    pub span: Span,
    pub quoted: bool,
}

impl Scalar {
    pub fn value(&self) -> String {
        if self.quoted {
            unquote(&self.raw)
        } else {
            self.raw.clone()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    /// Byte range including both braces.
    pub span: Span,
    /// Byte range between the braces.
    pub inner: Span,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Value {
    Scalar(Scalar),
    Block(Block),
}

impl Value {
    pub fn span(&self) -> Span {
        match self {
            Value::Scalar(scalar) => scalar.span,
            Value::Block(block) => block.span,
        }
    }

    pub fn as_scalar(&self) -> Option<&Scalar> {
        match self {
            Value::Scalar(scalar) => Some(scalar),
            Value::Block(_) => None,
        }
    }

    pub fn as_block(&self) -> Option<&Block> {
        match self {
            Value::Block(block) => Some(block),
            Value::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Pair {
    pub key: String,
    /// The assignment token, usually `=` but comparisons appear in triggers.
    pub operator: String,
    pub value: Value,
    /// Byte range covering the whole `key = value` assignment.
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Item {
    Pair(Pair),
    Value(Value),
}

#[derive(Debug, Clone)]
pub struct Document {
    pub items: Vec<Item>,
    /// Blocks the file never closed. Hearts of Iron IV accepts these — one of
    /// its own country files ends mid-block — so they are closed at the end of
    /// the file rather than rejected.
    pub unclosed_blocks: usize,
}

/// Shared lookup helpers over a list of items.
pub trait Items {
    fn items(&self) -> &[Item];

    fn pairs(&self) -> Vec<&Pair> {
        self.items()
            .iter()
            .filter_map(|item| match item {
                Item::Pair(pair) => Some(pair),
                Item::Value(_) => None,
            })
            .collect()
    }

    /// First direct child with the given key, compared case-insensitively.
    fn find(&self, key: &str) -> Option<&Pair> {
        self.items().iter().find_map(|item| match item {
            Item::Pair(pair) if pair.key.eq_ignore_ascii_case(key) => Some(pair),
            _ => None,
        })
    }

    fn find_all(&self, key: &str) -> Vec<&Pair> {
        self.items()
            .iter()
            .filter_map(|item| match item {
                Item::Pair(pair) if pair.key.eq_ignore_ascii_case(key) => Some(pair),
                _ => None,
            })
            .collect()
    }

    /// Scalar value of the first direct child assigned with the given key.
    /// Comparisons such as `threat > 2` are not assignments and are skipped.
    fn scalar(&self, key: &str) -> Option<String> {
        self.find(key)
            .filter(|pair| matches!(pair.operator.as_str(), "=" | "=="))
            .and_then(|pair| pair.value.as_scalar())
            .map(|scalar| scalar.value())
    }

    fn block(&self, key: &str) -> Option<&Block> {
        self.find(key).and_then(|pair| pair.value.as_block())
    }
}

impl Items for Document {
    fn items(&self) -> &[Item] {
        &self.items
    }
}

impl Items for Block {
    fn items(&self) -> &[Item] {
        &self.items
    }
}

pub fn parse(source: &str) -> Result<Document, ParseError> {
    let tokens = tokenize(source);
    let mut parser = Parser {
        tokens,
        position: 0,
        source_len: source.len(),
        unclosed_blocks: 0,
    };
    let items = parser.parse_items(false)?;
    Ok(Document {
        items,
        unclosed_blocks: parser.unclosed_blocks,
    })
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
    source_len: usize,
    unclosed_blocks: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    /// Offset where the current token starts, used as the end of a block body.
    fn current_offset(&self) -> usize {
        self.tokens
            .get(self.position)
            .map(|token| token.span.start)
            .unwrap_or(self.source_len)
    }

    fn parse_items(&mut self, inside_block: bool) -> Result<Vec<Item>, ParseError> {
        let mut items = Vec::new();

        loop {
            match self.peek() {
                // A block left open at end of file is closed here, the way the
                // game does, instead of failing the whole file.
                None => break,
                Some(token) if token.kind == TokenKind::CloseBrace => {
                    if inside_block {
                        break;
                    }
                    // A stray closing brace at top level is skipped rather than
                    // fatal: real mod files occasionally carry one.
                    self.position += 1;
                    continue;
                }
                Some(_) => items.push(self.parse_item()?),
            }
        }

        Ok(items)
    }

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let token = self
            .advance()
            .expect("parse_item is only called with a token available");

        match token.kind {
            TokenKind::OpenBrace => Ok(Item::Value(Value::Block(self.parse_block(&token)?))),
            TokenKind::Word | TokenKind::Quoted => {
                let is_pair = self
                    .peek()
                    .is_some_and(|next| next.kind == TokenKind::Operator);

                if !is_pair {
                    return Ok(Item::Value(Value::Scalar(scalar_from(&token))));
                }

                let operator = self.advance().expect("operator was peeked");
                let value = self.parse_value()?;

                Ok(Item::Pair(Pair {
                    key: token.value(),
                    operator: operator.raw,
                    span: Span::new(token.span.start, value.span().end),
                    value,
                }))
            }
            TokenKind::Operator => Err(ParseError {
                message: format!("unexpected operator `{}`", token.raw),
                offset: token.span.start,
            }),
            TokenKind::CloseBrace => unreachable!("close brace is handled by parse_items"),
        }
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        let token = self.advance().ok_or_else(|| ParseError {
            message: "expected a value but reached end of file".into(),
            offset: self.source_len,
        })?;

        match token.kind {
            TokenKind::OpenBrace => Ok(Value::Block(self.parse_block(&token)?)),
            TokenKind::Word | TokenKind::Quoted => Ok(Value::Scalar(scalar_from(&token))),
            TokenKind::CloseBrace | TokenKind::Operator => Err(ParseError {
                message: format!("expected a value but found `{}`", token.raw),
                offset: token.span.start,
            }),
        }
    }

    fn parse_block(&mut self, open: &Token) -> Result<Block, ParseError> {
        let items = self.parse_items(true)?;
        let inner_end = self.current_offset();

        let end = match self.advance() {
            Some(close) => close.span.end,
            None => {
                self.unclosed_blocks += 1;
                self.source_len
            }
        };

        Ok(Block {
            span: Span::new(open.span.start, end),
            inner: Span::new(open.span.end, inner_end),
            items,
        })
    }
}

fn scalar_from(token: &Token) -> Scalar {
    Scalar {
        raw: token.raw.clone(),
        span: token.span,
        quoted: token.kind == TokenKind::Quoted,
    }
}
