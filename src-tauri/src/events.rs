//! Events: `events/*.txt`, the pop-ups a country, a state or a leader gets.
//!
//! An event is one `country_event = { }` (or `news_event`, `state_event`,
//! `unit_leader_event`, `operative_leader_event`) block at the top level of
//! a file that also declares its `add_namespace`. Its title and description
//! are usually one localisation key each, but either can be repeated as
//! `desc = { text = key trigger = { } }` blocks picked by trigger; both are
//! kept as a list of "key or block body" so a form shows what is there.
//! An option is a name plus whatever effects follow it; the effects are
//! handed over as text, since they are script rather than fields.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paradox::edit::{
    apply_edits, block_body, child_indent, dedent, detect_newline, format_block, indent_at,
    line_end, line_start, remove_pair, BlockEditor, TextEdit,
};
use crate::paradox::lexer::{needs_quotes, quote};
use crate::paradox::{self, Block, Document, Items, Pair, Span};
use crate::text_file;

/// The event kinds, as the file spells them.
pub const EVENT_KINDS: &[&str] = &[
    "country_event",
    "news_event",
    "state_event",
    "unit_leader_event",
    "operative_leader_event",
];

/// Flags the form offers as yes/no.
const FLAGS: &[&str] = &["fire_only_once", "is_triggered_only", "hidden", "major"];

