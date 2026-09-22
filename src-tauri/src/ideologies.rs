//! Ideologies: `common/ideologies/*.txt`, the groups a country's politics
//! fall into, each with its sub-ideologies, colour, world tension impact,
//! rules and modifiers.
//!
//! An ideology is one `id = { }` block inside `ideologies = { }`. Its
//! sub-ideologies and rules are keyed by name and edited in place; its
//! modifier blocks are handed over as text, since a mod tends to keep a
//! commented-out menu of them in there.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paradox::edit::{
    apply_edits, block_body, child_indent, detect_newline, format_block, format_lines, indent_at,
    insert_lines, remove_pair, words_of, BlockEditor, TextEdit,
};
use crate::paradox::lexer::{needs_quotes, quote};
use crate::paradox::{self, Block, Document, Items, Pair, Span};
use crate::text_file;

/// The AI behaviours an ideology can adopt, as the file's `ai_<x> = yes`.
pub const AI_BEHAVIOURS: &[&str] = &["democratic", "communist", "fascist", "neutral"];

/// Yes/no fields at the top of an ideology.
const FLAGS: &[&str] = &[
    "can_host_government_in_exile",
    "can_be_boosted",
    "can_collaborate",
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubIdeology {
    pub id: String,
    /// `yes`, `no`, or empty when the file does not say (which means yes).
    pub can_be_randomly_selected: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ideology {
    pub id: String,
    /// The body of `color = { }`: three numbers, 0-255 or 0-1.
    pub color: String,
    pub types: Vec<SubIdeology>,
    /// Localisation keys of the faction names, unquoted.
    pub dynamic_faction_names: Vec<String>,
    pub war_impact_on_world_tension: String,
    pub faction_impact_on_world_tension: String,
    pub can_host_government_in_exile: String,
    pub can_be_boosted: String,
    pub can_collaborate: String,
    /// `democratic`, `communist`, `fascist`, `neutral`, or empty: which
    /// `ai_<x> = yes` the file sets.
    pub ai_behaviour: String,
    pub ai_ideology_wanted_units_factor: String,
    pub rules: Vec<Rule>,
    /// Bodies of the modifier blocks, kept as text.
    pub modifiers: String,
    pub faction_modifiers: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeologyFile {
    pub path: String,
    pub ideologies: Vec<Ideology>,
    pub has_bom: bool,
    pub encoding: text_file::Encoding,
    /// The text the ideologies were read from, for the code view.
    pub source: String,
}

/// The editable fields sent back when saving. Empty strings clear a field;
/// a sub-ideology, faction name or rule left out of its list is removed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdeologyUpdate {
    pub id: String,
    pub color: String,
    pub types: Vec<SubIdeology>,
    pub dynamic_faction_names: Vec<String>,
    pub war_impact_on_world_tension: String,
    pub faction_impact_on_world_tension: String,
    pub can_host_government_in_exile: String,
    pub can_be_boosted: String,
    pub can_collaborate: String,
    pub ai_behaviour: String,
    pub ai_ideology_wanted_units_factor: String,
    pub rules: Vec<Rule>,
    pub modifiers: String,
    pub faction_modifiers: String,
}

pub fn read(path: &str) -> AppResult<IdeologyFile> {
    let file = text_file::read(Path::new(path))?;
    parse(path, file.content, file.has_bom, file.encoding)
}

/// Reads ideologies out of text the window holds, which need not be what
/// is on disk: the code view parses its buffer as the user types.
pub fn parse(
    path: &str,
    content: String,
    has_bom: bool,
    encoding: text_file::Encoding,
) -> AppResult<IdeologyFile> {
    let document = paradox::parse(&content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let ideologies = ideology_pairs(&document)
        .into_iter()
        .filter_map(|pair| {
            pair.value
                .as_block()
                .map(|block| read_ideology(&content, &pair.key, block, pair.span.start))
        })
        .collect();

    Ok(IdeologyFile {
        path: path.replace('\\', "/"),
        ideologies,
        has_bom,
        encoding,
        source: content,
    })
}

/// Every `id = { }` pair inside the file's `ideologies` blocks, in file order.
fn ideology_pairs(document: &Document) -> Vec<&Pair> {
    document
        .find_all("ideologies")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .flat_map(|block| block.pairs())
        .filter(|pair| pair.value.as_block().is_some())
        .collect()
}

fn read_ideology(source: &str, id: &str, block: &Block, start: usize) -> Ideology {
    let scalar = |key: &str| block.scalar(key).unwrap_or_default();
    let text = |key: &str| {
        block
            .find(key)
            .map(|pair| block_body(source, &pair.value))
            .unwrap_or_default()
    };

    let types = block
        .block("types")
        .map(|types| {
            types
                .pairs()
                .into_iter()
                .filter_map(|pair| pair.value.as_block().map(|sub| (pair, sub)))
                .map(|(pair, sub)| SubIdeology {
                    id: pair.key.clone(),
                    can_be_randomly_selected: sub
                        .scalar("can_be_randomly_selected")
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    let rules = block
        .block("rules")
        .map(|rules| {
            rules
                .pairs()
                .into_iter()
                .filter_map(|pair| {
                    pair.value.as_scalar().map(|value| Rule {
                        key: pair.key.clone(),
                        value: value.value(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let ai_behaviour = AI_BEHAVIOURS
        .iter()
        .find(|behaviour| {
            block
                .scalar(&format!("ai_{behaviour}"))
                .is_some_and(|value| value.eq_ignore_ascii_case("yes"))
        })
        .map(|behaviour| behaviour.to_string())
        .unwrap_or_default();

    Ideology {
        id: id.to_string(),
        color: text("color"),
        types,
        dynamic_faction_names: block
            .block("dynamic_faction_names")
            .map(words_of)
            .unwrap_or_default(),
        war_impact_on_world_tension: scalar("war_impact_on_world_tension"),
        faction_impact_on_world_tension: scalar("faction_impact_on_world_tension"),
        can_host_government_in_exile: scalar("can_host_government_in_exile"),
        can_be_boosted: scalar("can_be_boosted"),
        can_collaborate: scalar("can_collaborate"),
        ai_behaviour,
        ai_ideology_wanted_units_factor: scalar("ai_ideology_wanted_units_factor"),
        rules,
        modifiers: text("modifiers"),
        faction_modifiers: text("faction_modifiers"),
        line: paradox::line_of(source, start),
    }
}

/// Rewrites one ideology in place, matched by its current id.
pub fn update(path: &str, ideology_id: &str, update: &IdeologyUpdate) -> AppResult<IdeologyFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = update_source(path, &file.content, ideology_id, update)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`update`], applied to text instead of a file.
pub fn update_source(
    path: &str,
    source: &str,
    ideology_id: &str,
    update: &IdeologyUpdate,
) -> AppResult<String> {
    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let pair = find_ideology(&document, ideology_id).ok_or_else(|| {
        AppError::message(format!(
            "no ideology with id `{ideology_id}` exists in {path}"
        ))
    })?;
    let block = pair
        .value
        .as_block()
        .ok_or_else(|| AppError::message(format!("`{ideology_id}` is not an ideology block")))?;

    validate(update)?;

    let new_id = update.id.trim();
    let mut edits = Vec::new();
    if new_id != ideology_id {
        if find_ideology(&document, new_id).is_some() {
            return Err(AppError::message(format!(
                "an ideology with id `{new_id}` already exists in this file"
            )));
        }
        // The id is the block's key, which sits at the very start of the pair.
        let key = Span::new(pair.span.start, pair.span.start + ideology_id.len());
        if key.text(source) != ideology_id {
            return Err(AppError::message(format!(
                "the id of `{ideology_id}` is written in a way that cannot be renamed"
            )));
        }
        edits.push(TextEdit::new(key, new_id));
    }

    let mut editor = BlockEditor::new(source, block);
    set_colour(&mut editor, &update.color);
    set_types(&mut editor, &update.types);
    set_names(&mut editor, &update.dynamic_faction_names);
    editor.set_scalar(
        "war_impact_on_world_tension",
        &update.war_impact_on_world_tension,
    );
    editor.set_scalar(
        "faction_impact_on_world_tension",
        &update.faction_impact_on_world_tension,
    );
    for (key, value) in FLAGS.iter().zip([
        &update.can_host_government_in_exile,
        &update.can_be_boosted,
        &update.can_collaborate,
    ]) {
        editor.set_scalar(key, value);
    }
    for behaviour in AI_BEHAVIOURS {
        let wanted = if update.ai_behaviour.trim() == *behaviour {
            "yes"
        } else {
            ""
        };
        editor.set_scalar(&format!("ai_{behaviour}"), wanted);
    }
    editor.set_scalar(
        "ai_ideology_wanted_units_factor",
        &update.ai_ideology_wanted_units_factor,
    );
    set_rules(&mut editor, &update.rules);
    editor.set_block("modifiers", &update.modifiers);
    editor.set_block("faction_modifiers", &update.faction_modifiers);

    edits.extend(editor.finish());
    Ok(apply_edits(source, edits))
}

/// Appends an ideology to the file's `ideologies` block, creating the block
/// in a file that has none.
pub fn add(path: &str, ideology: &IdeologyUpdate) -> AppResult<IdeologyFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = add_source(path, &file.content, ideology)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`add`], applied to text instead of a file.
pub fn add_source(path: &str, source: &str, ideology: &IdeologyUpdate) -> AppResult<String> {
    validate(ideology)?;

    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let id = ideology.id.trim();
    if find_ideology(&document, id).is_some() {
        return Err(AppError::message(format!(
            "an ideology with id `{id}` already exists in this file"
        )));
    }

    let newline = detect_newline(source);
    match document.block("ideologies") {
        Some(block) => {
            let indent = child_indent(source, block);
            let line = format!(
                "{id} = {}",
                format_lines(&render_ideology(ideology), &indent, newline)
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
                format_lines(&render_ideology(ideology), "", "\n")
            );
            editor.set_block("ideologies", &body);
            Ok(apply_edits(source, editor.finish()))
        }
    }
}

pub fn delete(path: &str, ideology_id: &str) -> AppResult<IdeologyFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = delete_source(path, &file.content, ideology_id)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`delete`], applied to text instead of a file.
pub fn delete_source(path: &str, source: &str, ideology_id: &str) -> AppResult<String> {
    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let pair = find_ideology(&document, ideology_id).ok_or_else(|| {
        AppError::message(format!(
            "no ideology with id `{ideology_id}` exists in {path}"
        ))
    })?;

    Ok(apply_edits(source, remove_pair(source, pair)))
}

fn find_ideology<'a>(document: &'a Document, id: &str) -> Option<&'a Pair> {
    ideology_pairs(document)
        .into_iter()
        .find(|pair| pair.key == id)
}

fn validate(update: &IdeologyUpdate) -> AppResult<()> {
    let id = update.id.trim();
    if id.is_empty() {
        return Err(AppError::message("an ideology needs an id"));
    }
    if needs_quotes(id) {
        return Err(AppError::message(format!(
            "`{id}` cannot be an ideology id: use letters, digits and underscores"
        )));
    }
    for sub in &update.types {
        let sub_id = sub.id.trim();
        if sub_id.is_empty() || needs_quotes(sub_id) {
            return Err(AppError::message(format!(
                "`{sub_id}` cannot be a sub-ideology id: use letters, digits and underscores"
            )));
        }
    }
    let behaviour = update.ai_behaviour.trim();
    if !behaviour.is_empty() && !AI_BEHAVIOURS.contains(&behaviour) {
        return Err(AppError::message(format!(
            "`{behaviour}` is not an AI behaviour; use one of {}",
            AI_BEHAVIOURS.join(", ")
        )));
    }

    Ok(())
}

/// `color = { r g b }` stays on one line however it is spelled, and is left
/// alone when it already says these numbers.
fn set_colour(editor: &mut BlockEditor<'_>, colour: &str) {
    let wanted = colour.split_whitespace().collect::<Vec<_>>().join(" ");
    let source = editor.source();
    let existing = editor.find_all("color").into_iter().next();

    match existing {
        Some(pair) if wanted.is_empty() => {
            editor.remove(pair);
        }
        Some(pair) => {
            let current = block_body(source, &pair.value)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if current != wanted {
                editor.edit(TextEdit::new(pair.value.span(), format!("{{ {wanted} }}")));
            }
        }
        None if wanted.is_empty() => {}
        None => {
            editor.add_line(format!("color = {{ {wanted} }}"));
        }
    }
}

/// Sub-ideologies are keyed by id: one already there has its flag set in
/// place, one missing is added, one no longer wanted is removed.
fn set_types(editor: &mut BlockEditor<'_>, types: &[SubIdeology]) {
    let source = editor.source();
    let newline = detect_newline(source);
    let existing = editor.find_all("types").into_iter().next();

    let Some(pair) = existing else {
        if !types.is_empty() {
            let indent = editor
                .block()
                .map(|block| child_indent(source, block))
                .unwrap_or_default();
            editor.add_line(format!(
                "types = {}",
                format_lines(&render_types(types), &indent, newline)
            ));
        }
        return;
    };
    let Some(block) = pair.value.as_block() else {
        let indent = indent_at(source, pair.span.start);
        editor.edit(TextEdit::new(
            pair.value.span(),
            format_lines(&render_types(types), &indent, newline),
        ));
        return;
    };

    let mut inner = BlockEditor::new(source, block);
    for sub in types {
        let id = sub.id.trim();
        match block.find(id) {
            Some(sub_pair) => match sub_pair.value.as_block() {
                Some(sub_block) => {
                    let mut flag = BlockEditor::new(source, sub_block);
                    flag.set_scalar("can_be_randomly_selected", &sub.can_be_randomly_selected);
                    for edit in flag.finish() {
                        inner.edit(edit);
                    }
                }
                None => {
                    let indent = indent_at(source, sub_pair.span.start);
                    inner.edit(TextEdit::new(
                        sub_pair.value.span(),
                        format_block(&render_sub_body(sub), &indent, newline),
                    ));
                }
            },
            None => {
                let indent = child_indent(source, block);
                inner.add_line(format!(
                    "{id} = {}",
                    format_block(&render_sub_body(sub), &indent, newline)
                ));
            }
        }
    }
    for sub_pair in block.pairs() {
        if !types.iter().any(|sub| sub.id.trim() == sub_pair.key) {
            inner.remove(sub_pair);
        }
    }
    for edit in inner.finish() {
        editor.edit(edit);
    }
}

/// The faction names are one quoted key per line; a list already saying
/// these names is left alone.
fn set_names(editor: &mut BlockEditor<'_>, names: &[String]) {
    let wanted: Vec<&str> = names
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .collect();
    let source = editor.source();
    let newline = detect_newline(source);
    let existing = editor.find_all("dynamic_faction_names").into_iter().next();

    let Some(pair) = existing else {
        if !wanted.is_empty() {
            let indent = editor
                .block()
                .map(|block| child_indent(source, block))
                .unwrap_or_default();
            editor.add_line(format!(
                "dynamic_faction_names = {}",
                format_lines(&render_names(&wanted), &indent, newline)
            ));
        }
        return;
    };

    if pair
        .value
        .as_block()
        .is_some_and(|block| words_of(block) == wanted)
    {
        return;
    }
    let indent = indent_at(source, pair.span.start);
    editor.edit(TextEdit::new(
        pair.value.span(),
        if wanted.is_empty() {
            "{ }".to_string()
        } else {
            format_lines(&render_names(&wanted), &indent, newline)
        },
    ));
}

/// Rules are keyed: each wanted one is set in place or added, each other
/// one in the file is removed.
fn set_rules(editor: &mut BlockEditor<'_>, rules: &[Rule]) {
    let wanted: Vec<(&str, &str)> = rules
        .iter()
        .map(|rule| (rule.key.trim(), rule.value.trim()))
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .collect();
    let source = editor.source();
    let newline = detect_newline(source);
    let existing = editor.find_all("rules").into_iter().next();

    let Some(pair) = existing else {
        if !wanted.is_empty() {
            let indent = editor
                .block()
                .map(|block| child_indent(source, block))
                .unwrap_or_default();
            editor.add_line(format!(
                "rules = {}",
                format_lines(&render_rules(&wanted), &indent, newline)
            ));
        }
        return;
    };
    let Some(block) = pair.value.as_block() else {
        let indent = indent_at(source, pair.span.start);
        editor.edit(TextEdit::new(
            pair.value.span(),
            format_lines(&render_rules(&wanted), &indent, newline),
        ));
        return;
    };

    let mut inner = BlockEditor::new(source, block);
    for (key, value) in &wanted {
        inner.set_scalar(key, value);
    }
    for rule_pair in block.pairs() {
        if !wanted
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(&rule_pair.key))
        {
            inner.remove(rule_pair);
        }
    }
    for edit in inner.finish() {
        editor.edit(edit);
    }
}

// The renderers produce a block body with `\n` line breaks and no leading
// indentation; the formatter re-indents it wherever it lands.

fn render_sub_body(sub: &SubIdeology) -> String {
    let flag = sub.can_be_randomly_selected.trim();
    if flag.is_empty() {
        String::new()
    } else {
        format!("can_be_randomly_selected = {flag}")
    }
}

fn render_types(types: &[SubIdeology]) -> String {
    types
        .iter()
        .map(|sub| {
            format!(
                "{} = {}",
                sub.id.trim(),
                format_block(&render_sub_body(sub), "", "\n")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_names(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| quote(name))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_rules(rules: &[(&str, &str)]) -> String {
    rules
        .iter()
        .map(|(key, value)| format!("{key} = {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_ideology(ideology: &IdeologyUpdate) -> String {
    let mut lines = Vec::new();
    let colour = ideology
        .color
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !colour.is_empty() {
        lines.push(format!("color = {{ {colour} }}"));
    }
    if !ideology.types.is_empty() {
        lines.push(format!(
            "types = {}",
            format_lines(&render_types(&ideology.types), "", "\n")
        ));
    }
    let names: Vec<&str> = ideology
        .dynamic_faction_names
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .collect();
    if !names.is_empty() {
        lines.push(format!(
            "dynamic_faction_names = {}",
            format_lines(&render_names(&names), "", "\n")
        ));
    }
    let rules: Vec<(&str, &str)> = ideology
        .rules
        .iter()
        .map(|rule| (rule.key.trim(), rule.value.trim()))
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .collect();
    if !rules.is_empty() {
        lines.push(format!(
            "rules = {}",
            format_lines(&render_rules(&rules), "", "\n")
        ));
    }
    for (key, value) in [
        ("modifiers", &ideology.modifiers),
        ("faction_modifiers", &ideology.faction_modifiers),
    ] {
        if !value.trim().is_empty() {
            lines.push(format!("{key} = {}", format_block(value, "", "\n")));
        }
    }
    for (key, value) in [
        (
            "war_impact_on_world_tension",
            ideology.war_impact_on_world_tension.as_str(),
        ),
        (
            "faction_impact_on_world_tension",
            ideology.faction_impact_on_world_tension.as_str(),
        ),
        (
            "can_host_government_in_exile",
            ideology.can_host_government_in_exile.as_str(),
        ),
        ("can_be_boosted", ideology.can_be_boosted.as_str()),
        ("can_collaborate", ideology.can_collaborate.as_str()),
        (
            "ai_ideology_wanted_units_factor",
            ideology.ai_ideology_wanted_units_factor.as_str(),
        ),
    ] {
        if !value.trim().is_empty() {
            lines.push(format!("{key} = {}", value.trim()));
        }
    }
    let behaviour = ideology.ai_behaviour.trim();
    if !behaviour.is_empty() {
        lines.push(format!("ai_{behaviour} = yes"));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two ideologies the way a mod writes them: empty sub-ideology braces,
    /// a commented-out menu of modifiers, and a trailing comment on the id.
    const FILE: &str = "ideologies = {\n\tanarchism = {\t#no state\n\t\tcolor = { 0 0 0 }\n\t\ttypes = {\n\t\t\tanarchism_ideology = {}\n\t\t\tanarchy = {\n\t\t\t\tcan_be_randomly_selected = no\n\t\t\t}\n\t\t}\n\n\t\tdynamic_faction_names = {\n\t\t\t\"FACTION_NAME_ANARCHISM_1\"\n\t\t\t\"FACTION_NAME_ANARCHISM_2\"\n\t\t}\n\n\t\trules = {\n\t\t\tcan_force_government = no\n\t\t\tcan_puppet = no # never\n\t\t}\n\n\t\tmodifiers = {\n\t\t\t#generate_wargoal_tension = \n\t\t\t#join_faction_tension = \n\t\t}\n\n\t\tfaction_modifiers  = {}\n\n\t\twar_impact_on_world_tension = 1\n\t\tfaction_impact_on_world_tension = 1\n\n\t\tcan_host_government_in_exile = no\n\t\tcan_be_boosted = no\n\t}\n\n\tdemocratic = {\n\t\ttypes = {\n\t\t\tconservatism = {\n\t\t\t}\n\t\t\tliberalism = {\n\t\t\t}\n\t\t}\n\t\tcolor = { 0 0 255 }\n\t\trules = {\n\t\t\tcan_force_government = yes\n\t\t}\n\t\twar_impact_on_world_tension = 0.25\t\t# no major danger\n\t\tfaction_impact_on_world_tension = 0.1\n\t\tmodifiers = {\n\t\t\tgenerate_wargoal_tension = 1.00\n\t\t}\n\t\tai_democratic = yes # uses the democratic AI behaviour\n\t\tai_ideology_wanted_units_factor = 1.10\n\t}\n}\n";

    fn parsed(source: &str) -> IdeologyFile {
        parse(
            "(test)",
            source.to_string(),
            false,
            text_file::Encoding::Utf8,
        )
        .expect("parses")
    }

    fn update_from(ideology: &Ideology) -> IdeologyUpdate {
        IdeologyUpdate {
            id: ideology.id.clone(),
            color: ideology.color.clone(),
            types: ideology.types.clone(),
            dynamic_faction_names: ideology.dynamic_faction_names.clone(),
            war_impact_on_world_tension: ideology.war_impact_on_world_tension.clone(),
            faction_impact_on_world_tension: ideology.faction_impact_on_world_tension.clone(),
            can_host_government_in_exile: ideology.can_host_government_in_exile.clone(),
            can_be_boosted: ideology.can_be_boosted.clone(),
            can_collaborate: ideology.can_collaborate.clone(),
            ai_behaviour: ideology.ai_behaviour.clone(),
            ai_ideology_wanted_units_factor: ideology.ai_ideology_wanted_units_factor.clone(),
            rules: ideology.rules.clone(),
            modifiers: ideology.modifiers.clone(),
            faction_modifiers: ideology.faction_modifiers.clone(),
        }
    }

    #[test]
    fn reads_every_field() {
        let file = parsed(FILE);
        assert_eq!(file.ideologies.len(), 2);

        let anarchism = &file.ideologies[0];
        assert_eq!(anarchism.id, "anarchism");
        assert_eq!(anarchism.line, 2);
        assert_eq!(anarchism.color, "0 0 0");
        assert_eq!(anarchism.types.len(), 2);
        assert_eq!(anarchism.types[0].id, "anarchism_ideology");
        assert_eq!(anarchism.types[0].can_be_randomly_selected, "");
        assert_eq!(anarchism.types[1].can_be_randomly_selected, "no");
        assert_eq!(
            anarchism.dynamic_faction_names,
            vec!["FACTION_NAME_ANARCHISM_1", "FACTION_NAME_ANARCHISM_2"]
        );
        assert_eq!(anarchism.rules.len(), 2);
        assert_eq!(anarchism.rules[1].key, "can_puppet");
        assert_eq!(anarchism.rules[1].value, "no");
        assert_eq!(
            anarchism.modifiers,
            "#generate_wargoal_tension =\n#join_faction_tension ="
        );
        assert_eq!(anarchism.faction_modifiers, "");
        assert_eq!(anarchism.war_impact_on_world_tension, "1");
        assert_eq!(anarchism.can_be_boosted, "no");
        assert_eq!(anarchism.ai_behaviour, "");

        let democratic = &file.ideologies[1];
        assert_eq!(democratic.ai_behaviour, "democratic");
        assert_eq!(democratic.ai_ideology_wanted_units_factor, "1.10");
        assert_eq!(democratic.war_impact_on_world_tension, "0.25");
        assert_eq!(democratic.modifiers, "generate_wargoal_tension = 1.00");
    }

    #[test]
    fn a_no_op_save_changes_nothing() {
        let file = parsed(FILE);
        for ideology in &file.ideologies {
            let text = update_source("(test)", FILE, &ideology.id, &update_from(ideology))
                .expect("updates");
            assert_eq!(text, FILE, "{} was rewritten", ideology.id);
        }
    }

    #[test]
    fn edits_fields_in_place_and_keeps_comments() {
        let file = parsed(FILE);
        let mut change = update_from(&file.ideologies[0]);
        change.color = "10 20 30".into();
        change.war_impact_on_world_tension = "0.5".into();
        change.can_be_boosted = "yes".into();
        change.can_collaborate = "no".into();
        change.rules[1].value = "yes".into();
        change.types[1].can_be_randomly_selected = String::new();

        let text = update_source("(test)", FILE, "anarchism", &change).expect("updates");

        assert!(text.contains("\t\tcolor = { 10 20 30 }\n"), "{text}");
        assert!(
            text.contains("war_impact_on_world_tension = 0.5\n"),
            "{text}"
        );
        assert!(text.contains("can_be_boosted = yes\n"), "{text}");
        assert!(text.contains("\t\tcan_collaborate = no\n"), "{text}");
        assert!(
            text.contains("can_puppet = yes # never"),
            "rule comment lost:\n{text}"
        );
        assert!(text.contains("#no state"), "{text}");
        assert!(
            text.contains("#generate_wargoal_tension = \n"),
            "modifier menu lost:\n{text}"
        );
        assert!(!text.contains("can_be_randomly_selected"), "{text}");
        assert!(text.contains("\t\t\tanarchy = {\n\t\t\t}\n"), "{text}");
        assert!(
            text.contains("# no major danger"),
            "other ideology touched:\n{text}"
        );
    }

    #[test]
    fn sub_ideologies_rules_and_names_are_keyed() {
        let file = parsed(FILE);
        let mut change = update_from(&file.ideologies[1]);
        change.types.push(SubIdeology {
            id: "socialism".into(),
            can_be_randomly_selected: "no".into(),
        });
        change.types.remove(0);
        change.rules.push(Rule {
            key: "can_puppet".into(),
            value: "no".into(),
        });
        change.rules.remove(0);
        change.dynamic_faction_names = vec!["FACTION_NAME_DEMOCRATIC_1".into()];
        change.ai_behaviour = "neutral".into();

        let text = update_source("(test)", FILE, "democratic", &change).expect("updates");
        let democratic = &parsed(&text).ideologies[1];
        assert_eq!(
            democratic
                .types
                .iter()
                .map(|sub| sub.id.as_str())
                .collect::<Vec<_>>(),
            vec!["liberalism", "socialism"]
        );
        assert_eq!(democratic.types[1].can_be_randomly_selected, "no");
        assert_eq!(democratic.rules.len(), 1);
        assert_eq!(democratic.rules[0].key, "can_puppet");
        assert_eq!(
            democratic.dynamic_faction_names,
            vec!["FACTION_NAME_DEMOCRATIC_1"]
        );
        assert_eq!(democratic.ai_behaviour, "neutral");
        assert!(!text.contains("ai_democratic"), "{text}");
        assert!(text.contains("\t\tai_neutral = yes\n"), "{text}");
        assert!(
            text.contains("\t\t\tsocialism = { can_be_randomly_selected = no }\n"),
            "{text}"
        );
        assert!(
            text.contains(
                "\t\tdynamic_faction_names = {\n\t\t\t\"FACTION_NAME_DEMOCRATIC_1\"\n\t\t}\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_colour_spelled_differently_is_left_alone() {
        let source = "ideologies = {\n\tx = {\n\t\tcolor = {  12   34 56  }\n\t}\n}\n";
        let file = parsed(source);
        assert_eq!(file.ideologies[0].color, "12   34 56");

        let mut change = update_from(&file.ideologies[0]);
        change.color = "12 34 56".into();
        assert_eq!(
            update_source("(test)", source, "x", &change).expect("updates"),
            source
        );
    }

    #[test]
    fn renames_the_block_key() {
        let file = parsed(FILE);
        let mut change = update_from(&file.ideologies[0]);
        change.id = "anarchy_group".into();

        let text = update_source("(test)", FILE, "anarchism", &change).expect("updates");
        let reread = parsed(&text);
        assert_eq!(reread.ideologies[0].id, "anarchy_group");
        assert!(text.contains("\tanarchy_group = {\t#no state\n"), "{text}");

        let mut clash = update_from(&reread.ideologies[0]);
        clash.id = "democratic".into();
        assert!(update_source("(test)", &text, "anarchy_group", &clash).is_err());
    }

    #[test]
    fn adds_and_deletes_an_ideology() {
        let added = add_source(
            "(test)",
            FILE,
            &IdeologyUpdate {
                id: "monarchism".into(),
                color: "200 160 40".into(),
                types: vec![SubIdeology {
                    id: "absolute_monarchism".into(),
                    can_be_randomly_selected: String::new(),
                }],
                dynamic_faction_names: vec!["FACTION_NAME_MONARCHISM_1".into()],
                war_impact_on_world_tension: "0.5".into(),
                faction_impact_on_world_tension: "0.5".into(),
                can_host_government_in_exile: "yes".into(),
                ai_behaviour: "neutral".into(),
                rules: vec![Rule {
                    key: "can_puppet".into(),
                    value: "yes".into(),
                }],
                modifiers: "generate_wargoal_tension = 0.5".into(),
                ..IdeologyUpdate::default()
            },
        )
        .expect("adds");

        let reread = parsed(&added);
        assert_eq!(reread.ideologies.len(), 3);
        let monarchism = &reread.ideologies[2];
        assert_eq!(monarchism.id, "monarchism");
        assert_eq!(monarchism.color, "200 160 40");
        assert_eq!(monarchism.types[0].id, "absolute_monarchism");
        assert_eq!(monarchism.rules[0].key, "can_puppet");
        assert_eq!(monarchism.ai_behaviour, "neutral");
        assert!(
            added.contains("#no state"),
            "the rest was disturbed:\n{added}"
        );
        assert!(
            added.contains("\n\tmonarchism = {\n\t\tcolor = { 200 160 40 }\n\t\ttypes = {\n\t\t\tabsolute_monarchism = { }\n\t\t}\n\t\tdynamic_faction_names = {\n\t\t\t\"FACTION_NAME_MONARCHISM_1\"\n\t\t}\n\t\trules = {\n\t\t\tcan_puppet = yes\n\t\t}\n\t\tmodifiers = { generate_wargoal_tension = 0.5 }\n\t\twar_impact_on_world_tension = 0.5\n"),
            "{added}"
        );

        let deleted = delete_source("(test)", &added, "anarchism").expect("deletes");
        let reread = parsed(&deleted);
        assert_eq!(reread.ideologies.len(), 2);
        assert!(!deleted.contains("anarchism = {"), "{deleted}");
        assert!(deleted.contains("monarchism"), "{deleted}");
    }

    #[test]
    fn refuses_bad_ids_and_behaviours() {
        let file = parsed(FILE);
        let mut change = update_from(&file.ideologies[0]);
        change.id = "two words".into();
        assert!(update_source("(test)", FILE, "anarchism", &change).is_err());

        let mut change = update_from(&file.ideologies[0]);
        change.types[0].id = String::new();
        assert!(update_source("(test)", FILE, "anarchism", &change).is_err());

        let mut change = update_from(&file.ideologies[0]);
        change.ai_behaviour = "monarchist".into();
        assert!(update_source("(test)", FILE, "anarchism", &change).is_err());
    }
}
