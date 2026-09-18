//! Persisted application settings, stored next to the other app data in the
//! platform local data directory.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::console::{self, Source};
use crate::error::{AppError, AppResult};

const SETTINGS_VERSION: u32 = 1;
const MAX_RECENT_PROJECTS: usize = 12;
/// Loopback port the MCP server listens on unless the user picks another.
pub const DEFAULT_MCP_PORT: u16 = 4747;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    pub mod_file_path: String,
    pub folder_path: String,
    pub display_name: String,
    #[serde(default)]
    pub image_path: String,
    /// Milliseconds since the Unix epoch.
    pub last_opened: u64,
}

/// How the app is exposed to MCP clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpSettings {
    /// Whether the server starts with the app.
    pub enabled: bool,
    /// Port on the loopback interface.
    pub port: u16,
    /// Bearer token every request must carry. Generated once and kept, so a
    /// client configured today keeps working after the next launch.
    pub token: String,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            port: DEFAULT_MCP_PORT,
            token: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    /// `system`, `light` or `dark`.
    pub theme: String,
    /// Hearts of Iron IV installation directory, used by the game asset browser.
    pub game_root_path: String,
    pub recent_projects: Vec<RecentProject>,
    /// Absent from files written before the server existed. Without the
    /// default, `load` would treat such a file as damaged and discard the game
    /// folder and recent projects along with it.
    #[serde(default)]
    pub mcp: McpSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            theme: "system".to_string(),
            game_root_path: String::new(),
            recent_projects: Vec::new(),
            mcp: McpSettings::default(),
        }
    }
}

/// A fresh bearer token: 128 random bits as hex, which is what a client is
/// asked to paste, so nothing in it needs quoting.
pub fn new_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn settings_path(app: &AppHandle) -> AppResult<PathBuf> {
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|error| AppError::message(format!("no local data directory: {error}")))?;

    Ok(directory.join("settings.json"))
}

/// What a load found on disk.
enum Found {
    /// No file yet: a first launch.
    Fresh,
    Parsed(Settings),
    /// A file that exists but does not parse.
    Damaged(serde_json::Error),
}

fn interpret(json: Option<&str>) -> Found {
    match json {
        None => Found::Fresh,
        Some(json) => match serde_json::from_str(json) {
            Ok(settings) => Found::Parsed(settings),
            Err(error) => Found::Damaged(error),
        },
    }
}

/// A token for a run whose settings file could not be read. Minted once per
/// process so every command agrees on it, and never written anywhere.
fn session_token() -> String {
    static TOKEN: OnceLock<String> = OnceLock::new();
    TOKEN.get_or_init(new_token).clone()
}

pub fn load(app: &AppHandle) -> AppResult<Settings> {
    let path = settings_path(app)?;
    let json = if path.exists() {
        Some(fs::read_to_string(&path).map_err(|source| AppError::io(&path, source))?)
    } else {
        None
    };

    match interpret(json.as_deref()) {
        Found::Fresh => {
            let mut settings = Settings::default();
            settings.mcp.token = new_token();
            save(app, &settings)?;
            Ok(settings)
        }
        Found::Parsed(mut settings) => {
            // A file from before the server existed has no token; mint one and
            // keep it, so a client configured today works after the next launch.
            if settings.mcp.token.is_empty() {
                settings.mcp.token = new_token();
                save(app, &settings)?;
            }
            Ok(settings)
        }
        Found::Damaged(error) => {
            // A file damaged by a crash or a hand edit should not brick the
            // app, but it must not be overwritten either: the game folder and
            // the recent projects are still in there, one fix away. Defaults
            // serve this run, and the next deliberate save replaces the file.
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                console::error(
                    app,
                    Source::App,
                    format!(
                        "{} could not be read and was left untouched; defaults apply until it is fixed or a setting is saved",
                        path.display()
                    ),
                    Some(error.to_string()),
                );
            }

            let mut settings = Settings::default();
            settings.mcp.token = session_token();
            Ok(settings)
        }
    }
}

pub fn save(app: &AppHandle, settings: &Settings) -> AppResult<()> {
    let path = settings_path(app)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::io(parent, source))?;
    }

    let json = serde_json::to_string_pretty(settings)?;
    fs::write(&path, json).map_err(|source| AppError::io(&path, source))
}