/// Script blocks the form offers as text.
const BLOCKS: &[&str] = &["trigger", "mean_time_to_happen", "immediate"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventOption {
    /// Localisation key of the button text.
    pub name: String,
    /// Everything else in the option, effects and `ai_chance` and `trigger`
    /// alike, as script text.
    pub body: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// `country_event`, `news_event`, and so on.
    pub kind: String,
    pub id: String,
    /// Each a localisation key, or the body of a `title = { }` block when
    /// the title is picked by trigger.
    pub titles: Vec<String>,
    pub descs: Vec<String>,
    pub picture: String,
    /// `yes`, `no`, or empty when the file does not say.
    pub fire_only_once: String,
    pub is_triggered_only: String,
    pub hidden: String,
    pub major: String,
    pub timeout_days: String,
    pub trigger: String,
    pub mean_time_to_happen: String,
    pub immediate: String,
    pub options: Vec<EventOption>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventFile {
    pub path: String,
    /// Every `add_namespace` the file declares, in order.
    pub namespaces: Vec<String>,
    pub events: Vec<Event>,
    pub has_bom: bool,
    pub encoding: text_file::Encoding,
    /// The text the events were read from, for the code view.
    pub source: String,
}

/// The editable fields sent back when saving. Empty strings clear a field;
/// a title, description or option left out of its list is removed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventUpdate {
    pub kind: String,
    pub id: String,
    pub titles: Vec<String>,
    pub descs: Vec<String>,
    pub picture: String,
    pub fire_only_once: String,
    pub is_triggered_only: String,
    pub hidden: String,
    pub major: String,
    pub timeout_days: String,
    pub trigger: String,
    pub mean_time_to_happen: String,
    pub immediate: String,
    pub options: Vec<EventOption>,
}

pub fn read(path: &str) -> AppResult<EventFile> {
    let file = text_file::read(Path::new(path))?;
    parse(path, file.content, file.has_bom, file.encoding)
}

/// Reads events out of text the window holds, which need not be what is on
/// disk: the code view parses its buffer as the user types.
pub fn parse(
    path: &str,
    content: String,
    has_bom: bool,
    encoding: text_file::Encoding,
) -> AppResult<EventFile> {
    let document = paradox::parse(&content).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let namespaces = document
        .find_all("add_namespace")
        .into_iter()
        .filter_map(|pair| pair.value.as_scalar().map(|scalar| scalar.value()))
        .collect();

    let events = event_pairs(&document)
        .into_iter()
        .filter_map(|pair| {
            pair.value
                .as_block()
                .map(|block| read_event(&content, &pair.key.to_lowercase(), block, pair.span.start))
        })
        .collect();

    Ok(EventFile {
        path: path.replace('\\', "/"),
        namespaces,
        events,
        has_bom,
        encoding,
        source: content,
    })
}

/// Every event block at the top level of the file, in file order.
fn event_pairs(document: &Document) -> Vec<&Pair> {
    document
        .pairs()
        .into_iter()
        .filter(|pair| is_event_kind(&pair.key) && pair.value.as_block().is_some())
        .collect()
}

fn is_event_kind(key: &str) -> bool {
    EVENT_KINDS
        .iter()
        .any(|kind| key.eq_ignore_ascii_case(kind))
}

fn read_event(source: &str, kind: &str, block: &Block, start: usize) -> Event {
    let texts = |key: &str| -> Vec<String> {
        block
            .find_all(key)
            .into_iter()
            .map(|pair| block_body(source, &pair.value))
            .collect()
    };
    let scalar = |key: &str| block.scalar(key).unwrap_or_default();
    let text = |key: &str| {
        block
            .find(key)
            .map(|pair| block_body(source, &pair.value))
            .unwrap_or_default()
    };

    let options = block
        .find_all("option")
        .into_iter()
        .filter_map(|pair| pair.value.as_block())
        .map(|option| EventOption {
            name: option.scalar("name").unwrap_or_default(),
            body: body_without(source, option, "name"),
        })
        .collect();

    Event {
        kind: kind.to_string(),
        id: scalar("id"),
        titles: texts("title"),
        descs: texts("desc"),
        picture: scalar("picture"),
        fire_only_once: scalar("fire_only_once"),
        is_triggered_only: scalar("is_triggered_only"),
        hidden: scalar("hidden"),
        major: scalar("major"),
        timeout_days: scalar("timeout_days"),
        trigger: text("trigger"),
        mean_time_to_happen: text("mean_time_to_happen"),
        immediate: text("immediate"),
        options,
        line: paradox::line_of(source, start),
    }
}

/// The body of `block` with every `key` assignment cut out, dedented: what
/// an option holds besides its name.
fn body_without(source: &str, block: &Block, key: &str) -> String {
    let mut cut: Vec<Span> = block
        .find_all(key)
        .into_iter()
        .map(|pair| owned_span(source, pair))
        .collect();
    cut.sort_by_key(|span| span.start);

    let mut kept = String::new();
    let mut at = block.inner.start;
    for span in cut {
        if span.start > at {
            kept.push_str(&source[at..span.start]);
        }
        at = at.max(span.end);
    }
    kept.push_str(&source[at..block.inner.end]);

    dedent(kept.trim_matches(|character| character == '\n' || character == '\r'))
        .join("\n")
        .trim()
        .to_string()
}

/// The span a removal of `pair` covers: its whole line when it sits alone
/// on one, otherwise just the assignment.
fn owned_span(source: &str, pair: &Pair) -> Span {
    remove_pair(source, pair)
        .into_iter()
        .next()
        .map(|edit| edit.span)
        .unwrap_or(pair.span)
}

/// Rewrites one event in place, matched by its current id.
pub fn update(path: &str, event_id: &str, update: &EventUpdate) -> AppResult<EventFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = update_source(path, &file.content, event_id, update)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`update`], applied to text instead of a file.
pub fn update_source(
    path: &str,
    source: &str,
    event_id: &str,
    update: &EventUpdate,
) -> AppResult<String> {
    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let pair = find_the_event(&document, event_id, path)?;
    let block = pair
        .value
        .as_block()
        .ok_or_else(|| AppError::message(format!("`{event_id}` is not an event block")))?;

    validate(update)?;

    let new_id = update.id.trim();
    if new_id != event_id && find_event(&document, new_id).is_some() {
        return Err(AppError::message(format!(
            "an event with id `{new_id}` already exists in this file"
        )));
    }

    let mut edits = Vec::new();
    if !pair.key.eq_ignore_ascii_case(update.kind.trim()) {
        // The kind is the block's key, which sits at the very start of the pair.
        let key = Span::new(pair.span.start, pair.span.start + pair.key.len());
        edits.push(TextEdit::new(key, update.kind.trim()));
    }

    let mut editor = BlockEditor::new(source, block);
    editor.set_scalar("id", new_id);
    set_texts(&mut editor, "title", &update.titles);
    set_texts(&mut editor, "desc", &update.descs);
    editor.set_scalar("picture", &update.picture);
    for (key, value) in FLAGS.iter().zip([
        &update.fire_only_once,
        &update.is_triggered_only,
        &update.hidden,
        &update.major,
    ]) {
        editor.set_scalar(key, value);
    }
    editor.set_scalar("timeout_days", &update.timeout_days);
    for (key, value) in BLOCKS.iter().zip([
        &update.trigger,
        &update.mean_time_to_happen,
        &update.immediate,
    ]) {
        editor.set_block(key, value);
    }
    set_options(&mut editor, &update.options);

    edits.extend(editor.finish());
    Ok(apply_edits(source, edits))
}

