//! Technologies: `common/technologies/*.txt`, the research trees.
//!
//! A technology is one `id = { }` block inside `technologies = { }`, placed
//! on one or more tree folders by a `folder = { name position }` block each,
//! and linked forward by `path = { leads_to_tech }` blocks. Rows are often
//! spelled through the file's `@1936 = 2` variables, which the form keeps
//! as written and the canvas resolves. What a technology *does* — the unit
//! and equipment modifiers it unlocks — is open-ended script, so it is
//! handed over as text for the code view rather than guessed at.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paradox::edit::{
    apply_edits, block_body, body_without, child_indent, detect_newline, format_block,
    format_lines, insert_lines, normalise_words, remove_pair, render_words, words_of, BlockEditor,
    TextEdit,
};
use crate::paradox::lexer::{needs_quotes, quote};
use crate::paradox::{self, Block, Document, Items, Pair, Span};
use crate::text_file;

/// Word lists the form edits.
const LISTS: &[&str] = &[
    "categories",
    "xor",
    "sub_technologies",
    "enable_equipments",
    "enable_equipment_modules",
    "enable_subunits",
];

/// Script blocks the form edits as text.
const BLOCKS: &[&str] = &[
    "allow",
    "allow_branch",
    "on_research_complete",
    "ai_will_do",
    "ai_research_weights",
];

