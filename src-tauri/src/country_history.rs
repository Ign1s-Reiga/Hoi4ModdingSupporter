//! Starting country setup: `history/countries/TAG - Name.txt`.
//!
//! Unlike a focus or state file there is no wrapper block — the assignments sit
//! at the top level, mixed in with `if` blocks, technology lists and character
//! definitions. Only the handful of fields below are touched; everything else
//! is left exactly where the author put it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paradox::edit::{apply_edits, BlockEditor};
use crate::paradox::{self, Block, Items};
use crate::text_file;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Politics {
    pub ruling_party: String,
    pub last_election: String,
    pub election_frequency: String,
    /// `yes`, `no`, or empty when the file does not say.
    pub elections_allowed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Popularity {
    pub ideology: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountryHistory {
    pub path: String,
    /// Country tag taken from the file name, e.g. `ENG`.
    pub tag: String,
    pub file_name: String,
    pub capital: String,
    pub oob: String,
    pub research_slots: String,
    pub convoys: String,
    pub stability: String,
    pub war_support: String,
    pub politics: Politics,
    pub popularities: Vec<Popularity>,
    pub has_bom: bool,
    pub encoding: text_file::Encoding,
    /// Blocks the file never closed. One of the files the game ships is like
    /// this and the game loads it anyway, so it is a warning, not an error.
    pub unclosed_blocks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountryHistoryInfo {
    pub path: String,
    pub relative_path: String,
    pub tag: String,
    pub file_name: String,
}

/// The editable fields sent back when saving. Empty strings clear a field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountryHistoryUpdate {
    pub capital: String,
    pub oob: String,
    pub research_slots: String,
    pub convoys: String,
    pub stability: String,
    pub war_support: String,
    pub politics: Politics,
    pub popularities: Vec<Popularity>,
}

pub fn read(path: &str) -> AppResult<CountryHistory> {
    let file = text_file::read(Path::new(path))?;
    let document = paradox::parse(&file.content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let file_name = Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();

    let politics = document
        .block("set_politics")
        .map(|block| Politics {
            ruling_party: block.scalar("ruling_party").unwrap_or_default(),
            last_election: block.scalar("last_election").unwrap_or_default(),
            election_frequency: block.scalar("election_frequency").unwrap_or_default(),
            elections_allowed: block.scalar("elections_allowed").unwrap_or_default(),
        })
        .unwrap_or_default();

    let popularities = document
        .block("set_popularities")
        .map(read_popularities)
        .unwrap_or_default();

    Ok(CountryHistory {
        path: path.replace('\\', "/"),
        tag: tag_of(&file_name),
        file_name,
        capital: document.scalar("capital").unwrap_or_default(),
        oob: document.scalar("oob").unwrap_or_default(),
        research_slots: document.scalar("set_research_slots").unwrap_or_default(),
        convoys: document.scalar("set_convoys").unwrap_or_default(),
        stability: document.scalar("set_stability").unwrap_or_default(),
        war_support: document.scalar("set_war_support").unwrap_or_default(),
        politics,
        popularities,
        has_bom: file.has_bom,
        encoding: file.encoding,
        unclosed_blocks: document.unclosed_blocks,
    })
}

fn read_popularities(block: &Block) -> Vec<Popularity> {
    block
        .pairs()
        .into_iter()
        .filter_map(|pair| {
            pair.value.as_scalar().map(|scalar| Popularity {
                ideology: pair.key.clone(),
                value: scalar.value(),
            })
        })
        .collect()
}

/// `ENG - Britain` -> `ENG`. Files that do not follow the convention keep the
/// whole stem, which is still a usable label.
fn tag_of(file_name: &str) -> String {
    file_name
        .split(['-', ' '])
        .next()
        .unwrap_or(file_name)
        .trim()
        .to_string()
}

pub fn update(path: &str, update: &CountryHistoryUpdate) -> AppResult<CountryHistory> {
    let file = text_file::read(Path::new(path))?;
    let document = paradox::parse(&file.content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let source = &file.content;
    let mut edits = Vec::new();

    let mut editor = BlockEditor::for_document(source, &document);
    for (key, value) in [
        ("capital", &update.capital),
        ("oob", &update.oob),
        ("set_research_slots", &update.research_slots),
        ("set_convoys", &update.convoys),
        ("set_stability", &update.stability),
        ("set_war_support", &update.war_support),
    ] {
        editor.set_scalar(key, value);
    }

    // The two blocks are edited key by key when they exist, so comments such as
    // the note HOI4 keeps beside `elections_allowed` are not swept away.
    match document.block("set_politics") {
        Some(block) => {
            let mut politics = BlockEditor::new(source, block);
            politics.set_scalar("ruling_party", &update.politics.ruling_party);
            politics.set_scalar("last_election", &update.politics.last_election);
            politics.set_scalar("election_frequency", &update.politics.election_frequency);
            politics.set_scalar("elections_allowed", &update.politics.elections_allowed);
            edits.extend(politics.finish());
        }
        None => {
            let body = render_politics(&update.politics);
            if !body.is_empty() {
                editor.set_block("set_politics", &body);
            }
        }
    }

    match document.block("set_popularities") {
        Some(block) => edits.extend(popularity_edits(source, block, &update.popularities)),
        None => {
            let body = render_popularities(&update.popularities);
            if !body.is_empty() {
                editor.set_block("set_popularities", &body);
            }
        }
    }

    edits.extend(editor.finish());

    let updated = apply_edits(source, edits);
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// Sets each ideology in place and drops the ones no longer listed.
fn popularity_edits(
    source: &str,
    block: &Block,
    popularities: &[Popularity],
) -> Vec<crate::paradox::edit::TextEdit> {
    let mut editor = BlockEditor::new(source, block);

    for popularity in popularities {
        let ideology = popularity.ideology.trim();
        if ideology.is_empty() {
            continue;
        }
        editor.set_scalar(ideology, &popularity.value);
    }

    let wanted: Vec<String> = popularities
        .iter()
        .map(|popularity| popularity.ideology.trim().to_lowercase())
        .filter(|ideology| !ideology.is_empty())
        .collect();

    for pair in block.pairs() {
        if !wanted.contains(&pair.key.to_lowercase()) {
            editor.remove(pair);
        }
    }

    editor.finish()
}

fn render_politics(politics: &Politics) -> String {
    let mut lines = Vec::new();

    for (key, value) in [
        ("ruling_party", &politics.ruling_party),
        ("last_election", &politics.last_election),
        ("election_frequency", &politics.election_frequency),
        ("elections_allowed", &politics.elections_allowed),
    ] {
        if !value.trim().is_empty() {
            lines.push(format!("{key} = {}", value.trim()));
        }
    }

    lines.join("\n")
}

fn render_popularities(popularities: &[Popularity]) -> String {
    popularities
        .iter()
        .filter(|popularity| !popularity.ideology.trim().is_empty())
        .map(|popularity| {
            format!(
                "{} = {}",
                popularity.ideology.trim(),
                popularity.value.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Lists the country files of a mod without reading them in full.
pub fn scan(folder: &str) -> AppResult<Vec<CountryHistoryInfo>> {
    let root = Path::new(folder).join("history").join("countries");
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("txt") {
            continue;
        }

        let file_name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();

        files.push(CountryHistoryInfo {
            path: crate::project::normalise_path(path),
            relative_path: path
                .strip_prefix(Path::new(folder))
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/"),
            tag: tag_of(&file_name),
            file_name,
        });
    }

    files.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape the game ships, byte order mark aside.
    const COUNTRY: &str = "capital = 126 #London\n\noob = \"ENG_1936\"\n\nset_research_slots = 4\nset_stability = 0.6\nset_war_support = 0.1\nset_convoys = 1000\n\nset_technology = {\n\tinfantry_weapons = 1\n}\n\nset_politics = {\n\truling_party = democratic\n\tlast_election = \"1935.11.14\"\n\telection_frequency = 48\n\telections_allowed = yes ##suspended during war\n}\n\nset_popularities = {\n\tdemocratic = 97\n\tfascism = 2\n\tcommunism = 1\n}\n\nif = {\n\tlimit = {\n\t\tNOT = { has_dlc = \"By Blood Alone\" }\n\t}\n\tset_technology = { early_fighter = 1 }\n}\n";

    fn write_temp(name: &str, content: &str) -> String {
        let directory = std::env::temp_dir().join("hoi4ms-country-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join(name);
        std::fs::write(&path, content).expect("write");
        path.display().to_string()
    }

    #[test]
    fn reads_the_fields_it_edits() {
        let path = write_temp("ENG - Britain.txt", COUNTRY);
        let country = read(&path).expect("reads");

        assert_eq!(country.tag, "ENG");
        assert_eq!(country.capital, "126");
        assert_eq!(country.oob, "ENG_1936");
        assert_eq!(country.research_slots, "4");
        assert_eq!(country.convoys, "1000");
        assert_eq!(country.stability, "0.6");
        assert_eq!(country.war_support, "0.1");
        assert_eq!(country.politics.ruling_party, "democratic");
        assert_eq!(country.politics.last_election, "1935.11.14");
        assert_eq!(country.politics.elections_allowed, "yes");
        assert_eq!(country.popularities.len(), 3);
        assert_eq!(country.popularities[0].ideology, "democratic");
        assert_eq!(country.popularities[0].value, "97");
    }

    #[test]
    fn editing_one_field_leaves_the_rest_of_the_file_alone() {
        let path = write_temp("edit.txt", COUNTRY);
        let country = read(&path).expect("reads");

        let mut change = update_from(&country);
        change.capital = "127".into();
        let saved = update(&path, &change).expect("updates");

        assert_eq!(saved.capital, "127");

        let text = std::fs::read_to_string(&path).expect("read back");
        assert!(text.contains("#London"), "trailing comment lost:\n{text}");
        assert!(
            text.contains("##suspended during war"),
            "politics comment lost"
        );
        assert!(
            text.contains("has_dlc = \"By Blood Alone\""),
            "if block disturbed"
        );
        assert!(
            text.contains("infantry_weapons = 1"),
            "technology disturbed"
        );
    }

    #[test]
    fn politics_and_popularities_are_edited_in_place() {
        let path = write_temp("politics.txt", COUNTRY);
        let country = read(&path).expect("reads");

        let mut change = update_from(&country);
        change.politics.ruling_party = "fascism".into();
        change.popularities = vec![
            Popularity {
                ideology: "democratic".into(),
                value: "20".into(),
            },
            Popularity {
                ideology: "fascism".into(),
                value: "80".into(),
            },
        ];

        let saved = update(&path, &change).expect("updates");

        assert_eq!(saved.politics.ruling_party, "fascism");
        assert_eq!(saved.popularities.len(), 2);
        assert_eq!(saved.popularities[1].value, "80");

        // The dropped ideology is gone, the kept comment is not.
        let text = std::fs::read_to_string(&path).expect("read back");
        assert!(!text.contains("communism ="));
        assert!(text.contains("##suspended during war"));
    }

    #[test]
    fn a_missing_field_is_appended() {
        let path = write_temp("sparse.txt", "capital = 1\n");
        let country = read(&path).expect("reads");

        let mut change = update_from(&country);
        change.stability = "0.4".into();
        change.politics.ruling_party = "neutrality".into();

        let saved = update(&path, &change).expect("updates");
        assert_eq!(saved.stability, "0.4");
        assert_eq!(saved.politics.ruling_party, "neutrality");
    }

    #[test]
    fn a_no_op_save_changes_nothing() {
        let path = write_temp("noop.txt", COUNTRY);
        let country = read(&path).expect("reads");

        update(&path, &update_from(&country)).expect("updates");

        assert_eq!(std::fs::read_to_string(&path).expect("read back"), COUNTRY);
    }

    fn update_from(country: &CountryHistory) -> CountryHistoryUpdate {
        CountryHistoryUpdate {
            capital: country.capital.clone(),
            oob: country.oob.clone(),
            research_slots: country.research_slots.clone(),
            convoys: country.convoys.clone(),
            stability: country.stability.clone(),
            war_support: country.war_support.clone(),
            politics: country.politics.clone(),
            popularities: country.popularities.clone(),
        }
    }
}