/// Appends an event at the end of the file.
pub fn add(path: &str, event: &EventUpdate) -> AppResult<EventFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = add_source(path, &file.content, event)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`add`], applied to text instead of a file.
pub fn add_source(path: &str, source: &str, event: &EventUpdate) -> AppResult<String> {
    validate(event)?;

    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let id = event.id.trim();
    if find_event(&document, id).is_some() {
        return Err(AppError::message(format!(
            "an event with id `{id}` already exists in this file"
        )));
    }

    let newline = detect_newline(source);
    let mut editor = BlockEditor::for_document(source, &document);
    // A blank line sets the new event apart from the one before it, the way
    // every event file is laid out.
    let lead = if source.trim().is_empty() {
        ""
    } else {
        newline
    };
    editor.add_line(format!(
        "{lead}{} = {}",
        event.kind.trim(),
        format_lines(&render_event(event), "", newline)
    ));

    Ok(apply_edits(source, editor.finish()))
}

pub fn delete(path: &str, event_id: &str) -> AppResult<EventFile> {
    let file = text_file::read(Path::new(path))?;
    let updated = delete_source(path, &file.content, event_id)?;
    text_file::write(Path::new(path), &updated, file.encoding, file.has_bom)?;

    read(path)
}

/// The edit behind [`delete`], applied to text instead of a file.
pub fn delete_source(path: &str, source: &str, event_id: &str) -> AppResult<String> {
    let document = paradox::parse(source).map_err(|source| AppError::Script {
        path: path.to_string(),
        source,
    })?;

    let pair = find_the_event(&document, event_id, path)?;

    // The blank line before the event goes with it, so two neighbours are
    // not left three lines apart.
    let mut edits = remove_pair(source, pair);
    if let Some(edit) = edits.first_mut() {
        let start = edit.span.start;
        if start > 0 {
            let previous = line_start(source, start - 1);
            if source[previous..line_end(source, previous)]
                .trim()
                .is_empty()
            {
                edit.span = Span::new(previous, edit.span.end);
            }
        }
    }

    Ok(apply_edits(source, edits))
}

fn find_event<'a>(document: &'a Document, id: &str) -> Option<&'a Pair> {
    events_with_id(document, id).into_iter().next()
}

fn events_with_id<'a>(document: &'a Document, id: &str) -> Vec<&'a Pair> {
    event_pairs(document)
        .into_iter()
        .filter(|pair| {
            pair.value
                .as_block()
                .and_then(|block| block.scalar("id"))
                .is_some_and(|current| current == id)
        })
        .collect()
}

/// The one event with `id`. Two events sharing an id is a bug in the mod
/// the game would trip over too; rather than change whichever comes first,
/// the form leaves both to the code view.
fn find_the_event<'a>(document: &'a Document, id: &str, path: &str) -> AppResult<&'a Pair> {
    let mut found = events_with_id(document, id);
    match found.len() {
        0 => Err(AppError::message(format!(
            "no event with id `{id}` exists in {path}"
        ))),
        1 => Ok(found.remove(0)),
        count => Err(AppError::message(format!(
            "{count} events share the id `{id}` in {path}; edit them in the code view"
        ))),
    }
}

fn validate(update: &EventUpdate) -> AppResult<()> {
    let id = update.id.trim();
    if id.is_empty() {
        return Err(AppError::message("an event needs an id"));
    }
    if needs_quotes(id) {
        return Err(AppError::message(format!(
            "`{id}` cannot be an event id: use a namespace, a dot and a number"
        )));
    }
    if !is_event_kind(update.kind.trim()) {
        return Err(AppError::message(format!(
            "`{}` is not an event kind; use one of {}",
            update.kind,
            EVENT_KINDS.join(", ")
        )));
    }

    Ok(())
}

/// Whether a title or description is a `{ text = ... }` block body rather
/// than a localisation key.
fn is_scripted(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.contains('{') || trimmed.contains('=') || trimmed.contains(char::is_whitespace)
}