/// Scalars the form edits.
const SCALARS: &[&str] = &[
    "research_cost",
    "start_year",
    "doctrine",
    "doctrine_name",
    "xp_research_type",
    "xp_research_cost",
    "xp_research_bonus",
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechFolder {
    pub name: String,
    /// As written: a number, or a `@variable` the file declares.
    pub x: String,
    pub y: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechPath {
    pub leads_to_tech: String,
    pub research_cost_coeff: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    /// Without the `@`.
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Technology {
    pub id: String,
    pub research_cost: String,
    pub start_year: String,
    /// `yes` for a doctrine, else empty.
    pub doctrine: String,
    pub doctrine_name: String,
    pub xp_research_type: String,
    pub xp_research_cost: String,
    pub xp_research_bonus: String,
    pub folders: Vec<TechFolder>,
    pub paths: Vec<TechPath>,
    pub categories: Vec<String>,
    pub xor: Vec<String>,
    pub sub_technologies: Vec<String>,
    pub enable_equipments: Vec<String>,
    pub enable_equipment_modules: Vec<String>,
    pub enable_subunits: Vec<String>,
    pub allow: String,
    pub allow_branch: String,
    pub on_research_complete: String,
    pub ai_will_do: String,
    pub ai_research_weights: String,
    /// Everything else in the block — the modifiers it grants — as script,
    /// for reading beside the form and editing in the code view.
    pub effects: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechnologyFile {
    pub path: String,
    /// The `@name = value` rows the file declares, for placing positions.
    pub variables: Vec<Variable>,
    pub technologies: Vec<Technology>,
    pub has_bom: bool,
    pub encoding: text_file::Encoding,
    /// The text the technologies were read from, for the code view.
    pub source: String,
}

/// The editable fields sent back when saving. Empty strings clear a field;
/// a folder, path or list entry left out is removed. The effects are not
/// here: they are edited in the code view.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechnologyUpdate {
    pub id: String,
    pub research_cost: String,
    pub start_year: String,
    pub doctrine: String,
    pub doctrine_name: String,
    pub xp_research_type: String,
    pub xp_research_cost: String,
    pub xp_research_bonus: String,
    pub folders: Vec<TechFolder>,
    pub paths: Vec<TechPath>,
    pub categories: Vec<String>,
    pub xor: Vec<String>,
    pub sub_technologies: Vec<String>,
    pub enable_equipments: Vec<String>,
    pub enable_equipment_modules: Vec<String>,
    pub enable_subunits: Vec<String>,
    pub allow: String,
    pub allow_branch: String,
    pub on_research_complete: String,
    pub ai_will_do: String,
    pub ai_research_weights: String,
}

pub fn read(path: &str) -> AppResult<TechnologyFile> {
    let file = text_file::read(Path::new(path))?;
    parse(path, file.content, file.has_bom, file.encoding)
}

/// Reads technologies out of text the window holds, which need not be what
/// is on disk: the code view parses its buffer as the user types.
pub fn parse(
    path: &str,
    content: String,
    has_bom: bool,
    encoding: text_file::Encoding,
) -> AppResult<TechnologyFile> {
    let document = paradox::parse(&content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let mut variables = Vec::new();
    let mut technologies = Vec::new();
    for block in document
        .find_all("technologies")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
    {
        for pair in block.pairs() {
            if let Some(name) = pair.key.strip_prefix('@') {
                if let Some(value) = pair.value.as_scalar() {
                    variables.push(Variable {
                        name: name.to_string(),
                        value: value.value(),
                    });
                }
            } else if let Some(tech) = pair.value.as_block() {
                technologies.push(read_technology(&content, &pair.key, tech, pair.span.start));
            }
        }
    }

    Ok(TechnologyFile {
        path: path.replace('\\', "/"),
        variables,
        technologies,
        has_bom,
        encoding,
        source: content,
    })
}

/// Every `id = { }` pair inside the file's `technologies` blocks, in file order.
fn technology_pairs(document: &Document) -> Vec<&Pair> {
    document
        .find_all("technologies")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .flat_map(|block| block.pairs())
        .filter(|pair| !pair.key.starts_with('@') && pair.value.as_block().is_some())
        .collect()
}

fn read_technology(source: &str, id: &str, block: &Block, start: usize) -> Technology {
    let scalar = |key: &str| block.scalar(key).unwrap_or_default();
    let text = |key: &str| {
        block
            .find(key)
            .map(|pair| block_body(source, &pair.value))
            .unwrap_or_default()
    };
    let list = |key: &str| block.block(key).map(words_of).unwrap_or_default();

    let folders = block
        .find_all("folder")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .map(|folder| {
            let position = folder.block("position");
            TechFolder {
                name: folder.scalar("name").unwrap_or_default(),
                x: position.and_then(|p| p.scalar("x")).unwrap_or_default(),
                y: position.and_then(|p| p.scalar("y")).unwrap_or_default(),
            }
        })
        .collect();

    let paths = block
        .find_all("path")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .map(|path| TechPath {
            leads_to_tech: path.scalar("leads_to_tech").unwrap_or_default(),
            research_cost_coeff: path.scalar("research_cost_coeff").unwrap_or_default(),
        })
        .collect();

    let edited: Vec<&str> = SCALARS
        .iter()
        .chain(LISTS)
        .chain(BLOCKS)
        .chain(["folder", "path"].iter())
        .copied()
        .collect();

    Technology {
        id: id.to_string(),
        research_cost: scalar("research_cost"),
        start_year: scalar("start_year"),
        doctrine: scalar("doctrine"),
        doctrine_name: scalar("doctrine_name"),
        xp_research_type: scalar("xp_research_type"),
        xp_research_cost: scalar("xp_research_cost"),
        xp_research_bonus: scalar("xp_research_bonus"),
        folders,
        paths,
        categories: list("categories"),
        xor: list("xor"),
        sub_technologies: list("sub_technologies"),
        enable_equipments: list("enable_equipments"),
        enable_equipment_modules: list("enable_equipment_modules"),
        enable_subunits: list("enable_subunits"),
        allow: text("allow"),
        allow_branch: text("allow_branch"),
        on_research_complete: text("on_research_complete"),
        ai_will_do: text("ai_will_do"),
        ai_research_weights: text("ai_research_weights"),
        effects: body_without(source, block, &edited),
        line: paradox::line_of(source, start),
    }
}

/// Rewrites one technology in place, matched by its current id.
pub fn update(path: &str, tech_id: &str, update: &TechnologyUpdate) -> AppResult<TechnologyFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = update_source(path, &file.content, tech_id, update)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`update`], applied to text instead of a file.
pub fn update_source(
    path: &str,
    source: &str,
    tech_id: &str,
    update: &TechnologyUpdate,
) -> AppResult<String> {
    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let pair = find_technology(&document, tech_id).ok_or_else(|| {
        AppError::message(format!(
            "no technology with id `{tech_id}` exists in {path}"
        ))
    })?;
    let block = pair
        .value
        .as_block()
        .ok_or_else(|| AppError::message(format!("`{tech_id}` is not a technology block")))?;

    validate(update)?;

    let new_id = update.id.trim();
    let mut edits = Vec::new();
    if new_id != tech_id {
        if find_technology(&document, new_id).is_some() {
            return Err(AppError::message(format!(
                "a technology with id `{new_id}` already exists in this file"
            )));
        }
        // The id is the block's key, which sits at the very start of the pair.
        let key = Span::new(pair.span.start, pair.span.start + tech_id.len());
        if key.text(source) != tech_id {
            return Err(AppError::message(format!(
                "the id of `{tech_id}` is written in a way that cannot be renamed"
            )));
        }
        edits.push(TextEdit::new(key, new_id));
    }

    let mut editor = BlockEditor::new(source, block);
    for (key, value) in SCALARS.iter().zip([
        &update.research_cost,
        &update.start_year,
        &update.doctrine,
        &update.doctrine_name,
        &update.xp_research_type,
        &update.xp_research_cost,
        &update.xp_research_bonus,
    ]) {
        if *key == "doctrine_name" {
            set_quoted(&mut editor, key, value);
        } else {
            editor.set_scalar(key, value);
        }
    }
    set_folders(&mut editor, &update.folders);
    set_paths(&mut editor, &update.paths);
    for (key, value) in LISTS.iter().zip([
        &update.categories,
        &update.xor,
        &update.sub_technologies,
        &update.enable_equipments,
        &update.enable_equipment_modules,
        &update.enable_subunits,
    ]) {
        set_list(&mut editor, key, value);
    }
    for (key, value) in BLOCKS.iter().zip([
        &update.allow,
        &update.allow_branch,
        &update.on_research_complete,
        &update.ai_will_do,
        &update.ai_research_weights,
    ]) {
        editor.set_block(key, value);
    }

    edits.extend(editor.finish());
    Ok(apply_edits(source, edits))
}

/// Appends a technology to the file's `technologies` block, creating the
/// block in a file that has none.
pub fn add(path: &str, tech: &TechnologyUpdate) -> AppResult<TechnologyFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = add_source(path, &file.content, tech)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`add`], applied to text instead of a file.
pub fn add_source(path: &str, source: &str, tech: &TechnologyUpdate) -> AppResult<String> {
    validate(tech)?;

    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let id = tech.id.trim();
    if find_technology(&document, id).is_some() {
        return Err(AppError::message(format!(
            "a technology with id `{id}` already exists in this file"
        )));
    }

    let newline = detect_newline(source);
    match document.block("technologies") {
        Some(block) => {
            let indent = child_indent(source, block);
            let line = format!(
                "{id} = {}",
                format_lines(&render_technology(tech), &indent, newline)
            );
            Ok(apply_edits(
                source,
                vec![insert_lines(source, block, &[line], &[])],
            ))
        }
        None => {
            let mut editor = BlockEditor::for_document(source, &document);
            let body = format!(
                "{id} = {}",
                format_lines(&render_technology(tech), "", "\n")
            );
            editor.set_block("technologies", &body);
            Ok(apply_edits(source, editor.finish()))
        }
    }
}

pub fn delete(path: &str, tech_id: &str) -> AppResult<TechnologyFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = delete_source(path, &file.content, tech_id)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`delete`], applied to text instead of a file.
pub fn delete_source(path: &str, source: &str, tech_id: &str) -> AppResult<String> {
    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let pair = find_technology(&document, tech_id).ok_or_else(|| {
        AppError::message(format!(
            "no technology with id `{tech_id}` exists in {path}"
        ))
    })?;

    Ok(apply_edits(source, remove_pair(source, pair)))
}

fn find_technology<'a>(document: &'a Document, id: &str) -> Option<&'a Pair> {
    technology_pairs(document)
        .into_iter()
        .find(|pair| pair.key == id)
}

