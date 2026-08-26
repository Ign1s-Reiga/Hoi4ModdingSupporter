//! Persisted application settings, stored next to the other app data in the
//! platform local data directory.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

const SETTINGS_VERSION: u32 = 1;
const MAX_RECENT_PROJECTS: usize = 12;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    /// `system`, `light` or `dark`.
    pub theme: String,
    /// Hearts of Iron IV installation directory, used by the game asset browser.
    pub game_root_path: String,
    pub recent_projects: Vec<RecentProject>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            theme: "system".to_string(),
            game_root_path: String::new(),
            recent_projects: Vec::new(),
        }
    }
}

fn settings_path(app: &AppHandle) -> AppResult<PathBuf> {
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|error| AppError::message(format!("no local data directory: {error}")))?;

    Ok(directory.join("settings.json"))
}

pub fn load(app: &AppHandle) -> AppResult<Settings> {
    let path = settings_path(app)?;

    if !path.exists() {
        let settings = Settings::default();
        save(app, &settings)?;
        return Ok(settings);
    }

    let json = fs::read_to_string(&path).map_err(|source| AppError::io(&path, source))?;

    // A settings file damaged by a crash or hand editing should not brick the
    // app: fall back to defaults rather than refusing to start.
    Ok(serde_json::from_str(&json).unwrap_or_default())
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
    path.replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
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
}