/// Writes a repeated key holding either a localisation key or a block, by
/// position: entries already saying the same are left alone, extra ones
/// are dropped, new ones appended.
fn set_texts(editor: &mut BlockEditor<'_>, key: &str, values: &[String]) {
    let source = editor.source();
    let newline = detect_newline(source);
    let wanted: Vec<&str> = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect();
    let existing = editor.find_all(key);

    for (value, pair) in wanted.iter().zip(existing.iter()) {
        if block_body(source, &pair.value) == *value {
            continue;
        }
        let indent = indent_at(source, pair.span.start);
        editor.edit(TextEdit::new(
            pair.value.span(),
            render_text(value, &indent, newline),
        ));
    }

    for pair in existing.iter().skip(wanted.len()) {
        editor.remove(pair);
    }

    let indent = editor
        .block()
        .map(|block| child_indent(source, block))
        .unwrap_or_default();
    for value in wanted.iter().skip(existing.len()) {
        editor.add_line(format!("{key} = {}", render_text(value, &indent, newline)));
    }
}

fn render_text(value: &str, indent: &str, newline: &str) -> String {
    if is_scripted(value) {
        format_block(value, indent, newline)
    } else {
        value.to_string()
    }
}

/// Writes the options by position. An option whose effects did not change
/// has only its name set, so comments in it survive; one whose effects
/// changed is written out again from the text the form holds.
fn set_options(editor: &mut BlockEditor<'_>, options: &[EventOption]) {
    let source = editor.source();
    let newline = detect_newline(source);
    let existing = editor.find_all("option");

    for (option, pair) in options.iter().zip(existing.iter()) {
        let indent = indent_at(source, pair.span.start);
        match pair.value.as_block() {
            Some(block) if body_without(source, block, "name") == option.body.trim() => {
                let mut inner = BlockEditor::new(source, block);
                inner.set_scalar("name", &option.name);
                for edit in inner.finish() {
                    editor.edit(edit);
                }
            }
            _ => {
                editor.edit(TextEdit::new(
                    pair.value.span(),
                    format_lines(&render_option(option), &indent, newline),
                ));
            }
        }
    }

    for pair in existing.iter().skip(options.len()) {
        editor.remove(pair);
    }

    let indent = editor
        .block()
        .map(|block| child_indent(source, block))
        .unwrap_or_default();
    for option in options.iter().skip(existing.len()) {
        editor.add_line(format!(
            "option = {}",
            format_lines(&render_option(option), &indent, newline)
        ));
    }
}

/// A block with its braces on lines of their own however short the body,
/// as every event and option block is laid out.
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

// The renderers produce a block body with `\n` line breaks and no leading
// indentation; the formatter re-indents it wherever it lands.

fn render_scalar(key: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let raw = if needs_quotes(trimmed) {
        quote(trimmed)
    } else {
        trimmed.to_string()
    };
    Some(format!("{key} = {raw}"))
}

fn render_option(option: &EventOption) -> String {
    let mut lines = Vec::new();
    lines.extend(render_scalar("name", &option.name));
    let body = option.body.trim();
    if !body.is_empty() {
        lines.extend(dedent(body));
    }
    lines.join("\n")
}