fn validate(update: &TechnologyUpdate) -> AppResult<()> {
    let id = update.id.trim();
    if id.is_empty() {
        return Err(AppError::message("a technology needs an id"));
    }
    if id.starts_with('@') || needs_quotes(id) {
        return Err(AppError::message(format!(
            "`{id}` cannot be a technology id: use letters, digits and underscores"
        )));
    }
    for folder in &update.folders {
        if folder.name.trim().is_empty() {
            return Err(AppError::message("every folder entry needs a folder name"));
        }
    }

    Ok(())
}

/// A word list the form edits: set in place, and dropped altogether when
/// the form empties it, since none of these means anything empty. A list
/// that already reads as empty — every entry commented out, as the game
/// does to retire an equipment — is left as it is.
fn set_list(editor: &mut BlockEditor<'_>, key: &str, values: &[String]) {
    if normalise_words(values).is_empty() {
        if let Some(pair) = editor.find_all(key).into_iter().next() {
            let already_empty = pair
                .value
                .as_block()
                .is_some_and(|block| words_of(block).is_empty());
            if !already_empty {
                editor.remove(pair);
            }
        }
        return;
    }
    editor.set_words(key, values);
}

/// A value the game's files quote, such as a doctrine name.
fn set_quoted(editor: &mut BlockEditor<'_>, key: &str, value: &str) {
    let trimmed = value.trim();
    let existing = editor.find_all(key).into_iter().next();

    match existing {
        Some(pair) if trimmed.is_empty() => {
            editor.remove(pair);
        }
        Some(pair) => {
            let current = pair.value.as_scalar().map(|scalar| scalar.value());
            if current.as_deref() != Some(trimmed) {
                editor.edit(TextEdit::new(pair.value.span(), quote(trimmed)));
            }
        }
        None if trimmed.is_empty() => {}
        None => {
            editor.add_line(format!("{key} = {}", quote(trimmed)));
        }
    }
}

