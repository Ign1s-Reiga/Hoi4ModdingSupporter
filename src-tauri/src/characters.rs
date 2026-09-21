//! Characters: `common/characters/*.txt`, the people a country recruits as
//! leaders, advisors and commanders.
//!
//! A character is one block keyed by its id inside `characters = { }`. Its
//! roles are blocks of their own, and a character may carry several of a
//! kind: a leader for two ideologies, or an advisor in two slots. Each role
//! is edited in place and matched to the form by position, so the file's
//! order is the form's order.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paradox::edit::{
    apply_edits, block_body, child_indent, dedent, detect_newline, format_block, indent_at,
    insert_lines, remove_pair, BlockEditor, TextEdit,
};
use crate::paradox::lexer::quote;
use crate::paradox::{self, Block, Document, Item, Items, Pair, Span, Value};
use crate::text_file;

/// The military roles, as the file spells them.
pub const COMMANDER_KINDS: &[&str] = &["corps_commander", "field_marshal", "navy_leader"];

/// The three portrait groups, each with a `large` and a `small` picture.
const PORTRAIT_GROUPS: &[&str] = &["civilian", "army", "navy"];

/// Advisor fields that hold a trigger or a weight block.
const ADVISOR_BLOCKS: &[&str] = &["allowed", "available", "visible", "ai_will_do"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Portraits {
    pub civilian_large: String,
    pub civilian_small: String,
    pub army_large: String,
    pub army_small: String,
    pub navy_large: String,
    pub navy_small: String,
}

impl Portraits {
    fn group(&self, name: &str) -> (&str, &str) {
        match name {
            "civilian" => (&self.civilian_large, &self.civilian_small),
            "army" => (&self.army_large, &self.army_small),
            _ => (&self.navy_large, &self.navy_small),
        }
    }

    fn is_empty(&self) -> bool {
        PORTRAIT_GROUPS.iter().all(|group| {
            let (large, small) = self.group(group);
            large.trim().is_empty() && small.trim().is_empty()
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CountryLeader {
    pub ideology: String,
    pub traits: Vec<String>,
    /// A date such as `1965.1.1.1`, written quoted as the game's files do.
    pub expire: String,
    pub desc: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Advisor {
    pub slot: String,
    pub idea_token: String,
    pub ledger: String,
    pub cost: String,
    pub removal_cost: String,
    /// `yes`, `no`, or empty when the file does not say.
    pub can_be_fired: String,
    pub traits: Vec<String>,
    pub allowed: String,
    pub available: String,
    pub visible: String,
    pub ai_will_do: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Commander {
    /// `corps_commander`, `field_marshal` or `navy_leader`.
    pub kind: String,
    pub skill: String,
    pub attack_skill: String,
    pub defense_skill: String,
    /// Army only; a navy leader has `maneuvering_skill` instead.
    pub planning_skill: String,
    /// Army only; a navy leader has `coordination_skill` instead.
    pub logistics_skill: String,
    pub maneuvering_skill: String,
    pub coordination_skill: String,
    pub legacy_id: String,
    pub traits: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub id: String,
    pub name: String,
    pub gender: String,
    pub portraits: Portraits,
    pub allowed_civil_war: String,
    pub leaders: Vec<CountryLeader>,
    pub advisors: Vec<Advisor>,
    pub commanders: Vec<Commander>,
    /// Defined through `instance` blocks, one per DLC set-up. The fields
    /// above come from the first of them; the form leaves such a character
    /// to the code view rather than guess which instance to change.
    pub has_instances: bool,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterFile {
    pub path: String,
    pub characters: Vec<Character>,
    pub has_bom: bool,
    pub encoding: text_file::Encoding,
    /// The text the characters were read from, for the code view.
    pub source: String,
}

/// The editable fields sent back when saving. Empty strings clear a field;
/// a role left out of its list is removed from the file.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CharacterUpdate {
    pub id: String,
    pub name: String,
    pub gender: String,
    pub portraits: Portraits,
    pub allowed_civil_war: String,
    pub leaders: Vec<CountryLeader>,
    pub advisors: Vec<Advisor>,
    pub commanders: Vec<Commander>,
}

pub fn read(path: &str) -> AppResult<CharacterFile> {
    let file = text_file::read(Path::new(path))?;
    parse(path, file.content, file.has_bom, file.encoding)
}

/// Reads characters out of text the window holds, which need not be what is
/// on disk: the code view parses its buffer as the user types.
pub fn parse(
    path: &str,
    content: String,
    has_bom: bool,
    encoding: text_file::Encoding,
) -> AppResult<CharacterFile> {
    let document = paradox::parse(&content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let characters = character_pairs(&document)
        .into_iter()
        .filter_map(|pair| {
            pair.value
                .as_block()
                .map(|block| read_character(&content, &pair.key, block, pair.span.start))
        })
        .collect();

    Ok(CharacterFile {
        path: path.replace('\\', "/"),
        characters,
        has_bom,
        encoding,
        source: content,
    })
}

/// Every `id = { }` pair inside the file's `characters` blocks, in file order.
fn character_pairs(document: &Document) -> Vec<&Pair> {
    document
        .find_all("characters")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .flat_map(|block| block.pairs())
        .filter(|pair| pair.value.as_block().is_some())
        .collect()
}

fn read_character(source: &str, id: &str, block: &Block, start: usize) -> Character {
    let instance = block.block("instance");
    let body = instance.unwrap_or(block);

    let leaders = body
        .find_all("country_leader")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .map(|leader| CountryLeader {
            ideology: leader.scalar("ideology").unwrap_or_default(),
            traits: read_words(leader, "traits"),
            expire: leader.scalar("expire").unwrap_or_default(),
            desc: leader.scalar("desc").unwrap_or_default(),
        })
        .collect();

    let advisors = body
        .find_all("advisor")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .map(|advisor| Advisor {
            slot: advisor.scalar("slot").unwrap_or_default(),
            idea_token: advisor.scalar("idea_token").unwrap_or_default(),
            ledger: advisor.scalar("ledger").unwrap_or_default(),
            cost: advisor.scalar("cost").unwrap_or_default(),
            removal_cost: advisor.scalar("removal_cost").unwrap_or_default(),
            can_be_fired: advisor.scalar("can_be_fired").unwrap_or_default(),
            traits: read_words(advisor, "traits"),
            allowed: read_block(source, advisor, "allowed"),
            available: read_block(source, advisor, "available"),
            visible: read_block(source, advisor, "visible"),
            ai_will_do: read_block(source, advisor, "ai_will_do"),
        })
        .collect();

    // Kinds are read in the file's order rather than grouped, so the form
    // shows the roles the way the author listed them.
    let commanders = body
        .pairs()
        .into_iter()
        .filter(|pair| {
            COMMANDER_KINDS
                .iter()
                .any(|kind| pair.key.eq_ignore_ascii_case(kind))
        })
        .filter_map(|pair| {
            pair.value
                .as_block()
                .map(|role| (pair.key.to_lowercase(), role))
        })
        .map(|(kind, role)| Commander {
            kind,
            skill: role.scalar("skill").unwrap_or_default(),
            attack_skill: role.scalar("attack_skill").unwrap_or_default(),
            defense_skill: role.scalar("defense_skill").unwrap_or_default(),
            planning_skill: role.scalar("planning_skill").unwrap_or_default(),
            logistics_skill: role.scalar("logistics_skill").unwrap_or_default(),
            maneuvering_skill: role.scalar("maneuvering_skill").unwrap_or_default(),
            coordination_skill: role.scalar("coordination_skill").unwrap_or_default(),
            legacy_id: role.scalar("legacy_id").unwrap_or_default(),
            traits: read_words(role, "traits"),
        })
        .collect();

    let mut portraits = Portraits::default();
    if let Some(block) = body.block("portraits") {
        for group in PORTRAIT_GROUPS {
            let Some(pictures) = block.block(group) else {
                continue;
            };
            let large = pictures.scalar("large").unwrap_or_default();
            let small = pictures.scalar("small").unwrap_or_default();
            match *group {
                "civilian" => (portraits.civilian_large, portraits.civilian_small) = (large, small),
                "army" => (portraits.army_large, portraits.army_small) = (large, small),
                _ => (portraits.navy_large, portraits.navy_small) = (large, small),
            }
        }
    }

    Character {
        id: id.to_string(),
        name: body.scalar("name").unwrap_or_default(),
        gender: body.scalar("gender").unwrap_or_default(),
        portraits,
        allowed_civil_war: read_block(source, body, "allowed_civil_war"),
        leaders,
        advisors,
        commanders,
        has_instances: instance.is_some(),
        line: paradox::line_of(source, start),
    }
}

/// The bare words of a list such as `traits = { a b }`.
fn read_words(block: &Block, key: &str) -> Vec<String> {
    block.block(key).map(words_of).unwrap_or_default()
}

fn words_of(block: &Block) -> Vec<String> {
    block
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Value(Value::Scalar(scalar)) => Some(scalar.value()),
            _ => None,
        })
        .collect()
}

fn read_block(source: &str, block: &Block, key: &str) -> String {
    block
        .find(key)
        .map(|pair| block_body(source, &pair.value))
        .unwrap_or_default()
}

/// Rewrites one character in place, matched by its current id.
pub fn update(
    path: &str,
    character_id: &str,
    update: &CharacterUpdate,
) -> AppResult<CharacterFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = update_source(path, &file.content, character_id, update)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`update`], applied to text instead of a file.
pub fn update_source(
    path: &str,
    source: &str,
    character_id: &str,
    update: &CharacterUpdate,
) -> AppResult<String> {
    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let pair = find_character(&document, character_id).ok_or_else(|| {
        AppError::message(format!(
            "no character with id `{character_id}` exists in {path}"
        ))
    })?;
    let block = pair
        .value
        .as_block()
        .ok_or_else(|| AppError::message(format!("`{character_id}` is not a character block")))?;

    if block.block("instance").is_some() {
        return Err(AppError::message(format!(
            "`{character_id}` is defined through instance blocks; edit it in the code view"
        )));
    }

    validate(update)?;

    let new_id = update.id.trim();
    let mut edits = Vec::new();
    if new_id != character_id {
        if find_character(&document, new_id).is_some() {
            return Err(AppError::message(format!(
                "a character with id `{new_id}` already exists in this file"
            )));
        }
        // The id is the block's key, which sits at the very start of the pair.
        let key = Span::new(pair.span.start, pair.span.start + character_id.len());
        if key.text(source) != character_id {
            return Err(AppError::message(format!(
                "the id of `{character_id}` is written in a way that cannot be renamed"
            )));
        }
        edits.push(TextEdit::new(key, new_id));
    }

    let mut editor = BlockEditor::new(source, block);
    editor.set_scalar("name", &update.name);
    editor.set_scalar("gender", &update.gender);
    set_portraits(&mut editor, &update.portraits);
    editor.set_block("allowed_civil_war", &update.allowed_civil_war);

    set_roles(
        &mut editor,
        "country_leader",
        &update.leaders,
        edit_leader,
        render_leader,
    );
    set_roles(
        &mut editor,
        "advisor",
        &update.advisors,
        edit_advisor,
        render_advisor,
    );
    for kind in COMMANDER_KINDS {
        let wanted: Vec<Commander> = update
            .commanders
            .iter()
            .filter(|commander| commander.kind == *kind)
            .cloned()
            .collect();
        set_roles(&mut editor, kind, &wanted, edit_commander, render_commander);
    }

    edits.extend(editor.finish());
    Ok(apply_edits(source, edits))
}

/// Appends a character to the file's `characters` block, creating the block
/// in a file that has none.
pub fn add(path: &str, character: &CharacterUpdate) -> AppResult<CharacterFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = add_source(path, &file.content, character)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`add`], applied to text instead of a file.
pub fn add_source(path: &str, source: &str, character: &CharacterUpdate) -> AppResult<String> {
    validate(character)?;

    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let id = character.id.trim();
    if find_character(&document, id).is_some() {
        return Err(AppError::message(format!(
            "a character with id `{id}` already exists in this file"
        )));
    }

    let newline = detect_newline(source);
    match document.block("characters") {
        Some(block) => {
            let indent = child_indent(source, block);
            let line = format!(
                "{id} = {}",
                format_lines(&render_character(character), &indent, newline)
            );
            Ok(apply_edits(
                source,
                vec![insert_lines(source, block, &[line], &[])],
            ))
        }
        None => {
            // `set_block` indents the body for the new block, so this one is
            // rendered flush left like every other body handed to it.
            let mut editor = BlockEditor::for_document(source, &document);
            let body = format!(
                "{id} = {}",
                format_lines(&render_character(character), "", "\n")
            );
            editor.set_block("characters", &body);
            Ok(apply_edits(source, editor.finish()))
        }
    }
}

pub fn delete(path: &str, character_id: &str) -> AppResult<CharacterFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = delete_source(path, &file.content, character_id)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`delete`], applied to text instead of a file.
pub fn delete_source(path: &str, source: &str, character_id: &str) -> AppResult<String> {
    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let pair = find_character(&document, character_id).ok_or_else(|| {
        AppError::message(format!(
            "no character with id `{character_id}` exists in {path}"
        ))
    })?;

    Ok(apply_edits(source, remove_pair(source, pair)))
}

fn find_character<'a>(document: &'a Document, id: &str) -> Option<&'a Pair> {
    character_pairs(document)
        .into_iter()
        .find(|pair| pair.key == id)
}

fn validate(update: &CharacterUpdate) -> AppResult<()> {
    let id = update.id.trim();
    if id.is_empty() {
        return Err(AppError::message("a character needs an id"));
    }
    if id
        .chars()
        .any(|character| character.is_whitespace() || "{}=\"#".contains(character))
    {
        return Err(AppError::message(format!(
            "`{id}` cannot be a character id: use letters, digits and underscores"
        )));
    }

    for commander in &update.commanders {
        if !COMMANDER_KINDS.contains(&commander.kind.as_str()) {
            return Err(AppError::message(format!(
                "`{}` is not a commander role; use one of {}",
                commander.kind,
                COMMANDER_KINDS.join(", ")
            )));
        }
    }

    Ok(())
}

/// Sets the six portrait pictures, editing the groups that are there and
/// adding or dropping the ones that are not.
fn set_portraits(editor: &mut BlockEditor<'_>, portraits: &Portraits) {
    let source = editor.source();
    let newline = detect_newline(source);
    let existing = editor.find_all("portraits").into_iter().next();

    let Some(pair) = existing else {
        if !portraits.is_empty() {
            let indent = editor
                .block()
                .map(|block| child_indent(source, block))
                .unwrap_or_default();
            editor.add_line(format!(
                "portraits = {}",
                format_lines(&render_portraits(portraits), &indent, newline)
            ));
        }
        return;
    };

    if portraits.is_empty() {
        editor.remove(pair);
        return;
    }

    let Some(block) = pair.value.as_block() else {
        let indent = indent_at(source, pair.span.start);
        editor.edit(TextEdit::new(
            pair.value.span(),
            format_lines(&render_portraits(portraits), &indent, newline),
        ));
        return;
    };

    let mut pictures = BlockEditor::new(source, block);
    for group in PORTRAIT_GROUPS {
        let (large, small) = portraits.group(group);
        let wanted = !large.trim().is_empty() || !small.trim().is_empty();

        match block.find(group) {
            Some(group_pair) => match group_pair.value.as_block() {
                Some(group_block) if wanted => {
                    let mut picture = BlockEditor::new(source, group_block);
                    picture.set_scalar("large", large);
                    picture.set_scalar("small", small);
                    for edit in picture.finish() {
                        pictures.edit(edit);
                    }
                }
                _ if wanted => {
                    let indent = indent_at(source, group_pair.span.start);
                    pictures.edit(TextEdit::new(
                        group_pair.value.span(),
                        format_lines(&render_portrait_group(large, small), &indent, newline),
                    ));
                }
                _ => {
                    pictures.remove(group_pair);
                }
            },
            None if wanted => {
                let indent = child_indent(source, block);
                pictures.add_line(format!(
                    "{group} = {}",
                    format_lines(&render_portrait_group(large, small), &indent, newline)
                ));
            }
            None => {}
        }
    }

    for edit in pictures.finish() {
        editor.edit(edit);
    }
}

/// Writes a repeated role block, editing the blocks already there by
/// position, dropping the extra ones and appending the new ones.
fn set_roles<T>(
    editor: &mut BlockEditor<'_>,
    key: &str,
    wanted: &[T],
    edit_existing: impl Fn(&mut BlockEditor<'_>, &T),
    render: impl Fn(&T) -> String,
) {
    let source = editor.source();
    let existing = editor.find_all(key);
    let newline = detect_newline(source);

    for (role, pair) in wanted.iter().zip(existing.iter()) {
        match pair.value.as_block() {
            Some(block) => {
                let mut inner = BlockEditor::new(source, block);
                edit_existing(&mut inner, role);
                for edit in inner.finish() {
                    editor.edit(edit);
                }
            }
            None => {
                let indent = indent_at(source, pair.span.start);
                editor.edit(TextEdit::new(
                    pair.value.span(),
                    format_lines(&render(role), &indent, newline),
                ));
            }
        }
    }

    for pair in existing.iter().skip(wanted.len()) {
        editor.remove(pair);
    }

    let indent = editor
        .block()
        .map(|block| child_indent(source, block))
        .unwrap_or_default();
    for role in wanted.iter().skip(existing.len()) {
        editor.add_line(format!(
            "{key} = {}",
            format_lines(&render(role), &indent, newline)
        ));
    }
}

fn edit_leader(editor: &mut BlockEditor<'_>, leader: &CountryLeader) {
    editor.set_scalar("ideology", &leader.ideology);
    set_words(editor, "traits", &leader.traits);
    set_quoted(editor, "expire", &leader.expire);
    editor.set_scalar("desc", &leader.desc);
}

fn edit_advisor(editor: &mut BlockEditor<'_>, advisor: &Advisor) {
    editor.set_scalar("slot", &advisor.slot);
    editor.set_scalar("idea_token", &advisor.idea_token);
    editor.set_scalar("ledger", &advisor.ledger);
    editor.set_scalar("cost", &advisor.cost);
    editor.set_scalar("removal_cost", &advisor.removal_cost);
    editor.set_scalar("can_be_fired", &advisor.can_be_fired);
    set_words(editor, "traits", &advisor.traits);
    for (key, value) in ADVISOR_BLOCKS.iter().zip([
        &advisor.allowed,
        &advisor.available,
        &advisor.visible,
        &advisor.ai_will_do,
    ]) {
        editor.set_block(key, value);
    }
}

fn edit_commander(editor: &mut BlockEditor<'_>, commander: &Commander) {
    for (key, value) in commander_skills(commander) {
        editor.set_scalar(key, value);
    }
    editor.set_scalar("legacy_id", &commander.legacy_id);
    set_words(editor, "traits", &commander.traits);
}

/// The skill fields a role of this kind carries. A navy leader has no
/// planning or logistics, an army commander no maneuvering or coordination;
/// the other pair is cleared so a role that changed kind sheds them.
fn commander_skills(commander: &Commander) -> Vec<(&'static str, &str)> {
    fn pick(value: &str, wanted: bool) -> &str {
        if wanted {
            value
        } else {
            ""
        }
    }

    let navy = commander.kind == "navy_leader";
    vec![
        ("skill", commander.skill.as_str()),
        ("attack_skill", commander.attack_skill.as_str()),
        ("defense_skill", commander.defense_skill.as_str()),
        ("planning_skill", pick(&commander.planning_skill, !navy)),
        ("logistics_skill", pick(&commander.logistics_skill, !navy)),
        (
            "maneuvering_skill",
            pick(&commander.maneuvering_skill, navy),
        ),
        (
            "coordination_skill",
            pick(&commander.coordination_skill, navy),
        ),
    ]
}

/// Sets a bare word list such as `traits = { a b }`. A list already
/// holding exactly these words is left alone, whatever its layout.
fn set_words(editor: &mut BlockEditor<'_>, key: &str, values: &[String]) {
    let wanted = normalise_words(values);
    let source = editor.source();
    let newline = detect_newline(source);

    let Some(pair) = editor.find_all(key).into_iter().next() else {
        if !wanted.is_empty() {
            let indent = editor
                .block()
                .map(|block| child_indent(source, block))
                .unwrap_or_default();
            editor.add_line(format!(
                "{key} = {}",
                format_block(&render_words(&wanted), &indent, newline)
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

    // A cleared list stays as `{ }` rather than vanishing: the game's own
    // generic commanders carry an empty `traits = { }`, and the key is
    // what a reader looks for.
    let indent = indent_at(source, pair.span.start);
    editor.edit(TextEdit::new(
        pair.value.span(),
        format_block(&render_words(&wanted), &indent, newline),
    ));
}

/// Sets a value the game's own files always quote, such as an `expire` date.
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

fn normalise_words(values: &[String]) -> Vec<String> {
    values
        .iter()
        .flat_map(|value| value.split_whitespace())
        .map(str::to_string)
        .collect()
}

/// Short lists stay on one line; long ones get a word per line.
fn render_words(words: &[String]) -> String {
    let joined = words.join(" ");
    if joined.len() <= 60 {
        joined
    } else {
        words.join("\n")
    }
}

fn render_portrait_group(large: &str, small: &str) -> String {
    [("large", large), ("small", small)]
        .into_iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(key, value)| format!("{key} = {}", value.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_portraits(portraits: &Portraits) -> String {
    PORTRAIT_GROUPS
        .iter()
        .filter_map(|group| {
            let (large, small) = portraits.group(group);
            let body = render_portrait_group(large, small);
            (!body.is_empty()).then(|| format!("{group} = {}", format_lines(&body, "", "\n")))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A block with its braces on lines of their own however short the body,
/// which is how every character, role and portrait block in the game's own
/// files is laid out. `format_block` would fold a one-field role onto one
/// line.
fn format_lines(content: &str, indent: &str, newline: &str) -> String {
    let inner = format!("{indent}\t");
    let body = dedent(content.trim())
        .iter()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{inner}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join(newline);

    format!("{{{newline}{body}{newline}{indent}}}")
}

// The renderers below produce a block body with `\n` line breaks and no
// leading indentation; the formatter re-indents it wherever it lands and
// swaps in the file's own newline.

fn render_scalars(fields: &[(&str, &str)]) -> Vec<String> {
    fields
        .iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(key, value)| format!("{key} = {}", value.trim()))
        .collect()
}

fn render_leader(leader: &CountryLeader) -> String {
    let mut lines = render_scalars(&[("ideology", leader.ideology.as_str())]);
    let traits = normalise_words(&leader.traits);
    if !traits.is_empty() {
        lines.push(format!(
            "traits = {}",
            format_block(&render_words(&traits), "", "\n")
        ));
    }
    if !leader.expire.trim().is_empty() {
        lines.push(format!("expire = {}", quote(leader.expire.trim())));
    }
    lines.extend(render_scalars(&[("desc", leader.desc.as_str())]));
    lines.join("\n")
}

fn render_advisor(advisor: &Advisor) -> String {
    let mut lines = render_scalars(&[
        ("slot", advisor.slot.as_str()),
        ("idea_token", advisor.idea_token.as_str()),
        ("ledger", advisor.ledger.as_str()),
        ("cost", advisor.cost.as_str()),
        ("removal_cost", advisor.removal_cost.as_str()),
        ("can_be_fired", advisor.can_be_fired.as_str()),
    ]);
    for (key, value) in ADVISOR_BLOCKS.iter().zip([
        &advisor.allowed,
        &advisor.available,
        &advisor.visible,
        &advisor.ai_will_do,
    ]) {
        if !value.trim().is_empty() {
            lines.push(format!("{key} = {}", format_block(value, "", "\n")));
        }
    }
    let traits = normalise_words(&advisor.traits);
    if !traits.is_empty() {
        lines.push(format!(
            "traits = {}",
            format_block(&render_words(&traits), "", "\n")
        ));
    }
    lines.join("\n")
}

fn render_commander(commander: &Commander) -> String {
    let skills = commander_skills(commander);
    let mut lines = render_scalars(&skills);
    lines.extend(render_scalars(&[(
        "legacy_id",
        commander.legacy_id.as_str(),
    )]));
    let traits = normalise_words(&commander.traits);
    if !traits.is_empty() {
        lines.push(format!(
            "traits = {}",
            format_block(&render_words(&traits), "", "\n")
        ));
    }
    lines.join("\n")
}

fn render_character(character: &CharacterUpdate) -> String {
    let mut lines = render_scalars(&[
        ("name", character.name.as_str()),
        ("gender", character.gender.as_str()),
    ]);

    let portraits = render_portraits(&character.portraits);
    if !portraits.is_empty() {
        lines.push(format!(
            "portraits = {}",
            format_lines(&portraits, "", "\n")
        ));
    }
    if !character.allowed_civil_war.trim().is_empty() {
        lines.push(format!(
            "allowed_civil_war = {}",
            format_block(&character.allowed_civil_war, "", "\n")
        ));
    }
    for leader in &character.leaders {
        lines.push(format!(
            "country_leader = {}",
            format_lines(&render_leader(leader), "", "\n")
        ));
    }
    for advisor in &character.advisors {
        lines.push(format!(
            "advisor = {}",
            format_lines(&render_advisor(advisor), "", "\n")
        ));
    }
    for commander in &character.commanders {
        lines.push(format!(
            "{} = {}",
            commander.kind,
            format_lines(&render_commander(commander), "", "\n")
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two characters the way a mod writes them: quoted portraits, a leader
    /// with traits, an advisor with triggers, a commander with odd indentation.
    const FILE: &str = "characters = {\n\tTST_leader = {\n\t\tname = TST_leader # the boss\n\t\tportraits = {\n\t\t\tcivilian = {\n\t\t\t\tlarge = \"GFX_portrait_TST_leader\"\n\t\t\t}\n\t\t}\n\t\tcountry_leader = {\n\t\t\tideology = fascism_ideology\n\t\t\ttraits = { dictator }\n\t\t\texpire = \"1965.1.1.1\"\n\t\t\tid = -1\n\t\t}\n\t}\n\n\tTST_general = {\n\t\tname = TST_general\n\t\tportraits = {\n\t\t\tarmy = {\n\t\t\t\tlarge = GFX_portrait_TST_general\n\t\t\t\tsmall = GFX_portrait_TST_general_small\n\t\t\t}\n\t\t}\n\t\tcorps_commander = {\n\t\ttraits = { trait_reckless }\n\t\tskill = 2\n\t    attack_skill = 2\n\t    defense_skill = 3\n\t    planning_skill = 1\n\t    logistics_skill = 2\n\t\t}\n\t\tadvisor = {\n\t\t\tslot = political_advisor\n\t\t\tidea_token = TST_general\n\t\t\tallowed = {\n\t\t\t\toriginal_tag = TST # only at home\n\t\t\t}\n\t\t\ttraits = {\n\t\t\t\tsilent_workhorse\n\t\t\t}\n\t\t}\n\t}\n}\n";

    fn parsed(source: &str) -> CharacterFile {
        parse(
            "(test)",
            source.to_string(),
            false,
            text_file::Encoding::Utf8,
        )
        .expect("parses")
    }

    fn update_from(character: &Character) -> CharacterUpdate {
        CharacterUpdate {
            id: character.id.clone(),
            name: character.name.clone(),
            gender: character.gender.clone(),
            portraits: character.portraits.clone(),
            allowed_civil_war: character.allowed_civil_war.clone(),
            leaders: character.leaders.clone(),
            advisors: character.advisors.clone(),
            commanders: character.commanders.clone(),
        }
    }

    #[test]
    fn reads_every_role_and_the_portraits() {
        let file = parsed(FILE);
        assert_eq!(file.characters.len(), 2);

        let leader = &file.characters[0];
        assert_eq!(leader.id, "TST_leader");
        assert_eq!(leader.name, "TST_leader");
        assert_eq!(leader.line, 2);
        assert_eq!(leader.portraits.civilian_large, "GFX_portrait_TST_leader");
        assert_eq!(leader.leaders.len(), 1);
        assert_eq!(leader.leaders[0].ideology, "fascism_ideology");
        assert_eq!(leader.leaders[0].traits, vec!["dictator"]);
        assert_eq!(leader.leaders[0].expire, "1965.1.1.1");
        assert!(leader.advisors.is_empty());
        assert!(leader.commanders.is_empty());

        let general = &file.characters[1];
        assert_eq!(
            general.portraits.army_small,
            "GFX_portrait_TST_general_small"
        );
        assert_eq!(general.commanders.len(), 1);
        assert_eq!(general.commanders[0].kind, "corps_commander");
        assert_eq!(general.commanders[0].defense_skill, "3");
        assert_eq!(general.commanders[0].traits, vec!["trait_reckless"]);
        assert_eq!(general.advisors.len(), 1);
        assert_eq!(general.advisors[0].slot, "political_advisor");
        assert_eq!(
            general.advisors[0].allowed,
            "original_tag = TST # only at home"
        );
        assert_eq!(general.advisors[0].traits, vec!["silent_workhorse"]);
    }

    #[test]
    fn a_no_op_save_changes_nothing() {
        let file = parsed(FILE);
        for character in &file.characters {
            let text = update_source("(test)", FILE, &character.id, &update_from(character))
                .expect("updates");
            assert_eq!(text, FILE, "{} was rewritten", character.id);
        }
    }

    #[test]
    fn edits_a_role_field_in_place_and_keeps_its_neighbours() {
        let file = parsed(FILE);
        let mut change = update_from(&file.characters[1]);
        change.commanders[0].skill = "4".into();
        change.advisors[0].traits = vec!["silent_workhorse".into(), "TST_veteran".into()];

        let text = update_source("(test)", FILE, "TST_general", &change).expect("updates");

        assert!(text.contains("\t\tskill = 4\n"), "{text}");
        assert!(
            text.contains("\t    attack_skill = 2\n"),
            "odd indentation lost:\n{text}"
        );
        assert!(
            text.contains("original_tag = TST # only at home"),
            "trigger comment lost:\n{text}"
        );
        assert!(
            text.contains("traits = { silent_workhorse TST_veteran }"),
            "{text}"
        );
        assert!(
            text.contains("# the boss"),
            "other character touched:\n{text}"
        );
    }

    #[test]
    fn expire_stays_quoted_and_unknown_keys_survive() {
        let file = parsed(FILE);
        let mut change = update_from(&file.characters[0]);
        change.leaders[0].expire = "1970.1.1.1".into();

        let text = update_source("(test)", FILE, "TST_leader", &change).expect("updates");

        assert!(text.contains("expire = \"1970.1.1.1\""), "{text}");
        assert!(text.contains("\t\t\tid = -1\n"), "{text}");
    }

    #[test]
    fn roles_are_added_dropped_and_matched_by_position() {
        let file = parsed(FILE);
        let mut change = update_from(&file.characters[0]);
        change.leaders.push(CountryLeader {
            ideology: "democratic_ideology".into(),
            traits: vec![],
            expire: "1965.1.1.1".into(),
            desc: String::new(),
        });
        change.commanders.push(Commander {
            kind: "navy_leader".into(),
            skill: "3".into(),
            attack_skill: "2".into(),
            defense_skill: "2".into(),
            maneuvering_skill: "3".into(),
            coordination_skill: "1".into(),
            ..Commander::default()
        });

        let text = update_source("(test)", FILE, "TST_leader", &change).expect("updates");
        let reread = parsed(&text);
        let leader = &reread.characters[0];
        assert_eq!(leader.leaders.len(), 2);
        assert_eq!(leader.leaders[1].ideology, "democratic_ideology");
        assert_eq!(leader.leaders[1].expire, "1965.1.1.1");
        assert_eq!(leader.commanders.len(), 1);
        assert_eq!(leader.commanders[0].kind, "navy_leader");
        assert_eq!(leader.commanders[0].maneuvering_skill, "3");
        assert!(!text.contains("planning_skill = \n"), "{text}");

        // Roles are matched by position: with the first leader gone, the
        // first block takes the second's fields and the second block goes.
        let mut change = update_from(leader);
        change.leaders.remove(0);
        let text = update_source("(test)", &text, "TST_leader", &change).expect("updates");
        let leader = &parsed(&text).characters[0];
        assert_eq!(leader.leaders.len(), 1);
        assert_eq!(leader.leaders[0].ideology, "democratic_ideology");
        assert!(!text.contains("fascism_ideology"), "{text}");
    }

    #[test]
    fn a_commander_that_changes_kind_swaps_its_skill_set() {
        let file = parsed(FILE);
        let mut change = update_from(&file.characters[1]);
        // Kinds are matched by key, so the corps_commander block goes and a
        // field_marshal block carrying the same skills is written.
        change.commanders[0].kind = "field_marshal".into();

        let text = update_source("(test)", FILE, "TST_general", &change).expect("updates");
        let general = &parsed(&text).characters[1];
        assert_eq!(general.commanders.len(), 1);
        assert_eq!(general.commanders[0].kind, "field_marshal");
        assert_eq!(general.commanders[0].planning_skill, "1");
        assert!(!text.contains("corps_commander"), "{text}");
    }

    #[test]
    fn portraits_are_edited_group_by_group() {
        let file = parsed(FILE);
        let mut change = update_from(&file.characters[1]);
        change.portraits.army_small = String::new();
        change.portraits.navy_large = "GFX_portrait_TST_general_navy".into();

        let text = update_source("(test)", FILE, "TST_general", &change).expect("updates");
        let general = &parsed(&text).characters[1];
        assert_eq!(general.portraits.army_large, "GFX_portrait_TST_general");
        assert_eq!(general.portraits.army_small, "");
        assert_eq!(
            general.portraits.navy_large,
            "GFX_portrait_TST_general_navy"
        );

        // Clearing every picture drops the whole block.
        let mut change = update_from(general);
        change.portraits = Portraits::default();
        let text = update_source("(test)", &text, "TST_general", &change).expect("updates");
        let general = &parsed(&text).characters[1];
        assert_eq!(general.portraits, Portraits::default());
        assert_eq!(text.matches("portraits").count(), 1, "{text}");
        assert!(text.contains("corps_commander"), "{text}");
    }

    #[test]
    fn renames_the_block_key() {
        let file = parsed(FILE);
        let mut change = update_from(&file.characters[0]);
        change.id = "TST_fuhrer".into();

        let text = update_source("(test)", FILE, "TST_leader", &change).expect("updates");
        let reread = parsed(&text);
        assert_eq!(reread.characters[0].id, "TST_fuhrer");
        assert_eq!(reread.characters[0].name, "TST_leader");

        let mut clash = update_from(&reread.characters[0]);
        clash.id = "TST_general".into();
        assert!(update_source("(test)", &text, "TST_fuhrer", &clash).is_err());
    }

    #[test]
    fn adds_and_deletes_a_character() {
        let added = add_source(
            "(test)",
            FILE,
            &CharacterUpdate {
                id: "TST_admiral".into(),
                name: "TST_admiral".into(),
                gender: "female".into(),
                portraits: Portraits {
                    navy_large: "GFX_portrait_TST_admiral".into(),
                    ..Portraits::default()
                },
                commanders: vec![Commander {
                    kind: "navy_leader".into(),
                    skill: "2".into(),
                    attack_skill: "1".into(),
                    defense_skill: "1".into(),
                    maneuvering_skill: "2".into(),
                    coordination_skill: "2".into(),
                    traits: vec!["bold".into()],
                    ..Commander::default()
                }],
                advisors: vec![Advisor {
                    slot: "high_command".into(),
                    idea_token: "TST_admiral".into(),
                    allowed: "original_tag = TST".into(),
                    traits: vec!["navy_capital_ship_1".into()],
                    ..Advisor::default()
                }],
                ..CharacterUpdate::default()
            },
        )
        .expect("adds");

        let reread = parsed(&added);
        assert_eq!(reread.characters.len(), 3);
        let admiral = &reread.characters[2];
        assert_eq!(admiral.id, "TST_admiral");
        assert_eq!(admiral.gender, "female");
        assert_eq!(admiral.portraits.navy_large, "GFX_portrait_TST_admiral");
        assert_eq!(admiral.commanders[0].coordination_skill, "2");
        assert_eq!(admiral.advisors[0].allowed, "original_tag = TST");
        assert!(
            added.contains("# the boss"),
            "the rest was disturbed:\n{added}"
        );
        assert!(
            added.contains("original_tag = TST # only at home"),
            "{added}"
        );
        assert!(
            added.contains("\n\tTST_admiral = {\n\t\tname = TST_admiral\n"),
            "{added}"
        );
        assert!(
            added.contains("\n\t\tnavy_leader = {\n\t\t\tskill = 2\n"),
            "{added}"
        );

        let deleted = delete_source("(test)", &added, "TST_general").expect("deletes");
        let reread = parsed(&deleted);
        assert_eq!(reread.characters.len(), 2);
        assert!(!deleted.contains("TST_general"), "{deleted}");
        assert!(deleted.contains("TST_admiral"), "{deleted}");
    }

    #[test]
    fn a_file_without_a_characters_block_gets_one() {
        let text = add_source(
            "(test)",
            "",
            &CharacterUpdate {
                id: "TST_new".into(),
                name: "TST_new".into(),
                ..CharacterUpdate::default()
            },
        )
        .expect("adds");

        assert_eq!(
            text,
            "characters = {\n\tTST_new = {\n\t\tname = TST_new\n\t}\n}\n"
        );
    }

    #[test]
    fn an_instance_character_is_read_but_not_edited() {
        let source = "characters = {\n\tTST_dlc = {\n\t\tinstance = {\n\t\t\tallowed = { has_dlc = \"La Resistance\" }\n\t\t\tname = TST_dlc\n\t\t\tadvisor = {\n\t\t\t\tslot = political_advisor\n\t\t\t\tidea_token = TST_dlc\n\t\t\t}\n\t\t}\n\t\tinstance = {\n\t\t\tname = TST_dlc\n\t\t}\n\t}\n}\n";
        let file = parsed(source);
        let character = &file.characters[0];
        assert!(character.has_instances);
        assert_eq!(character.name, "TST_dlc");
        assert_eq!(character.advisors.len(), 1);

        let error = update_source("(test)", source, "TST_dlc", &update_from(character))
            .expect_err("refuses");
        assert!(error.to_string().contains("instance"), "{error}");

        // The code view can still drop it.
        assert_eq!(
            delete_source("(test)", source, "TST_dlc").expect("deletes"),
            "characters = {\n}\n"
        );
    }

    #[test]
    fn refuses_ids_the_script_cannot_hold() {
        let file = parsed(FILE);
        let mut change = update_from(&file.characters[0]);
        change.id = "two words".into();
        assert!(update_source("(test)", FILE, "TST_leader", &change).is_err());

        let mut change = update_from(&file.characters[1]);
        change.commanders[0].kind = "general".into();
        assert!(update_source("(test)", FILE, "TST_general", &change).is_err());
    }
}
