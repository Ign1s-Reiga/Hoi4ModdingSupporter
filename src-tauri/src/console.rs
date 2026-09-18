//! The activity log behind the console panel.
//!
//! Anything the backend does on the user's behalf that they could not
//! otherwise see — a file written, a cache rebuilt, a tool call made by an MCP
//! client — is recorded here and pushed to the window as it happens. That is
//! what makes an assistant driving the app through MCP auditable: every edit
//! it makes shows up in the same place the user's own saves do.
//!
//! The log is bounded so a long session cannot grow it without limit; the
//! panel shows the tail and the oldest entries fall off the front.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// Entries kept in memory. Two thousand is a long afternoon of edits.
const CAPACITY: usize = 2_000;

/// Event carrying each new entry to the window.
pub const ENTRY_EVENT: &str = "console://entry";
/// Event sent when the log is emptied, so the panel follows.
pub const CLEARED_EVENT: &str = "console://cleared";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Info,
    Warn,
    Error,
}

/// Who did the thing being logged, so the panel can be filtered to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// The app itself: caches, startup, settings.
    App,
    /// A file written on the user's behalf, from any editor.
    Files,
    /// The MCP server: clients, tool calls.
    Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: u64,
    /// Milliseconds since the Unix epoch.
    pub at_ms: u64,
    pub level: Level,
    pub source: Source,
    pub message: String,
    /// Longer text shown when the entry is expanded: tool arguments, a
    /// result, the full error.
    pub detail: Option<String>,
}

#[derive(Default)]
pub struct Console {
    entries: Mutex<VecDeque<Entry>>,
    next_id: AtomicU64,
}

impl Console {
    fn push(&self, level: Level, source: Source, message: String, detail: Option<String>) -> Entry {
        let entry = Entry {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            at_ms: now_ms(),
            level,
            source,
            message,
            detail,
        };

        // A poisoned lock means a panic elsewhere; losing one log line to it
        // is better than propagating the panic into every caller.
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= CAPACITY {
                entries.pop_front();
            }
            entries.push_back(entry.clone());
        }

        entry
    }

    pub fn entries(&self) -> Vec<Entry> {
        self.entries
            .lock()
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

/// Records an entry and pushes it to the window.
///
/// Callers hold an `AppHandle` rather than the console itself so that the
/// modules doing the work — focus edits, the MCP server — stay unaware of how
/// the log is stored or displayed.
pub fn log(
    app: &AppHandle,
    level: Level,
    source: Source,
    message: impl Into<String>,
    detail: Option<String>,
) {
    let entry = app
        .state::<Console>()
        .push(level, source, message.into(), detail);

    // The window may be closing; an entry that cannot be delivered is still
    // in the log for the next `console_entries` call.
    let _ = app.emit(ENTRY_EVENT, &entry);
}

pub fn info(app: &AppHandle, source: Source, message: impl Into<String>, detail: Option<String>) {
    log(app, Level::Info, source, message, detail);
}

pub fn warn(app: &AppHandle, source: Source, message: impl Into<String>, detail: Option<String>) {
    log(app, Level::Warn, source, message, detail);
}

pub fn error(app: &AppHandle, source: Source, message: impl Into<String>, detail: Option<String>) {
    log(app, Level::Error, source, message, detail);
}

pub fn clear(app: &AppHandle) {
    app.state::<Console>().clear();
    let _ = app.emit(CLEARED_EVENT, ());
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_entries_in_order_with_rising_ids() {
        let console = Console::default();
        console.push(Level::Info, Source::App, "first".into(), None);
        console.push(
            Level::Warn,
            Source::Files,
            "second".into(),
            Some("why".into()),
        );

        let entries = console.entries();

        assert_eq!(entries.len(), 2);
        assert!(entries[0].id < entries[1].id);
        assert_eq!(entries[1].message, "second");
        assert_eq!(entries[1].detail.as_deref(), Some("why"));
    }

    #[test]
    fn drops_the_oldest_entry_past_capacity() {
        let console = Console::default();
        for index in 0..=CAPACITY {
            console.push(Level::Info, Source::App, index.to_string(), None);
        }

        let entries = console.entries();

        assert_eq!(entries.len(), CAPACITY);
        assert_eq!(entries[0].message, "1");
        assert_eq!(entries[CAPACITY - 1].message, CAPACITY.to_string());
    }

    #[test]
    fn clearing_empties_the_log_but_not_the_id_sequence() {
        let console = Console::default();
        console.push(Level::Info, Source::App, "before".into(), None);
        console.clear();
        let after = console.push(Level::Info, Source::App, "after".into(), None);

        assert_eq!(console.entries().len(), 1);
        // Ids keep rising, so the panel can never confuse a new entry with a
        // cleared one it still has on screen.
        assert_eq!(after.id, 1);
    }
}