/// Folders are keyed by name: one the technology is already on has its
/// position set in place, one it joins is added, one it leaves is removed.
fn set_folders(editor: &mut BlockEditor<'_>, folders: &[TechFolder]) {
    let source = editor.source();
    let newline = detect_newline(source);
    let existing = editor.find_all("folder");

    let named = |pair: &&Pair, name: &str| {
        pair.value
            .as_block()
            .and_then(|block| block.scalar("name"))
            .is_some_and(|current| current == name)
    };

    for folder in folders {
        let name = folder.name.trim();
        match existing.iter().find(|pair| named(pair, name)) {
            Some(pair) => {
                let Some(block) = pair.value.as_block() else {
                    continue;
                };
                let mut inner = BlockEditor::new(source, block);
                match block.block("position") {
                    Some(position) => {
                        let mut place = BlockEditor::new(source, position);
                        place.set_scalar("x", &folder.x);
                        place.set_scalar("y", &folder.y);
                        for edit in place.finish() {
                            inner.edit(edit);
                        }
                    }
                    None => {
                        inner.set_block("position", &render_position(folder));
                    }
                }
                for edit in inner.finish() {
                    editor.edit(edit);
                }
            }
            None => {
                let indent = editor
                    .block()
                    .map(|block| child_indent(source, block))
                    .unwrap_or_default();
                editor.add_line(format!(
                    "folder = {}",
                    format_lines(&render_folder(folder), &indent, newline)
                ));
            }
        }
    }
    for pair in existing {
        let keep = folders
            .iter()
            .any(|folder| named(&pair, folder.name.trim()));
        if !keep {
            editor.remove(pair);
        }
    }
}

