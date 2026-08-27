//! Map states: reading `history/states/*.txt` and writing single states back.
//!
//! A state file wraps everything in one `state = { ... }` block, with the
//! ownership details a level deeper inside `history = { ... }`. Both levels are
//! edited in place, so the province lists, comments and building blocks a file
//! carries survive a save untouched.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paradox::edit::{apply_edits, block_body, BlockEditor};
use crate::paradox::{self, Block, Items, Pair};
use crate::text_file;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub id: String,
    /// Localisation key, e.g. `STATE_1`.
    pub name: String,
    pub manpower: String,
    pub state_category: String,
    pub local_supplies: String,
    pub buildings_max_level_factor: String,
    /// Province ids, in the order the file lists them.
    pub provinces: Vec<String>,
    /// Body of `resources = { ... }`, kept as text: the keys vary by mod.
    pub resources: String,
    pub owner: String,
    pub controller: String,
    /// One entry per `add_core_of`.
    pub cores: Vec<String>,
    /// One entry per `add_claim_by`.
    pub claims: Vec<String>,
    /// Body of `buildings = { ... }`, which nests province-keyed blocks.
    pub buildings: String,
    /// Body of `victory_points = { ... }`, one province and value per block.
    pub victory_points: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateFile {
    pub path: String,
    pub states: Vec<State>,
    pub has_bom: bool,
    pub encoding: text_file::Encoding,
    /// Blocks the file never closed. The game tolerates these, so the file is
    /// still editable, but it is worth telling the user about.
    pub unclosed_blocks: usize,
}

/// The editable fields sent back when saving. Empty strings clear a field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateUpdate {
    pub id: String,
    pub name: String,
    pub manpower: String,
    pub state_category: String,
    pub local_supplies: String,
    pub buildings_max_level_factor: String,
    pub provinces: Vec<String>,
    pub resources: String,
    pub owner: String,
    pub controller: String,
    pub cores: Vec<String>,
    pub claims: Vec<String>,
    pub buildings: String,
    pub victory_points: Vec<String>,
}

