//! National focus trees: reading `common/national_focus/*.txt` into an editable
//! shape and writing single focuses back without disturbing the rest of the file.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paradox::edit::{
    apply_edits, block_body, child_indent, detect_newline, format_block, indent_at, insert_lines,
    remove_pair, BlockEditor, TextEdit,
};
use crate::paradox::{self, Block, Item, Items, Pair, Value};
use crate::text_file;

/// Block-valued fields the editor exposes as free text.
const BLOCK_FIELDS: &[&str] = &[
    "available",
    "bypass",
    "allow_branch",
    "completion_reward",
    "ai_will_do",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Focus {
    pub id: String,
    pub icon: String,
    pub x: String,
    pub y: String,
    pub cost: String,
    pub relative_position_id: String,
    /// Each inner list is one `prerequisite = { focus = a focus = b }` block:
    /// any of them satisfies that prerequisite, all blocks must be satisfied.
    pub prerequisites: Vec<Vec<String>>,
    pub mutually_exclusive: Vec<String>,
    pub available: String,
    pub bypass: String,
    pub allow_branch: String,
    pub completion_reward: String,
    pub ai_will_do: String,
    /// Id of the enclosing `focus_tree`, empty for a `shared_focus`.
    pub tree_id: String,
    pub shared: bool,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusTree {
    pub id: String,
    pub country: String,
    pub default: bool,
    pub focus_count: usize,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusFile {
    pub path: String,
    pub trees: Vec<FocusTree>,
    pub focuses: Vec<Focus>,
    pub has_bom: bool,
    pub encoding: text_file::Encoding,
}

/// The editable fields sent back when saving. Empty strings clear a field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusUpdate {
    pub id: String,
    pub icon: String,
    pub x: String,
    pub y: String,
    pub cost: String,
    pub relative_position_id: String,
    pub prerequisites: Vec<Vec<String>>,
    pub mutually_exclusive: Vec<String>,
    pub available: String,
    pub bypass: String,
    pub allow_branch: String,
    pub completion_reward: String,
    pub ai_will_do: String,
}

pub fn read(path: &str) -> AppResult<FocusFile> {
    let file = text_file::read(Path::new(path))?;
    let document = paradox::parse(&file.content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let mut trees = Vec::new();
    let mut focuses = Vec::new();

    for pair in document.pairs() {
        let Some(block) = pair.value.as_block() else {
            continue;
        };

        match pair.key.to_lowercase().as_str() {
            "focus_tree" => {
                let tree_id = block.scalar("id").unwrap_or_default();
                let tree_focuses: Vec<&Pair> = block.find_all("focus");

                trees.push(FocusTree {
                    id: tree_id.clone(),
                    country: block
                        .find("country")
                        .map(|country| block_body(&file.content, &country.value))
                        .unwrap_or_default(),
                    default: block
                        .scalar("default")
                        .map(|value| value.eq_ignore_ascii_case("yes"))
                        .unwrap_or(false),
                    focus_count: tree_focuses.len(),
                    line: paradox::line_of(&file.content, pair.span.start),
                });

                for focus_pair in tree_focuses {
                    if let Some(focus_block) = focus_pair.value.as_block() {
                        focuses.push(read_focus(
                            &file.content,
                            focus_block,
                            &tree_id,
                            false,
                            focus_pair.span.start,
                        ));
                    }
                }
            }
            "shared_focus" | "focus" => {
                focuses.push(read_focus(&file.content, block, "", true, pair.span.start));
            }
            _ => {}
        }
    }

    Ok(FocusFile {
        path: path.replace('\\', "/"),
        trees,
        focuses,
        has_bom: file.has_bom,
        encoding: file.encoding,
    })
}

fn read_focus(source: &str, block: &Block, tree_id: &str, shared: bool, start: usize) -> Focus {
    let prerequisites = block
        .find_all("prerequisite")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .map(collect_focus_references)
        .filter(|group| !group.is_empty())
        .collect();

    let mutually_exclusive = block
        .find_all("mutually_exclusive")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .flat_map(collect_focus_references)
        .collect();

    let read_block_field = |key: &str| {
        block
            .find(key)
            .map(|pair| block_body(source, &pair.value))
            .unwrap_or_default()
    };

    Focus {
        id: block.scalar("id").unwrap_or_default(),
        icon: block.scalar("icon").unwrap_or_default(),
        x: block.scalar("x").unwrap_or_default(),
        y: block.scalar("y").unwrap_or_default(),
        cost: block.scalar("cost").unwrap_or_default(),
        relative_position_id: block.scalar("relative_position_id").unwrap_or_default(),
        prerequisites,
        mutually_exclusive,
        available: read_block_field("available"),
        bypass: read_block_field("bypass"),
        allow_branch: read_block_field("allow_branch"),
        completion_reward: read_block_field("completion_reward"),
        ai_will_do: read_block_field("ai_will_do"),
        tree_id: tree_id.to_string(),
        shared,
        line: paradox::line_of(source, start),
    }
}

fn collect_focus_references(block: &Block) -> Vec<String> {
    block
        .find_all("focus")
        .into_iter()
        .filter_map(|pair| pair.value.as_scalar())
        .map(|scalar| scalar.value())
        .collect()
}

/// Rewrites one focus in place, matched by its current id.
pub fn update(path: &str, focus_id: &str, update: &FocusUpdate) -> AppResult<FocusFile> {
    let file = text_file::read(Path::new(path))?;
    let document = paradox::parse(&file.content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let source = &file.content;
    let focus = find_focus_block(&document.items, focus_id).ok_or_else(|| {
        AppError::message(format!("no focus with id `{focus_id}` exists in {path}"))
    })?;

    // The id is editable, and both of these would corrupt the file: an empty
    // one drops the `id` assignment the game requires, and a duplicate makes
    // every later lookup land on whichever focus comes first.
    let new_id = update.id.trim();
    if new_id.is_empty() {
        return Err(AppError::message("a focus needs an id"));
    }
    if new_id != focus_id && find_focus_pair(&document.items, new_id).is_some() {
        return Err(AppError::message(format!(
            "a focus with id `{new_id}` already exists in this file"
        )));
    }

    let mut editor = BlockEditor::new(source, focus);

    for (key, value) in [
        ("id", &update.id),
        ("icon", &update.icon),
        ("x", &update.x),
        ("y", &update.y),
        ("cost", &update.cost),
        ("relative_position_id", &update.relative_position_id),
    ] {
        editor.set_scalar(key, value);
    }

    for (key, value) in BLOCK_FIELDS.iter().zip([
        &update.available,
        &update.bypass,
        &update.allow_branch,
        &update.completion_reward,
        &update.ai_will_do,
    ]) {
        editor.set_block(key, value);
    }

    set_reference_groups(&mut editor, "prerequisite", &update.prerequisites);

    let exclusive_groups = if update.mutually_exclusive.is_empty() {
        Vec::new()
    } else {
        vec![update.mutually_exclusive.clone()]
    };
    set_reference_groups(&mut editor, "mutually_exclusive", &exclusive_groups);

    let updated = apply_edits(source, editor.finish());
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// Appends a new focus to `tree_id`, or to the only tree in the file when the
/// id is empty.
pub fn add(path: &str, tree_id: &str, focus: &FocusUpdate) -> AppResult<FocusFile> {
    if focus.id.trim().is_empty() {
        return Err(AppError::message("a new focus needs an id"));
    }

    let file = text_file::read(Path::new(path))?;
    let document = paradox::parse(&file.content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    if find_focus_block(&document.items, &focus.id).is_some() {
        return Err(AppError::message(format!(
            "a focus with id `{}` already exists in this file",
            focus.id
        )));
    }

    let source = &file.content;
    let trees: Vec<&Pair> = document.find_all("focus_tree");
    let tree = trees
        .iter()
        .find(|pair| {
            pair.value
                .as_block()
                .and_then(|block| block.scalar("id"))
                .map(|id| id == tree_id)
                .unwrap_or(false)
        })
        .or_else(|| trees.first())
        .ok_or_else(|| AppError::message(format!("{path} has no focus_tree to add a focus to")))?;

    let tree_block = tree
        .value
        .as_block()
        .ok_or_else(|| AppError::message("focus_tree is not a block"))?;

    let indent = child_indent(source, tree_block);
    let newline = detect_newline(source);
    let body = render_focus_body(focus, &format!("{indent}\t"), newline);
    let line = format!("focus = {{{newline}{body}{newline}{indent}}}");

    let updated = apply_edits(source, vec![insert_lines(source, tree_block, &[line])]);
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

pub fn delete(path: &str, focus_id: &str) -> AppResult<FocusFile> {
    let file = text_file::read(Path::new(path))?;
    let document = paradox::parse(&file.content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let source = &file.content;
    let pair = find_focus_pair(&document.items, focus_id).ok_or_else(|| {
        AppError::message(format!("no focus with id `{focus_id}` exists in {path}"))
    })?;

    let updated = apply_edits(source, remove_pair(source, pair));
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// Writes a repeated `key = { focus = ... }` field, reusing the blocks that are
/// already there so untouched groups keep their formatting.
fn set_reference_groups(editor: &mut BlockEditor<'_>, key: &str, groups: &[Vec<String>]) {
    let source = editor.source();
    let Some(block) = editor.block() else {
        return;
    };

    let wanted: Vec<Vec<String>> = groups
        .iter()
        .map(|group| normalise_ids(group))
        .filter(|group| !group.is_empty())
        .collect();
    let existing = editor.find_all(key);
    let newline = detect_newline(source);

    for (group, pair) in wanted.iter().zip(existing.iter()) {
        // A block already listing exactly these focuses is left alone, so its
        // comments and spacing survive an edit to some unrelated field.
        let unchanged = pair
            .value
            .as_block()
            .is_some_and(|current| collect_focus_references(current) == *group);
        if unchanged {
            continue;
        }

        let indent = indent_at(source, pair.span.start);
        editor.edit(TextEdit::new(
            pair.value.span(),
            format_block(&render_references(group), &indent, newline),
        ));
    }

    for pair in existing.iter().skip(wanted.len()) {
        editor.remove(pair);
    }

    if wanted.len() > existing.len() {
        let indent = child_indent(source, block);
        for group in &wanted[existing.len()..] {
            editor.add_line(format!(
                "{key} = {}",
                format_block(&render_references(group), &indent, newline)
            ));
        }
    }
}

/// Trims the ids a form sends and drops the blanks.
fn normalise_ids(ids: &[String]) -> Vec<String> {
    ids.iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect()
}

fn render_references(ids: &[String]) -> String {
    ids.iter()
        .map(|id| format!("focus = {id}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_focus_body(focus: &FocusUpdate, indent: &str, newline: &str) -> String {
    let mut lines = Vec::new();

    for (key, value) in [
        ("id", &focus.id),
        ("icon", &focus.icon),
        ("x", &focus.x),
        ("y", &focus.y),
        ("relative_position_id", &focus.relative_position_id),
        ("cost", &focus.cost),
    ] {
        if !value.trim().is_empty() {
            lines.push(format!("{indent}{key} = {}", value.trim()));
        }
    }

    for group in focus.prerequisites.iter().map(|group| normalise_ids(group)) {
        if group.is_empty() {
            continue;
        }
        lines.push(format!(
            "{indent}prerequisite = {}",
            format_block(&render_references(&group), indent, newline)
        ));
    }

    let exclusive = normalise_ids(&focus.mutually_exclusive);
    if !exclusive.is_empty() {
        lines.push(format!(
            "{indent}mutually_exclusive = {}",
            format_block(&render_references(&exclusive), indent, newline)
        ));
    }

    for (key, value) in BLOCK_FIELDS.iter().zip([
        &focus.available,
        &focus.bypass,
        &focus.allow_branch,
        &focus.completion_reward,
        &focus.ai_will_do,
    ]) {
        if !value.trim().is_empty() {
            lines.push(format!(
                "{indent}{key} = {}",
                format_block(value, indent, newline)
            ));
        }
    }

    lines.join(newline)
}

/// Finds the block of the focus with `focus_id`, searching trees and shared
/// focuses alike.
fn find_focus_block<'a>(items: &'a [Item], focus_id: &str) -> Option<&'a Block> {
    find_focus_pair(items, focus_id).and_then(|pair| pair.value.as_block())
}

fn find_focus_pair<'a>(items: &'a [Item], focus_id: &str) -> Option<&'a Pair> {
    for item in items {
        let Item::Pair(pair) = item else {
            continue;
        };

        let is_focus = matches!(pair.key.to_lowercase().as_str(), "focus" | "shared_focus");

        if let Value::Block(block) = &pair.value {
            if is_focus && block.scalar("id").as_deref() == Some(focus_id) {
                return Some(pair);
            }

            // `focus` blocks live inside `focus_tree`, so keep descending.
            if !is_focus {
                if let Some(found) = find_focus_pair(&block.items, focus_id) {
                    return Some(found);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const TREE: &str = "focus_tree = {\n\tid = test_tree\n\tcountry = {\n\t\tfactor = 0\n\t}\n\n\tfocus = {\n\t\tid = alpha\n\t\ticon = GFX_goal_a\n\t\tx = 0\n\t\ty = 0\n\t\tcost = 10\n\t\tcompletion_reward = {\n\t\t\tadd_political_power = 100\n\t\t}\n\t}\n\n\tfocus = {\n\t\tid = beta\n\t\ticon = GFX_goal_b\n\t\tx = 2\n\t\ty = 1\n\t\tcost = 5\n\t\tprerequisite = { focus = alpha }\n\t\tmutually_exclusive = { focus = gamma }\n\t}\n}\n";

    fn write_temp(name: &str, content: &str) -> String {
        let directory = std::env::temp_dir().join("hoi4ms-focus-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join(name);
        std::fs::write(&path, content).expect("write");
        path.display().to_string()
    }

    #[test]
    fn reads_a_tree_and_its_focuses() {
        let path = write_temp("read.txt", TREE);
        let file = read(&path).expect("reads");

        assert_eq!(file.trees.len(), 1);
        assert_eq!(file.trees[0].id, "test_tree");
        assert_eq!(file.trees[0].focus_count, 2);
        assert_eq!(file.focuses.len(), 2);

        let beta = &file.focuses[1];
        assert_eq!(beta.id, "beta");
        assert_eq!(beta.prerequisites, vec![vec!["alpha".to_string()]]);
        assert_eq!(beta.mutually_exclusive, vec!["gamma".to_string()]);
        assert_eq!(
            file.focuses[0].completion_reward,
            "add_political_power = 100"
        );
    }

    #[test]
    fn updating_one_focus_leaves_the_other_untouched() {
        let path = write_temp("update.txt", TREE);
        let mut change = FocusUpdate {
            id: "alpha".into(),
            icon: "GFX_goal_a".into(),
            x: "4".into(),
            y: "3".into(),
            cost: "7".into(),
            completion_reward: "add_political_power = 250".into(),
            ..Default::default()
        };
        change.prerequisites = vec![vec!["beta".into()]];

        let file = update(&path, "alpha", &change).expect("updates");
        let alpha = file
            .focuses
            .iter()
            .find(|focus| focus.id == "alpha")
            .expect("alpha survives");

        assert_eq!(alpha.x, "4");
        assert_eq!(alpha.y, "3");
        assert_eq!(alpha.cost, "7");
        assert_eq!(alpha.prerequisites, vec![vec!["beta".to_string()]]);
        assert_eq!(alpha.completion_reward, "add_political_power = 250");

        let beta = file
            .focuses
            .iter()
            .find(|focus| focus.id == "beta")
            .expect("beta survives");
        assert_eq!(beta.cost, "5");
        assert_eq!(beta.mutually_exclusive, vec!["gamma".to_string()]);
    }

    #[test]
    fn filling_in_several_missing_fields_writes_all_of_them() {
        let path = write_temp(
            "missing.txt",
            "focus_tree = {\n\tid = test_tree\n\tfocus = {\n\t\tid = alpha\n\t}\n}\n",
        );
        let change = FocusUpdate {
            id: "alpha".into(),
            icon: "GFX_goal_a".into(),
            x: "3".into(),
            y: "4".into(),
            cost: "10".into(),
            completion_reward: "add_political_power = 60".into(),
            ..Default::default()
        };

        let file = update(&path, "alpha", &change).expect("updates");
        let alpha = &file.focuses[0];

        assert_eq!(alpha.icon, "GFX_goal_a");
        assert_eq!(alpha.x, "3");
        assert_eq!(alpha.y, "4");
        assert_eq!(alpha.cost, "10");
        assert_eq!(alpha.completion_reward, "add_political_power = 60");
    }

    #[test]
    fn leaves_an_unchanged_prerequisite_block_alone() {
        let path = write_temp(
            "prerequisite.txt",
            "focus_tree = {\n\tid = test_tree\n\tfocus = {\n\t\tid = beta\n\t\tcost = 5\n\t\tprerequisite = {\n\t\t\t# the army branch\n\t\t\tfocus = alpha\n\t\t}\n\t}\n}\n",
        );
        let file = read(&path).expect("reads");
        let beta = file
            .focuses
            .iter()
            .find(|focus| focus.id == "beta")
            .expect("beta");

        let change = FocusUpdate {
            id: beta.id.clone(),
            cost: "9".into(),
            prerequisites: beta.prerequisites.clone(),
            ..Default::default()
        };
        update(&path, "beta", &change).expect("updates");

        let saved = std::fs::read_to_string(&path).expect("read back");
        assert!(
            saved.contains("# the army branch"),
            "comment lost:\n{saved}"
        );
        assert!(saved.contains("cost = 9"));
    }

    #[test]
    fn keeps_windows_1252_bytes_when_saving_a_focus() {
        let path = write_temp("legacy.txt", "");
        // 0xE9 is `é` in Windows-1252 and not valid UTF-8 on its own.
        std::fs::write(
            &path,
            b"focus_tree = {\n\tid = t\n\tfocus = {\n\t\tid = caf\xE9_focus\n\t\tcost = 5\n\t}\n}\n",
        )
        .expect("write");

        let change = FocusUpdate {
            id: "caf\u{E9}_focus".into(),
            cost: "7".into(),
            ..Default::default()
        };
        update(&path, "caf\u{E9}_focus", &change).expect("updates");

        let bytes = std::fs::read(&path).expect("read back");
        assert!(
            bytes.windows(2).any(|pair| pair == [0xE9, b'_']),
            "the Windows-1252 byte was re-encoded"
        );
    }

    #[test]
    fn clearing_a_field_removes_it() {
        let path = write_temp("clear.txt", TREE);
        let change = FocusUpdate {
            id: "beta".into(),
            icon: "GFX_goal_b".into(),
            x: "2".into(),
            y: "1".into(),
            cost: "5".into(),
            ..Default::default()
        };

        let file = update(&path, "beta", &change).expect("updates");
        let beta = file
            .focuses
            .iter()
            .find(|focus| focus.id == "beta")
            .expect("beta survives");

        assert!(beta.prerequisites.is_empty());
        assert!(beta.mutually_exclusive.is_empty());
    }

    #[test]
    fn adds_and_deletes_a_focus() {
        let path = write_temp("add.txt", TREE);
        let new_focus = FocusUpdate {
            id: "gamma".into(),
            icon: "GFX_goal_c".into(),
            x: "5".into(),
            y: "2".into(),
            cost: "10".into(),
            prerequisites: vec![vec!["beta".into()]],
            completion_reward: "add_stability = 0.05".into(),
            ..Default::default()
        };

        let file = add(&path, "test_tree", &new_focus).expect("adds");
        assert_eq!(file.focuses.len(), 3);

        let gamma = file
            .focuses
            .iter()
            .find(|focus| focus.id == "gamma")
            .expect("gamma exists");
        assert_eq!(gamma.x, "5");
        assert_eq!(gamma.prerequisites, vec![vec!["beta".to_string()]]);
        assert_eq!(gamma.completion_reward, "add_stability = 0.05");

        let after_delete = delete(&path, "gamma").expect("deletes");
        assert_eq!(after_delete.focuses.len(), 2);
        assert!(after_delete.focuses.iter().all(|focus| focus.id != "gamma"));
    }

    #[test]
    fn refuses_to_clear_an_id_on_update() {
        let path = write_temp("empty-id.txt", TREE);
        let change = FocusUpdate {
            id: "   ".into(),
            cost: "10".into(),
            ..Default::default()
        };

        let error = update(&path, "alpha", &change).expect_err("an empty id is refused");
        assert!(error.to_string().contains("needs an id"));

        // The file is left exactly as it was.
        assert_eq!(std::fs::read_to_string(&path).expect("read back"), TREE);
    }

    #[test]
    fn refuses_to_rename_onto_another_focus() {
        let path = write_temp("collide.txt", TREE);
        let change = FocusUpdate {
            id: "beta".into(),
            cost: "10".into(),
            ..Default::default()
        };

        let error = update(&path, "alpha", &change).expect_err("a duplicate id is refused");
        assert!(error.to_string().contains("already exists"));
        assert_eq!(std::fs::read_to_string(&path).expect("read back"), TREE);
    }

    #[test]
    fn keeping_the_same_id_is_not_a_collision() {
        let path = write_temp("same-id.txt", TREE);
        let change = FocusUpdate {
            id: "alpha".into(),
            cost: "42".into(),
            ..Default::default()
        };

        let file = update(&path, "alpha", &change).expect("updates");
        let alpha = file
            .focuses
            .iter()
            .find(|focus| focus.id == "alpha")
            .expect("alpha");
        assert_eq!(alpha.cost, "42");
    }

    #[test]
    fn rejects_a_duplicate_id() {
        let path = write_temp("duplicate.txt", TREE);
        let duplicate = FocusUpdate {
            id: "alpha".into(),
            ..Default::default()
        };

        assert!(add(&path, "test_tree", &duplicate).is_err());
    }

    #[test]
    fn reads_shared_focuses() {
        let path = write_temp(
            "shared.txt",
            "shared_focus = {\n\tid = shared_one\n\tx = 1\n\ty = 1\n\tcost = 10\n}\n",
        );
        let file = read(&path).expect("reads");

        assert_eq!(file.focuses.len(), 1);
        assert!(file.focuses[0].shared);
        assert_eq!(file.focuses[0].id, "shared_one");
    }
}