/// Paths are keyed by the technology they lead to. The game's own files
/// carry the odd `path = { research_cost_coeff = 1 }` that leads nowhere,
/// so an empty target is a key like any other rather than an error.
fn set_paths(editor: &mut BlockEditor<'_>, paths: &[TechPath]) {
    let source = editor.source();
    let newline = detect_newline(source);
    let existing = editor.find_all("path");

    let leading_to = |pair: &&Pair, target: &str| {
        pair.value
            .as_block()
            .map(|block| block.scalar("leads_to_tech").unwrap_or_default())
            .is_some_and(|current| current == target)
    };

    for path in paths {
        let target = path.leads_to_tech.trim();
        match existing.iter().find(|pair| leading_to(pair, target)) {
            Some(pair) => {
                let Some(block) = pair.value.as_block() else {
                    continue;
                };
                let mut inner = BlockEditor::new(source, block);
                inner.set_scalar("research_cost_coeff", &path.research_cost_coeff);
                for edit in inner.finish() {
                    editor.edit(edit);
                }
            }
            None => {
                let indent = editor
                    .block()
                    .map(|block| child_indent(source, block))
                    .unwrap_or_default();
                editor.add_line(format!(
                    "path = {}",
                    format_lines(&render_path(path), &indent, newline)
                ));
            }
        }
    }
    for pair in existing {
        let keep = paths
            .iter()
            .any(|path| leading_to(&pair, path.leads_to_tech.trim()));
        if !keep {
            editor.remove(pair);
        }
    }
}

// The renderers produce a block body with `\n` line breaks and no leading
// indentation; the formatter re-indents it wherever it lands.

fn render_position(folder: &TechFolder) -> String {
    format!("x = {} y = {}", folder.x.trim(), folder.y.trim())
}

fn render_folder(folder: &TechFolder) -> String {
    format!(
        "name = {}\nposition = {{ {} }}",
        folder.name.trim(),
        render_position(folder)
    )
}

fn render_path(path: &TechPath) -> String {
    let mut lines = Vec::new();
    if !path.leads_to_tech.trim().is_empty() {
        lines.push(format!("leads_to_tech = {}", path.leads_to_tech.trim()));
    }
    if !path.research_cost_coeff.trim().is_empty() {
        lines.push(format!(
            "research_cost_coeff = {}",
            path.research_cost_coeff.trim()
        ));
    }
    lines.join("\n")
}