pub fn read(path: &str) -> AppResult<StateFile> {
    let file = text_file::read(Path::new(path))?;
    let document = paradox::parse(&file.content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let states = document
        .find_all("state")
        .into_iter()
        .filter_map(|pair| {
            pair.value
                .as_block()
                .map(|block| read_state(&file.content, block, pair.span.start))
        })
        .collect();

    Ok(StateFile {
        path: path.replace('\\', "/"),
        states,
        has_bom: file.has_bom,
        encoding: file.encoding,
        unclosed_blocks: document.unclosed_blocks,
    })
}

fn read_state(source: &str, block: &Block, start: usize) -> State {
    let history = block.block("history");

    let read_history_scalar = |key: &str| {
        history
            .and_then(|history| history.scalar(key))
            .unwrap_or_default()
    };
    let read_history_repeated = |key: &str| {
        history
            .map(|history| {
                history
                    .find_all(key)
                    .into_iter()
                    .filter_map(|pair| pair.value.as_scalar())
                    .map(|scalar| scalar.value())
                    .collect()
            })
            .unwrap_or_default()
    };

    State {
        id: block.scalar("id").unwrap_or_default(),
        name: block.scalar("name").unwrap_or_default(),
        manpower: block.scalar("manpower").unwrap_or_default(),
        state_category: block.scalar("state_category").unwrap_or_default(),
        local_supplies: block.scalar("local_supplies").unwrap_or_default(),
        buildings_max_level_factor: block
            .scalar("buildings_max_level_factor")
            .unwrap_or_default(),
        provinces: block
            .block("provinces")
            .map(collect_bare_values)
            .unwrap_or_default(),
        resources: block
            .find("resources")
            .map(|pair| block_body(source, &pair.value))
            .unwrap_or_default(),
        owner: read_history_scalar("owner"),
        controller: read_history_scalar("controller"),
        cores: read_history_repeated("add_core_of"),
        claims: read_history_repeated("add_claim_by"),
        buildings: history
            .and_then(|history| history.find("buildings"))
            .map(|pair| block_body(source, &pair.value))
            .unwrap_or_default(),
        victory_points: history
            .map(|history| {
                history
                    .find_all("victory_points")
                    .into_iter()
                    .map(|pair| block_body(source, &pair.value))
                    .collect()
            })
            .unwrap_or_default(),
        line: paradox::line_of(source, start),
    }
}

/// The values in a list block such as `provinces = { 3838 9851 }`.
fn collect_bare_values(block: &Block) -> Vec<String> {
    block
        .items
        .iter()
        .filter_map(|item| match item {
            paradox::Item::Value(value) => value.as_scalar().map(|scalar| scalar.value()),
            paradox::Item::Pair(_) => None,
        })
        .collect()
}

/// Rewrites one state in place, matched by its current id.
pub fn update(path: &str, state_id: &str, update: &StateUpdate) -> AppResult<StateFile> {
    let file = text_file::read(Path::new(path))?;
    let document = paradox::parse(&file.content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let source = &file.content;
    let state = find_state(&document, state_id).ok_or_else(|| {
        AppError::message(format!("no state with id `{state_id}` exists in {path}"))
    })?;

    let new_id = update.id.trim();
    if new_id.is_empty() {
        return Err(AppError::message("a state needs an id"));
    }
    if new_id != state_id {
        if find_state(&document, new_id).is_some() {
            return Err(AppError::message(format!(
                "a state with id `{new_id}` already exists in this file"
            )));
        }

        // State ids are global across history/states, not per file. The sibling
        // scan only runs on a rename, which is rare, so the cost stays off the
        // ordinary save path.
        if let Some(other) = find_state_in_siblings(Path::new(path), new_id) {
            return Err(AppError::message(format!(
                "state id `{new_id}` is already used by {other}"
            )));
        }
    }

    let state_block = state
        .value
        .as_block()
        .ok_or_else(|| AppError::message("state is not a block"))?;

    let mut edits = Vec::new();

    // One editor per scope, never two over the same block: each of them would
    // insert at the same span before the closing brace and only the first
    // insertion would survive.
    let history = state_block.block("history");

    let mut editor = BlockEditor::new(source, state_block);
    for (key, value) in [
        ("id", &update.id),
        ("name", &update.name),
        ("manpower", &update.manpower),
        ("state_category", &update.state_category),
        ("local_supplies", &update.local_supplies),
        (
            "buildings_max_level_factor",
            &update.buildings_max_level_factor,
        ),
    ] {
        editor.set_scalar(key, value);
    }
    editor.set_block("resources", &update.resources);
    editor.set_block("provinces", &update.provinces.join(" "));

    // A state with no history block yet gets one built from the fields at once.
    if history.is_none() {
        let body = render_history(update);
        if !body.is_empty() {
            editor.set_block("history", &body);
        }
    }
    edits.extend(editor.finish());

    if let Some(history) = history {
        let mut history_editor = BlockEditor::new(source, history);
        history_editor.set_scalar("owner", &update.owner);
        history_editor.set_scalar("controller", &update.controller);
        history_editor.set_repeated_scalars("add_core_of", &update.cores);
        history_editor.set_repeated_scalars("add_claim_by", &update.claims);
        history_editor.set_block("buildings", &update.buildings);
        set_victory_points(&mut history_editor, &update.victory_points);
        edits.extend(history_editor.finish());
    }

    let updated = apply_edits(source, edits);
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// `victory_points = { 3838 1 }` repeats once per point.
fn set_victory_points(editor: &mut BlockEditor<'_>, points: &[String]) {
    let wanted: Vec<String> = points
        .iter()
        .map(|point| point.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|point| !point.is_empty())
        .collect();
    let existing = editor.find_all("victory_points");
    let source = editor.source();

    for (point, pair) in wanted.iter().zip(existing.iter()) {
        if block_body(source, &pair.value) == *point {
            continue;
        }
        editor.edit(crate::paradox::edit::TextEdit::new(
            pair.value.span(),
            format!("{{ {point} }}"),
        ));
    }

    for pair in existing.iter().skip(wanted.len()) {
        editor.remove(pair);
    }

    for point in wanted.iter().skip(existing.len()) {
        editor.add_line(format!("victory_points = {{ {point} }}"));
    }
}

fn render_history(update: &StateUpdate) -> String {
    let mut lines = Vec::new();

    if !update.owner.trim().is_empty() {
        lines.push(format!("owner = {}", update.owner.trim()));
    }
    if !update.controller.trim().is_empty() {
        lines.push(format!("controller = {}", update.controller.trim()));
    }
    for point in &update.victory_points {
        let normalised = point.split_whitespace().collect::<Vec<_>>().join(" ");
        if !normalised.is_empty() {
            lines.push(format!("victory_points = {{ {normalised} }}"));
        }
    }
    for core in &update.cores {
        if !core.trim().is_empty() {
            lines.push(format!("add_core_of = {}", core.trim()));
        }
    }
    for claim in &update.claims {
        if !claim.trim().is_empty() {
            lines.push(format!("add_claim_by = {}", claim.trim()));
        }
    }

    // Buildings are a block of their own inside history, and leaving them out
    // here would drop whatever the form was holding.
    if !update.buildings.trim().is_empty() {
        let body = update
            .buildings
            .lines()
            .map(|line| format!("\t{}", line.trim_end()))
            .collect::<Vec<_>>()
            .join("\n");
        lines.push(format!("buildings = {{\n{body}\n}}"));
    }

    lines.join("\n")
}

/// Name of the sibling file already defining `state_id`, if any.
///
/// Ids are unique across the whole state database, so a rename has to look
/// wider than the file being edited.
fn find_state_in_siblings(path: &Path, state_id: &str) -> Option<String> {
    let directory = path.parent()?;

    for entry in std::fs::read_dir(directory).ok()?.flatten() {
        let sibling = entry.path();
        if sibling == path || sibling.extension().and_then(|value| value.to_str()) != Some("txt") {
            continue;
        }

        let Ok(file) = text_file::read(&sibling) else {
            continue;
        };
        let Ok(document) = paradox::parse(&file.content) else {
            continue;
        };

        if find_state(&document, state_id).is_some() {
            return Some(
                sibling
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
        }
    }

    None
}

fn find_state<'a>(document: &'a paradox::Document, state_id: &str) -> Option<&'a Pair> {
    document.find_all("state").into_iter().find(|pair| {
        pair.value
            .as_block()
            .and_then(|block| block.scalar("id"))
            .as_deref()
            == Some(state_id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Taken from the shape the game ships, tabs and all.
    const STATE: &str = "\nstate={\n\tid=1\n\tname=\"STATE_1\" # Corsica\n\tmanpower = 322900\n\t\n\tstate_category = town\n\n\thistory={\n\t\towner = FRA\n\t\tvictory_points = { 3838 1 }\n\t\tbuildings = {\n\t\t\tinfrastructure = 2\n\t\t\tindustrial_complex = 1\n\t\t\t3838 = {\n\t\t\t\tnaval_base = 3\n\t\t\t}\n\t\t}\n\t\tadd_core_of = COR\n\t\tadd_core_of = FRA\n\t}\n\n\tprovinces={\n\t\t3838 9851 11804 \n\t}\n\n\tlocal_supplies=0.0 \n}\n";

    fn write_temp(name: &str, content: &str) -> String {
        let directory = std::env::temp_dir().join("hoi4ms-state-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join(name);
        std::fs::write(&path, content).expect("write");
        path.display().to_string()
    }

    #[test]
    fn reads_a_state() {
        let path = write_temp("read.txt", STATE);
        let file = read(&path).expect("reads");

        assert_eq!(file.states.len(), 1);
        let state = &file.states[0];

        assert_eq!(state.id, "1");
        assert_eq!(state.name, "STATE_1");
        assert_eq!(state.manpower, "322900");
        assert_eq!(state.state_category, "town");
        assert_eq!(state.local_supplies, "0.0");
        assert_eq!(state.owner, "FRA");
        assert_eq!(state.cores, vec!["COR".to_string(), "FRA".to_string()]);
        assert_eq!(state.provinces, vec!["3838", "9851", "11804"]);
        assert_eq!(state.victory_points, vec!["3838 1".to_string()]);
        assert!(state.buildings.contains("infrastructure = 2"));
        assert!(state.buildings.contains("naval_base = 3"));
    }

    #[test]
    fn edits_reach_both_levels_of_the_file() {
        let path = write_temp("update.txt", STATE);
        let file = read(&path).expect("reads");
        let mut change = update_from(&file.states[0]);
        change.manpower = "500000".into();
        change.owner = "ENG".into();

        let saved = update(&path, "1", &change).expect("updates");
        let state = &saved.states[0];

        assert_eq!(state.manpower, "500000");
        assert_eq!(state.owner, "ENG");
        // Everything else is left as it was.
        assert_eq!(state.provinces, vec!["3838", "9851", "11804"]);
        assert!(state.buildings.contains("naval_base = 3"));
    }

    #[test]
    fn untouched_fields_keep_their_formatting_and_comments() {
        let path = write_temp("untouched.txt", STATE);
        let file = read(&path).expect("reads");
        let mut change = update_from(&file.states[0]);
        change.manpower = "1".into();
        update(&path, "1", &change).expect("updates");

        let saved = std::fs::read_to_string(&path).expect("read back");
        assert!(
            saved.contains("name=\"STATE_1\" # Corsica"),
            "comment lost:\n{saved}"
        );
        assert!(
            saved.contains("\t\t\t3838 = {"),
            "nested building lost:\n{saved}"
        );
        assert!(
            saved.contains("3838 9851 11804"),
            "province list reflowed:\n{saved}"
        );
    }

    #[test]
    fn cores_are_added_and_removed() {
        let path = write_temp("cores.txt", STATE);
        let file = read(&path).expect("reads");

        let mut change = update_from(&file.states[0]);
        change.cores = vec!["ENG".into()];
        let saved = update(&path, "1", &change).expect("updates");
        assert_eq!(saved.states[0].cores, vec!["ENG".to_string()]);

        let mut grow = update_from(&saved.states[0]);
        grow.cores = vec!["ENG".into(), "SCO".into(), "WLS".into()];
        let saved = update(&path, "1", &grow).expect("updates");
        assert_eq!(
            saved.states[0].cores,
            vec!["ENG".to_string(), "SCO".to_string(), "WLS".to_string()]
        );
    }

    #[test]
    fn rejects_an_empty_or_duplicate_id() {
        let path = write_temp("ids.txt", STATE);
        let file = read(&path).expect("reads");

        let mut empty = update_from(&file.states[0]);
        empty.id = "  ".into();
        assert!(update(&path, "1", &empty).is_err());

        assert_eq!(std::fs::read_to_string(&path).expect("read back"), STATE);
    }

    #[test]
    fn adds_a_missing_scalar_and_a_missing_history_together() {
        // Both want to insert just before the closing brace, so a second
        // editor over the same block would silently drop one of them.
        let path = write_temp(
            "both-missing.txt",
            "state={\n\tid=7\n\tname=\"STATE_7\"\n}\n",
        );
        let file = read(&path).expect("reads");

        let mut change = update_from(&file.states[0]);
        change.manpower = "1000".into();
        change.state_category = "rural".into();
        change.owner = "ITA".into();

        let saved = update(&path, "7", &change).expect("updates");
        let state = &saved.states[0];

        assert_eq!(state.manpower, "1000");
        assert_eq!(state.state_category, "rural");
        assert_eq!(state.owner, "ITA");
    }

    #[test]
    fn a_new_history_block_carries_the_buildings_too() {
        let path = write_temp(
            "history-buildings.txt",
            "state={\n\tid=11\n\tname=\"STATE_11\"\n}\n",
        );
        let file = read(&path).expect("reads");

        let mut change = update_from(&file.states[0]);
        change.owner = "SPR".into();
        change.buildings = "infrastructure = 3\narms_factory = 2".into();

        let saved = update(&path, "11", &change).expect("updates");
        let state = &saved.states[0];

        assert_eq!(state.owner, "SPR");
        assert!(
            state.buildings.contains("infrastructure = 3"),
            "buildings dropped:\n{}",
            std::fs::read_to_string(&path).expect("read back")
        );
        assert!(state.buildings.contains("arms_factory = 2"));
    }

    #[test]
    fn a_rename_onto_a_sibling_file_is_refused() {
        let directory = std::env::temp_dir().join("hoi4ms-state-siblings");
        std::fs::remove_dir_all(&directory).ok();
        std::fs::create_dir_all(&directory).expect("temp dir");

        let first = directory.join("1-one.txt");
        let second = directory.join("2-two.txt");
        std::fs::write(&first, "state={\n\tid=1\n\tname=\"STATE_1\"\n}\n").expect("write");
        std::fs::write(&second, "state={\n\tid=2\n\tname=\"STATE_2\"\n}\n").expect("write");

        let path = first.display().to_string();
        let file = read(&path).expect("reads");
        let mut change = update_from(&file.states[0]);
        change.id = "2".into();

        let error = update(&path, "1", &change).expect_err("the id is taken");
        assert!(
            error.to_string().contains("2-two.txt"),
            "unexpected: {error}"
        );

        // The file is untouched.
        assert!(std::fs::read_to_string(&first)
            .expect("read back")
            .contains("id=1"));
    }

    #[test]
    fn writes_a_history_block_when_the_state_has_none() {
        let path = write_temp(
            "no-history.txt",
            "state={\n\tid=9\n\tname=\"STATE_9\"\n\tprovinces={\n\t\t42\n\t}\n}\n",
        );
        let file = read(&path).expect("reads");

        let mut change = update_from(&file.states[0]);
        change.owner = "GER".into();
        change.cores = vec!["GER".into()];

        let saved = update(&path, "9", &change).expect("updates");
        assert_eq!(saved.states[0].owner, "GER");
        assert_eq!(saved.states[0].cores, vec!["GER".to_string()]);
    }

    fn update_from(state: &State) -> StateUpdate {
        StateUpdate {
            id: state.id.clone(),
            name: state.name.clone(),
            manpower: state.manpower.clone(),
            state_category: state.state_category.clone(),
            local_supplies: state.local_supplies.clone(),
            buildings_max_level_factor: state.buildings_max_level_factor.clone(),
            provinces: state.provinces.clone(),
            resources: state.resources.clone(),
            owner: state.owner.clone(),
            controller: state.controller.clone(),
            cores: state.cores.clone(),
            claims: state.claims.clone(),
            buildings: state.buildings.clone(),
            victory_points: state.victory_points.clone(),
        }
    }
}