fn render_event(event: &EventUpdate) -> String {
    let mut lines = Vec::new();
    lines.extend(render_scalar("id", &event.id));
    for title in &event.titles {
        if !title.trim().is_empty() {
            lines.push(format!("title = {}", render_text(title.trim(), "", "\n")));
        }
    }
    for desc in &event.descs {
        if !desc.trim().is_empty() {
            lines.push(format!("desc = {}", render_text(desc.trim(), "", "\n")));
        }
    }
    lines.extend(render_scalar("picture", &event.picture));
    for (key, value) in FLAGS.iter().zip([
        &event.fire_only_once,
        &event.is_triggered_only,
        &event.hidden,
        &event.major,
    ]) {
        lines.extend(render_scalar(key, value));
    }
    lines.extend(render_scalar("timeout_days", &event.timeout_days));
    for (key, value) in
        BLOCKS
            .iter()
            .zip([&event.trigger, &event.mean_time_to_happen, &event.immediate])
    {
        if !value.trim().is_empty() {
            lines.push(String::new());
            lines.push(format!("{key} = {}", format_lines(value, "", "\n")));
        }
    }
    for option in &event.options {
        lines.push(String::new());
        lines.push(format!(
            "option = {}",
            format_lines(&render_option(option), "", "\n")
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two events the way a mod writes them: a triggered one with a
    /// conditional description and two options, and a news event.
    const FILE: &str = "add_namespace = tst\n\n# The first event\ncountry_event = {\n\tid = tst.1\n\ttitle = tst.1.t\n\tdesc = tst.1.d # plain\n\tdesc = {\n\t\ttext = tst.1.d.b\n\t\ttrigger = { has_government = fascism }\n\t}\n\tpicture = GFX_report_event_generic_sign_treaty\n\tis_triggered_only = yes\n\tfire_only_once = yes\n\n\ttrigger = {\n\t\ttag = TST\n\t}\n\n\timmediate = {\n\t\thidden_effect = { set_country_flag = tst_1_fired }\n\t}\n\n\toption = {\n\t\tname = tst.1.a\n\t\tai_chance = { factor = 80 }\n\t\tadd_political_power = 50 # a bribe\n\t}\n\toption = {\n\t\tname = tst.1.b # decline\n\t\ttrigger = { has_war = no }\n\t\tadd_stability = -0.05\n\t}\n}\n\nnews_event = {\n\tid = tst.2\n\ttitle = tst.2.t\n\tdesc = tst.2.d\n\tpicture = GFX_news_event_generic_army\n\tmajor = yes\n\tis_triggered_only = yes\n\n\toption = {\n\t\tname = tst.2.a\n\t}\n}\n";

    fn parsed(source: &str) -> EventFile {
        parse(
            "(test)",
            source.to_string(),
            false,
            text_file::Encoding::Utf8,
        )
        .expect("parses")
    }

    fn update_from(event: &Event) -> EventUpdate {
        EventUpdate {
            kind: event.kind.clone(),
            id: event.id.clone(),
            titles: event.titles.clone(),
            descs: event.descs.clone(),
            picture: event.picture.clone(),
            fire_only_once: event.fire_only_once.clone(),
            is_triggered_only: event.is_triggered_only.clone(),
            hidden: event.hidden.clone(),
            major: event.major.clone(),
            timeout_days: event.timeout_days.clone(),
            trigger: event.trigger.clone(),
            mean_time_to_happen: event.mean_time_to_happen.clone(),
            immediate: event.immediate.clone(),
            options: event.options.clone(),
        }
    }

    #[test]
    fn reads_the_namespace_the_events_and_their_options() {
        let file = parsed(FILE);
        assert_eq!(file.namespaces, vec!["tst"]);
        assert_eq!(file.events.len(), 2);

        let first = &file.events[0];
        assert_eq!(first.kind, "country_event");
        assert_eq!(first.id, "tst.1");
        assert_eq!(first.line, 4);
        assert_eq!(first.titles, vec!["tst.1.t"]);
        assert_eq!(first.descs.len(), 2);
        assert_eq!(first.descs[0], "tst.1.d");
        assert_eq!(
            first.descs[1],
            "text = tst.1.d.b\ntrigger = { has_government = fascism }"
        );
        assert_eq!(first.picture, "GFX_report_event_generic_sign_treaty");
        assert_eq!(first.is_triggered_only, "yes");
        assert_eq!(first.fire_only_once, "yes");
        assert_eq!(first.major, "");
        assert_eq!(first.trigger, "tag = TST");
        assert_eq!(
            first.immediate,
            "hidden_effect = { set_country_flag = tst_1_fired }"
        );
        assert_eq!(first.options.len(), 2);
        assert_eq!(first.options[0].name, "tst.1.a");
        assert_eq!(
            first.options[0].body,
            "ai_chance = { factor = 80 }\nadd_political_power = 50 # a bribe"
        );
        assert_eq!(first.options[1].name, "tst.1.b");
        assert_eq!(
            first.options[1].body,
            "trigger = { has_war = no }\nadd_stability = -0.05"
        );

        let second = &file.events[1];
        assert_eq!(second.kind, "news_event");
        assert_eq!(second.major, "yes");
        assert_eq!(second.options[0].body, "");
    }

    #[test]
    fn a_no_op_save_changes_nothing() {
        let file = parsed(FILE);
        for event in &file.events {
            let text =
                update_source("(test)", FILE, &event.id, &update_from(event)).expect("updates");
            assert_eq!(text, FILE, "{} was rewritten", event.id);
        }
    }

    #[test]
    fn edits_fields_in_place_and_keeps_the_rest() {
        let file = parsed(FILE);
        let mut change = update_from(&file.events[0]);
        change.picture = "GFX_report_event_generic_factory".into();
        change.fire_only_once = String::new();
        change.major = "yes".into();
        change.options[0].name = "tst.1.a.renamed".into();
        change.descs[0] = "tst.1.d.new".into();

        let text = update_source("(test)", FILE, "tst.1", &change).expect("updates");

        assert!(
            text.contains("picture = GFX_report_event_generic_factory"),
            "{text}"
        );
        assert!(!text.contains("fire_only_once"), "{text}");
        assert!(text.contains("\tmajor = yes\n"), "{text}");
        assert!(text.contains("name = tst.1.a.renamed\n"), "{text}");
        // The option's effects were not touched, so its comment is still there.
        assert!(
            text.contains("add_political_power = 50 # a bribe"),
            "{text}"
        );
        assert!(text.contains("desc = tst.1.d.new # plain"), "{text}");
        assert!(
            text.contains("trigger = { has_government = fascism }"),
            "{text}"
        );
        assert!(text.contains("# The first event"), "{text}");
        assert!(text.contains("tst.2.t"), "second event touched:\n{text}");
    }

    #[test]
    fn an_option_whose_effects_change_is_written_out_again() {
        let file = parsed(FILE);
        let mut change = update_from(&file.events[0]);
        change.options[1].body =
            "trigger = { has_war = no }\nadd_stability = -0.10\nadd_war_support = 0.05".into();

        let text = update_source("(test)", FILE, "tst.1", &change).expect("updates");
        let event = &parsed(&text).events[0];
        assert_eq!(event.options.len(), 2);
        assert_eq!(event.options[1].name, "tst.1.b");
        assert_eq!(
            event.options[1].body,
            "trigger = { has_war = no }\nadd_stability = -0.10\nadd_war_support = 0.05"
        );
        assert!(
            text.contains("\toption = {\n\t\tname = tst.1.b\n\t\ttrigger = { has_war = no }\n\t\tadd_stability = -0.10\n\t\tadd_war_support = 0.05\n\t}\n"),
            "{text}"
        );
        // The first option kept its comment.
        assert!(text.contains("# a bribe"), "{text}");
    }

    #[test]
    fn options_and_descriptions_are_added_and_dropped_by_position() {
        let file = parsed(FILE);
        let mut change = update_from(&file.events[1]);
        change.options.push(EventOption {
            name: "tst.2.b".into(),
            body: "add_political_power = -10".into(),
        });
        change
            .descs
            .push("text = tst.2.d.b\ntrigger = { has_war = yes }".into());
        change.titles = vec!["tst.2.t.new".into()];

        let text = update_source("(test)", FILE, "tst.2", &change).expect("updates");
        let event = &parsed(&text).events[1];
        assert_eq!(event.options.len(), 2);
        assert_eq!(event.options[1].body, "add_political_power = -10");
        assert_eq!(event.descs.len(), 2);
        assert_eq!(
            event.descs[1],
            "text = tst.2.d.b\ntrigger = { has_war = yes }"
        );
        assert_eq!(event.titles, vec!["tst.2.t.new"]);
        assert!(
            text.contains(
                "\tdesc = {\n\t\ttext = tst.2.d.b\n\t\ttrigger = { has_war = yes }\n\t}\n"
            ),
            "{text}"
        );

        let mut change = update_from(event);
        change.options.remove(0);
        change.descs.remove(1);
        let text = update_source("(test)", &text, "tst.2", &change).expect("updates");
        let event = &parsed(&text).events[1];
        assert_eq!(event.options.len(), 1);
        assert_eq!(event.options[0].name, "tst.2.b");
        assert_eq!(event.descs, vec!["tst.2.d"]);
    }

    #[test]
    fn changes_the_kind_and_the_id() {
        let file = parsed(FILE);
        let mut change = update_from(&file.events[1]);
        change.kind = "country_event".into();
        change.id = "tst.3".into();

        let text = update_source("(test)", FILE, "tst.2", &change).expect("updates");
        let event = &parsed(&text).events[1];
        assert_eq!(event.kind, "country_event");
        assert_eq!(event.id, "tst.3");
        assert!(!text.contains("news_event = {"), "{text}");

        let mut clash = update_from(event);
        clash.id = "tst.1".into();
        assert!(update_source("(test)", &text, "tst.3", &clash).is_err());
    }

    #[test]
    fn adds_and_deletes_an_event() {
        let added = add_source(
            "(test)",
            FILE,
            &EventUpdate {
                kind: "country_event".into(),
                id: "tst.3".into(),
                titles: vec!["tst.3.t".into()],
                descs: vec!["tst.3.d".into()],
                picture: "GFX_report_event_generic_sign_treaty".into(),
                is_triggered_only: "yes".into(),
                trigger: "tag = TST".into(),
                options: vec![EventOption {
                    name: "tst.3.a".into(),
                    body: "add_political_power = 10".into(),
                }],
                ..EventUpdate::default()
            },
        )
        .expect("adds");

        let reread = parsed(&added);
        assert_eq!(reread.events.len(), 3);
        let event = &reread.events[2];
        assert_eq!(event.id, "tst.3");
        assert_eq!(event.trigger, "tag = TST");
        assert_eq!(event.options[0].body, "add_political_power = 10");
        assert!(added.starts_with(FILE), "the rest was disturbed:\n{added}");
        assert!(
            added.ends_with("\n\ncountry_event = {\n\tid = tst.3\n\ttitle = tst.3.t\n\tdesc = tst.3.d\n\tpicture = GFX_report_event_generic_sign_treaty\n\tis_triggered_only = yes\n\n\ttrigger = {\n\t\ttag = TST\n\t}\n\n\toption = {\n\t\tname = tst.3.a\n\t\tadd_political_power = 10\n\t}\n}\n"),
            "{added}"
        );

        let deleted = delete_source("(test)", &added, "tst.2").expect("deletes");
        let reread = parsed(&deleted);
        assert_eq!(reread.events.len(), 2);
        assert!(!deleted.contains("tst.2"), "{deleted}");
        // Its blank line went with it: the neighbours are one blank line apart.
        assert!(
            deleted.contains("\t}\n}\n\ncountry_event = {\n\tid = tst.3\n"),
            "{deleted}"
        );
    }

    #[test]
    fn a_namespace_only_file_gets_its_first_event() {
        let text = add_source(
            "(test)",
            "add_namespace = tst\n",
            &EventUpdate {
                kind: "news_event".into(),
                id: "tst.1".into(),
                ..EventUpdate::default()
            },
        )
        .expect("adds");

        assert_eq!(
            text,
            "add_namespace = tst\n\nnews_event = {\n\tid = tst.1\n}\n"
        );
    }

    #[test]
    fn refuses_bad_ids_and_kinds() {
        let file = parsed(FILE);
        let mut change = update_from(&file.events[0]);
        change.id = "two words".into();
        assert!(update_source("(test)", FILE, "tst.1", &change).is_err());

        let mut change = update_from(&file.events[0]);
        change.kind = "province_event".into();
        assert!(update_source("(test)", FILE, "tst.1", &change).is_err());
    }

    #[test]
    fn two_events_sharing_an_id_are_left_to_the_code_view() {
        let source = "country_event = {
	id = tst.1
	title = tst.1.t
}
country_event = {
	id = tst.1
	title = tst.1
}
";
        let file = parsed(source);
        assert_eq!(file.events.len(), 2);

        let error = update_source("(test)", source, "tst.1", &update_from(&file.events[1]))
            .expect_err("refuses");
        assert!(error.to_string().contains("2 events share"), "{error}");
        assert!(delete_source("(test)", source, "tst.1").is_err());
    }

    #[test]
    fn a_capitalised_key_is_edited_where_it_is() {
        let source = "country_event = {\n\tid = tst.1\n\tPicture = GFX_a\n\tHidden = yes\n\toption = { name = tst.1.a }\n}\n";
        let file = parsed(source);
        assert_eq!(file.events[0].picture, "GFX_a");
        assert_eq!(file.events[0].hidden, "yes");

        let mut change = update_from(&file.events[0]);
        change.picture = "GFX_b".into();
        let text = update_source("(test)", source, "tst.1", &change).expect("updates");
        assert!(text.contains("\tPicture = GFX_b\n"), "{text}");
    }
}