fn render_technology(tech: &TechnologyUpdate) -> String {
    let mut lines = Vec::new();
    for (key, values) in LISTS.iter().zip([
        &tech.categories,
        &tech.xor,
        &tech.sub_technologies,
        &tech.enable_equipments,
        &tech.enable_equipment_modules,
        &tech.enable_subunits,
    ]) {
        let words = normalise_words(values);
        if !words.is_empty() && *key != "categories" {
            lines.push(format!(
                "{key} = {}",
                format_block(&render_words(&words), "", "\n")
            ));
        }
    }
    for path in &tech.paths {
        lines.push(format!(
            "path = {}",
            format_lines(&render_path(path), "", "\n")
        ));
    }
    for (key, value) in [
        ("research_cost", tech.research_cost.as_str()),
        ("start_year", tech.start_year.as_str()),
        ("doctrine", tech.doctrine.as_str()),
        ("xp_research_type", tech.xp_research_type.as_str()),
        ("xp_research_cost", tech.xp_research_cost.as_str()),
        ("xp_research_bonus", tech.xp_research_bonus.as_str()),
    ] {
        if !value.trim().is_empty() {
            lines.push(format!("{key} = {}", value.trim()));
        }
    }
    if !tech.doctrine_name.trim().is_empty() {
        lines.push(format!(
            "doctrine_name = {}",
            quote(tech.doctrine_name.trim())
        ));
    }
    for folder in &tech.folders {
        lines.push(format!(
            "folder = {}",
            format_lines(&render_folder(folder), "", "\n")
        ));
    }
    let categories = normalise_words(&tech.categories);
    if !categories.is_empty() {
        lines.push(format!(
            "categories = {}",
            format_lines(&categories.join("\n"), "", "\n")
        ));
    }
    for (key, value) in BLOCKS.iter().zip([
        &tech.allow,
        &tech.allow_branch,
        &tech.on_research_complete,
        &tech.ai_will_do,
        &tech.ai_research_weights,
    ]) {
        if !value.trim().is_empty() {
            lines.push(format!("{key} = {}", format_block(value, "", "\n")));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two technologies the way the game writes them: row variables, a
    /// commented position, an equipment list, a doctrine with effects.
    const FILE: &str = "technologies = {\n\n\t@1918 = 0\n\t@1936 = 2\n\n\tinfantry_weapons = {\n\n\t\tenable_equipments = {\n\t\t\tinfantry_equipment_0\n\t\t}\n\n\t\tpath = {\n\t\t\tleads_to_tech = infantry_weapons1\n\t\t\tresearch_cost_coeff = 1\n\t\t}\n\n\t\tresearch_cost = 1.5\n\t\tstart_year = 1918\n\t\tfolder = {\n\t\t\tname = infantry_folder\n\t\t\tposition = { x = 0 y = @1918 } # top row\n\t\t}\n\n\t\tcategories = {\n\t\t\tinfantry_weapons\n\t\t}\n\n\t\tai_will_do = {\n\t\t\tfactor = 1\n\t\t}\n\t}\n\n\tmobile_warfare = {\n\t\tdoctrine_name = \"MOBILE_WARFARE_DOCTRINE\"\n\t\txp_research_type = army\n\t\txor = { superior_firepower trench_warfare }\n\t\tpath = {\n\t\t\tleads_to_tech = delay\n\t\t\tresearch_cost_coeff = 1\n\t\t}\n\t\tcategory_all_armor = {\n\t\t\tbreakthrough = 0.20 # punch\n\t\t}\n\t\tplanning_speed = 0.5\n\t\tdoctrine = yes\n\t\tresearch_cost = 2.25\n\t\tfolder = {\n\t\t\tname = land_doctrine_folder\n\t\t\tposition = { x = 0 y = 0 }\n\t\t}\n\t\tcategories = {\n\t\t\tland_doctrine\n\t\t\tcat_mobile_warfare\n\t\t}\n\t}\n}\n";

    fn parsed(source: &str) -> TechnologyFile {
        parse(
            "(test)",
            source.to_string(),
            false,
            text_file::Encoding::Utf8,
        )
        .expect("parses")
    }

    fn update_from(tech: &Technology) -> TechnologyUpdate {
        TechnologyUpdate {
            id: tech.id.clone(),
            research_cost: tech.research_cost.clone(),
            start_year: tech.start_year.clone(),
            doctrine: tech.doctrine.clone(),
            doctrine_name: tech.doctrine_name.clone(),
            xp_research_type: tech.xp_research_type.clone(),
            xp_research_cost: tech.xp_research_cost.clone(),
            xp_research_bonus: tech.xp_research_bonus.clone(),
            folders: tech.folders.clone(),
            paths: tech.paths.clone(),
            categories: tech.categories.clone(),
            xor: tech.xor.clone(),
            sub_technologies: tech.sub_technologies.clone(),
            enable_equipments: tech.enable_equipments.clone(),
            enable_equipment_modules: tech.enable_equipment_modules.clone(),
            enable_subunits: tech.enable_subunits.clone(),
            allow: tech.allow.clone(),
            allow_branch: tech.allow_branch.clone(),
            on_research_complete: tech.on_research_complete.clone(),
            ai_will_do: tech.ai_will_do.clone(),
            ai_research_weights: tech.ai_research_weights.clone(),
        }
    }

    #[test]
    fn reads_variables_folders_paths_and_effects() {
        let file = parsed(FILE);
        assert_eq!(file.variables.len(), 2);
        assert_eq!(file.variables[1].name, "1936");
        assert_eq!(file.variables[1].value, "2");
        assert_eq!(file.technologies.len(), 2);

        let rifles = &file.technologies[0];
        assert_eq!(rifles.id, "infantry_weapons");
        assert_eq!(rifles.line, 6);
        assert_eq!(rifles.research_cost, "1.5");
        assert_eq!(rifles.start_year, "1918");
        assert_eq!(rifles.enable_equipments, vec!["infantry_equipment_0"]);
        assert_eq!(rifles.paths.len(), 1);
        assert_eq!(rifles.paths[0].leads_to_tech, "infantry_weapons1");
        assert_eq!(rifles.folders.len(), 1);
        assert_eq!(rifles.folders[0].name, "infantry_folder");
        assert_eq!(rifles.folders[0].x, "0");
        assert_eq!(rifles.folders[0].y, "@1918");
        assert_eq!(rifles.categories, vec!["infantry_weapons"]);
        assert_eq!(rifles.ai_will_do, "factor = 1");
        assert_eq!(rifles.effects, "");

        let doctrine = &file.technologies[1];
        assert_eq!(doctrine.doctrine, "yes");
        assert_eq!(doctrine.doctrine_name, "MOBILE_WARFARE_DOCTRINE");
        assert_eq!(doctrine.xor, vec!["superior_firepower", "trench_warfare"]);
        assert_eq!(
            doctrine.effects,
            "category_all_armor = {\n\tbreakthrough = 0.20 # punch\n}\nplanning_speed = 0.5"
        );
    }

    #[test]
    fn a_no_op_save_changes_nothing() {
        let file = parsed(FILE);
        for tech in &file.technologies {
            let text =
                update_source("(test)", FILE, &tech.id, &update_from(tech)).expect("updates");
            assert_eq!(text, FILE, "{} was rewritten", tech.id);
        }
    }

    #[test]
    fn moves_a_technology_and_keeps_the_rest() {
        let file = parsed(FILE);
        let mut change = update_from(&file.technologies[0]);
        change.folders[0].x = "2".into();
        change.folders[0].y = "@1936".into();
        change.research_cost = "2".into();
        change.paths[0].research_cost_coeff = "0.9".into();

        let text = update_source("(test)", FILE, "infantry_weapons", &change).expect("updates");

        assert!(
            text.contains("position = { x = 2 y = @1936 } # top row"),
            "{text}"
        );
        assert!(text.contains("\t\tresearch_cost = 2\n"), "{text}");
        assert!(text.contains("research_cost_coeff = 0.9"), "{text}");
        assert!(
            text.contains("breakthrough = 0.20 # punch"),
            "effects touched:\n{text}"
        );
        assert!(text.contains("\t@1936 = 2\n"), "{text}");
    }

    #[test]
    fn folders_and_paths_are_keyed() {
        let file = parsed(FILE);
        let mut change = update_from(&file.technologies[0]);
        change.folders.push(TechFolder {
            name: "support_folder".into(),
            x: "-1".into(),
            y: "3".into(),
        });
        change.paths = vec![
            TechPath {
                leads_to_tech: "infantry_weapons2".into(),
                research_cost_coeff: "1".into(),
            },
            TechPath {
                leads_to_tech: "infantry_weapons1".into(),
                research_cost_coeff: "1".into(),
            },
        ];
        change.enable_equipments =
            vec!["infantry_equipment_0".into(), "infantry_equipment_1".into()];
        change.xor = vec!["cavalry".into()];

        let text = update_source("(test)", FILE, "infantry_weapons", &change).expect("updates");
        let rifles = &parsed(&text).technologies[0];
        assert_eq!(rifles.folders.len(), 2);
        assert_eq!(rifles.folders[1].name, "support_folder");
        assert_eq!(rifles.folders[1].x, "-1");
        assert_eq!(rifles.paths.len(), 2);
        // The path already there kept its place; the new one was appended.
        assert_eq!(rifles.paths[0].leads_to_tech, "infantry_weapons1");
        assert_eq!(rifles.paths[1].leads_to_tech, "infantry_weapons2");
        assert_eq!(rifles.enable_equipments.len(), 2);
        assert_eq!(rifles.xor, vec!["cavalry"]);
        assert!(text.contains("\t\tfolder = {\n\t\t\tname = support_folder\n\t\t\tposition = { x = -1 y = 3 }\n\t\t}\n"), "{text}");

        let mut change = update_from(rifles);
        change.folders.remove(0);
        change.paths.remove(0);
        change.xor.clear();
        let text = update_source("(test)", &text, "infantry_weapons", &change).expect("updates");
        let rifles = &parsed(&text).technologies[0];
        assert_eq!(rifles.folders.len(), 1);
        assert_eq!(rifles.folders[0].name, "support_folder");
        assert_eq!(rifles.paths.len(), 1);
        assert_eq!(rifles.paths[0].leads_to_tech, "infantry_weapons2");
        assert!(rifles.xor.is_empty());
    }

    #[test]
    fn renames_the_block_key_and_quotes_the_doctrine_name() {
        let file = parsed(FILE);
        let mut change = update_from(&file.technologies[1]);
        change.id = "mobile_warfare_2".into();
        change.doctrine_name = "MOBILE_WARFARE_DOCTRINE_2".into();

        let text = update_source("(test)", FILE, "mobile_warfare", &change).expect("updates");
        let doctrine = &parsed(&text).technologies[1];
        assert_eq!(doctrine.id, "mobile_warfare_2");
        assert!(
            text.contains("doctrine_name = \"MOBILE_WARFARE_DOCTRINE_2\""),
            "{text}"
        );

        let mut clash = update_from(doctrine);
        clash.id = "infantry_weapons".into();
        assert!(update_source("(test)", &text, "mobile_warfare_2", &clash).is_err());
    }

    #[test]
    fn adds_and_deletes_a_technology() {
        let added = add_source(
            "(test)",
            FILE,
            &TechnologyUpdate {
                id: "infantry_weapons1".into(),
                research_cost: "1.5".into(),
                start_year: "1936".into(),
                folders: vec![TechFolder {
                    name: "infantry_folder".into(),
                    x: "0".into(),
                    y: "@1936".into(),
                }],
                paths: vec![TechPath {
                    leads_to_tech: "infantry_weapons2".into(),
                    research_cost_coeff: "1".into(),
                }],
                categories: vec!["infantry_weapons".into()],
                enable_equipments: vec!["infantry_equipment_1".into()],
                ai_will_do: "factor = 1".into(),
                ..TechnologyUpdate::default()
            },
        )
        .expect("adds");

        let reread = parsed(&added);
        assert_eq!(reread.technologies.len(), 3);
        let rifles = &reread.technologies[2];
        assert_eq!(rifles.id, "infantry_weapons1");
        assert_eq!(rifles.folders[0].y, "@1936");
        assert!(
            added.contains("# top row"),
            "the rest was disturbed:\n{added}"
        );
        assert!(
            added.contains("\n\tinfantry_weapons1 = {\n\t\tenable_equipments = { infantry_equipment_1 }\n\t\tpath = {\n\t\t\tleads_to_tech = infantry_weapons2\n\t\t\tresearch_cost_coeff = 1\n\t\t}\n\t\tresearch_cost = 1.5\n\t\tstart_year = 1936\n\t\tfolder = {\n\t\t\tname = infantry_folder\n\t\t\tposition = { x = 0 y = @1936 }\n\t\t}\n\t\tcategories = {\n\t\t\tinfantry_weapons\n\t\t}\n\t\tai_will_do = { factor = 1 }\n\t}\n"),
            "{added}"
        );

        let deleted = delete_source("(test)", &added, "mobile_warfare").expect("deletes");
        let reread = parsed(&deleted);
        assert_eq!(reread.technologies.len(), 2);
        assert!(!deleted.contains("mobile_warfare"), "{deleted}");
    }

    #[test]
    fn refuses_bad_input() {
        let file = parsed(FILE);
        let mut change = update_from(&file.technologies[0]);
        change.id = "@1950".into();
        assert!(update_source("(test)", FILE, "infantry_weapons", &change).is_err());

        let mut change = update_from(&file.technologies[0]);
        change.folders[0].name = String::new();
        assert!(update_source("(test)", FILE, "infantry_weapons", &change).is_err());
    }
}