/// Moves a project to the front of the recent list, de-duplicating by folder.
pub fn remember_project(settings: &mut Settings, project: RecentProject) {
    settings
        .recent_projects
        .retain(|existing| !paths_match(&existing.folder_path, &project.folder_path));

    settings.recent_projects.insert(0, project);
    settings.recent_projects.truncate(MAX_RECENT_PROJECTS);
}

pub fn forget_project(settings: &mut Settings, folder_path: &str) {
    settings
        .recent_projects
        .retain(|existing| !paths_match(&existing.folder_path, folder_path));
}

fn paths_match(left: &str, right: &str) -> bool {
    normalise(left) == normalise(right)
}

fn normalise(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(folder: &str) -> RecentProject {
        RecentProject {
            mod_file_path: format!("{folder}.mod"),
            folder_path: folder.to_string(),
            display_name: folder.to_string(),
            image_path: String::new(),
            last_opened: 0,
        }
    }

    #[test]
    fn recent_projects_are_deduplicated_case_insensitively() {
        let mut settings = Settings::default();
        remember_project(&mut settings, project("C:/mods/Alpha"));
        remember_project(&mut settings, project("C:/mods/Beta"));
        remember_project(&mut settings, project("c:\\mods\\alpha\\"));

        assert_eq!(settings.recent_projects.len(), 2);
        assert_eq!(settings.recent_projects[0].folder_path, "c:\\mods\\alpha\\");
    }

    #[test]
    fn recent_projects_are_capped() {
        let mut settings = Settings::default();
        for index in 0..(MAX_RECENT_PROJECTS + 5) {
            remember_project(&mut settings, project(&format!("C:/mods/{index}")));
        }

        assert_eq!(settings.recent_projects.len(), MAX_RECENT_PROJECTS);
    }

    #[test]
    fn forgetting_removes_the_entry() {
        let mut settings = Settings::default();
        remember_project(&mut settings, project("C:/mods/Alpha"));
        forget_project(&mut settings, "C:/MODS/ALPHA");

        assert!(settings.recent_projects.is_empty());
    }

    #[test]
    fn a_file_from_before_the_server_keeps_everything_it_had() {
        // Written by a build with no `mcp` field. Losing the game folder over
        // a missing key is exactly what `unwrap_or_default` would do without
        // the field-level default.
        let json = r#"{
            "version": 1,
            "theme": "dark",
            "gameRootPath": "D:/Games/Hearts of Iron IV",
            "recentProjects": [{
                "modFilePath": "C:/mods/alpha.mod",
                "folderPath": "C:/mods/alpha",
                "displayName": "Alpha",
                "lastOpened": 5
            }]
        }"#;

        let settings: Settings = serde_json::from_str(json).expect("parses");

        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.game_root_path, "D:/Games/Hearts of Iron IV");
        assert_eq!(settings.recent_projects.len(), 1);
        assert_eq!(settings.mcp, McpSettings::default());
        assert!(
            settings.mcp.token.is_empty(),
            "the token is minted on load, not on parse"
        );
    }

    #[test]
    fn a_partial_mcp_block_fills_in_the_rest() {
        let json = r#"{ "version": 1, "theme": "system", "gameRootPath": "", "recentProjects": [], "mcp": { "port": 5000 } }"#;

        let settings: Settings = serde_json::from_str(json).expect("parses");

        assert_eq!(settings.mcp.port, 5000);
        assert!(settings.mcp.enabled);
    }

    #[test]
    fn a_damaged_file_is_recognised_rather_than_replaced_by_defaults() {
        // `unwrap_or_default` would have handed back defaults here, and the
        // token minting would then have saved them over the file.
        assert!(matches!(
            interpret(Some("{ \"version\": 1, ")),
            Found::Damaged(_)
        ));
        assert!(matches!(interpret(None), Found::Fresh));
        assert!(matches!(
            interpret(Some(r#"{ "version": 1, "theme": "dark", "gameRootPath": "", "recentProjects": [] }"#)),
            Found::Parsed(settings) if settings.theme == "dark"
        ));
    }

    #[test]
    fn a_session_token_is_stable_for_the_run() {
        assert_eq!(session_token(), session_token());
        assert_eq!(session_token().len(), 32);
    }

    #[test]
    fn tokens_are_long_random_and_plain() {
        let first = new_token();
        let second = new_token();

        assert_eq!(first.len(), 32);
        assert_ne!(first, second);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
