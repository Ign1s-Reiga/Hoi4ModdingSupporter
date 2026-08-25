//! Reading and surgically editing Clausewitz (Paradox) script files.
//!
//! The rule the whole module follows: a mod author formatting is theirs. We
//! parse to find out where a value lives, then rewrite only that byte range,
//! so comments, spacing and unrelated keys survive a save untouched.

pub mod edit;
pub mod lexer;
pub mod parser;

use serde::{Deserialize, Serialize};

pub use parser::{parse, Block, Item, Items, Pair, ParseError, Value};

/// A byte range inside a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        source
            .get(self.start..self.end)
            .unwrap_or_default()
    }
}

/// One-based line number containing `offset`, for jumping the editor to a node.
pub fn line_of(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
focus_tree = {
    id = test_tree
    country = { factor = 1 }

    focus = {
        id = test_focus # trailing comment
        icon = GFX_goal_generic_demand_territory
        x = 3
        y = 0
        cost = 10
        prerequisite = { focus = other_focus }
        completion_reward = {
            add_political_power = 120
        }
    }
}
"#;

    #[test]
    fn parses_nested_blocks() {
        let document = parse(SAMPLE).expect("sample parses");
        let tree = document.find("focus_tree").expect("focus_tree exists");
        let tree_block = tree.value.as_block().expect("focus_tree is a block");

        assert_eq!(tree_block.scalar("id").as_deref(), Some("test_tree"));

        let focus = tree_block.find("focus").expect("focus exists");
        let focus_block = focus.value.as_block().expect("focus is a block");
        assert_eq!(focus_block.scalar("id").as_deref(), Some("test_focus"));
        assert_eq!(focus_block.scalar("x").as_deref(), Some("3"));
    }

    #[test]
    fn ignores_comments_and_reads_inline_blocks() {
        let document = parse(SAMPLE).expect("sample parses");
        let tree = document.block("focus_tree").expect("focus_tree");
        let focus_block = tree.block("focus").expect("focus is a block");

        // The trailing comment is not part of the value.
        assert_eq!(focus_block.scalar("id").as_deref(), Some("test_focus"));

        let prerequisite = focus_block.block("prerequisite").expect("prerequisite");
        assert_eq!(prerequisite.scalar("focus").as_deref(), Some("other_focus"));
    }

    #[test]
    fn spans_point_back_at_the_source() {
        let document = parse(SAMPLE).expect("sample parses");
        let tree = document.find("focus_tree").expect("focus_tree exists");
        let block = tree.value.as_block().expect("block");
        let id = block.find("id").expect("id");

        assert_eq!(id.span.text(SAMPLE), "id = test_tree");
        assert_eq!(id.value.span().text(SAMPLE), "test_tree");
    }

    #[test]
    fn quoted_values_are_unescaped() {
        let document = parse(r#"name = "A \"quoted\" name""#).expect("parses");
        assert_eq!(
            document.scalar("name").as_deref(),
            Some(r#"A "quoted" name"#)
        );
    }

    #[test]
    fn reports_unbalanced_braces() {
        let error = parse("focus = { id = a").expect_err("unbalanced braces fail");
        assert!(error.message.contains("closing brace"));
    }

    #[test]
    fn reads_bare_lists() {
        let document = parse("tags = { \"Alternative History\" Gameplay }").expect("parses");
        let block = document.block("tags").expect("tags block");
        assert_eq!(block.items.len(), 2);
    }
}
