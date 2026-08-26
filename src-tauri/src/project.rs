//! Mod descriptor (`.mod`) reading and project file discovery.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::error::{AppError, AppResult};
use crate::paradox::{self, Items};
use crate::text_file;

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "yml", "yaml", "gfx", "gui", "asset", "csv", "json", "lua", "mod", "info", "settings",
    "sfx", "log",
];

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif", "tga", "dds"];

const SKIPPED_DIRECTORIES: &[&str] = &[".git", ".vs", ".idea", "node_modules", "target", "bin", "obj"];

/// Top level folders a mod can override, in the order the workspace shows them.
pub const ASSET_AREAS: &[(&str, &str)] = &[
    ("common", "Common"),
    ("events", "Events"),
    ("history", "History"),
    ("gfx", "GFX"),
    ("interface", "Interface"),
    ("localisation", "Localisation"),
    ("map", "Map"),
    ("music", "Music"),
    ("portraits", "Portraits"),
    ("sound", "Sound"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Text,
    Image,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub full_path: String,
    /// Path relative to the scan root, always with forward slashes.
    pub relative_path: String,
    pub name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_ms: u64,
    pub kind: FileKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub root: String,
    pub files: Vec<ProjectFile>,
    /// True when the scan hit its file limit and stopped early.
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProject {
    pub mod_file_path: String,
    pub folder_path: String,
    pub name: String,
    pub version: String,
    pub supported_version: String,
    pub tags: Vec<String>,
    /// Absolute path of the thumbnail, empty when the mod has none.
    pub image_path: String,
    pub replace_paths: Vec<String>,
}

/// Reads a `.mod` descriptor and resolves the folder it points at.
pub fn read_descriptor(mod_file_path: &str) -> AppResult<ModProject> {
    let path = Path::new(mod_file_path);

    if !path.is_file() {
        return Err(AppError::message(format!(
            "{mod_file_path} does not exist"
        )));
    }

    let file = text_file::read(path)?;
    let document = paradox::parse(&file.content).map_err(|source| AppError::Script {
        path: mod_file_path.to_string(),
        source,
    })?;

    let descriptor_directory = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let declared_path = document.scalar("path").unwrap_or_default();
    let folder = resolve_project_folder(&descriptor_directory, &declared_path);

    if !folder.is_dir() {
        return Err(AppError::message(format!(
            "the mod folder {} does not exist",
            folder.display()
        )));
    }

    let name = document.scalar("name").unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default()
    });

    let tags = document
        .block("tags")
        .map(|block| {
            block
                .items
                .iter()
                .filter_map(|item| match item {
                    paradox::Item::Value(value) => value.as_scalar().map(|scalar| scalar.value()),
                    paradox::Item::Pair(_) => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let replace_paths = document
        .find_all("replace_path")
        .into_iter()
        .filter_map(|pair| pair.value.as_scalar().map(|scalar| scalar.value()))
        .collect();

    let image_path = document
        .scalar("picture")
        .map(|picture| resolve_image(&folder, &descriptor_directory, &picture))
        .unwrap_or_default();

    Ok(ModProject {
        mod_file_path: normalise_path(path),
        folder_path: normalise_path(&folder),
        name,
        version: document.scalar("version").unwrap_or_default(),
        supported_version: document.scalar("supported_version").unwrap_or_default(),
        tags,
        image_path,
        replace_paths,
    })
}

/// `path` in a descriptor is usually written relative to the Hearts of Iron IV
/// user directory (`mod/my_mod`), while the descriptor itself lives in
/// `.../Hearts of Iron IV/mod/`. When the first segment repeats the descriptor
/// folder name, resolve against its parent instead.
fn resolve_project_folder(descriptor_directory: &Path, declared_path: &str) -> PathBuf {
    if declared_path.trim().is_empty() {
        return descriptor_directory.to_path_buf();
    }

    let candidate = Path::new(declared_path);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }

    let normalised = declared_path.replace('\\', "/");
    let first_segment = normalised.split('/').next().unwrap_or_default();
    let descriptor_name = descriptor_directory
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    let base = if first_segment.eq_ignore_ascii_case(&descriptor_name) {
        descriptor_directory
            .parent()
            .unwrap_or(descriptor_directory)
    } else {
        descriptor_directory
    };

    base.join(normalised)
}

fn resolve_image(folder: &Path, descriptor_directory: &Path, picture: &str) -> String {
    if picture.trim().is_empty() {
        return String::new();
    }

    let direct = Path::new(picture);
    if direct.is_absolute() {
        return if direct.is_file() {
            normalise_path(direct)
        } else {
            String::new()
        };
    }

    for base in [folder, descriptor_directory] {
        let candidate = base.join(picture);
        if candidate.is_file() {
            return normalise_path(&candidate);
        }
    }

    String::new()
}

/// Walks `root` and returns every file below it, skipping tool directories.
pub fn scan(root: &str, max_files: usize) -> AppResult<ScanResult> {
    let root_path = Path::new(root);

    if !root_path.is_dir() {
        return Err(AppError::message(format!("{root} is not a folder")));
    }

    let mut files = Vec::new();
    let mut truncated = false;

    let walker = WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_skipped_directory(entry.path(), entry.file_type().is_dir()));

    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        if files.len() >= max_files {
            truncated = true;
            break;
        }

        if let Some(file) = describe(root_path, entry.path()) {
            files.push(file);
        }
    }

    files.sort_by(|left, right| {
        left.relative_path
            .to_lowercase()
            .cmp(&right.relative_path.to_lowercase())
    });

    Ok(ScanResult {
        root: normalise_path(root_path),
        files,
        truncated,
    })
}

fn is_skipped_directory(path: &Path, is_directory: bool) -> bool {
    if !is_directory {
        return false;
    }

    path.file_name()
        .map(|name| {
            let name = name.to_string_lossy().to_lowercase();
            SKIPPED_DIRECTORIES.contains(&name.as_str())
        })
        .unwrap_or(false)
}

fn describe(root: &Path, path: &Path) -> Option<ProjectFile> {
    let metadata = path.metadata().ok()?;
    let relative = path.strip_prefix(root).ok()?;

    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    Some(ProjectFile {
        full_path: normalise_path(path),
        relative_path: relative.to_string_lossy().replace('\\', "/"),
        name: path.file_name()?.to_string_lossy().to_string(),
        extension: extension.clone(),
        size_bytes: metadata.len(),
        modified_ms: modified_ms(&metadata),
        kind: classify(&extension),
    })
}

fn modified_ms(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

pub fn classify(extension: &str) -> FileKind {
    if TEXT_EXTENSIONS.contains(&extension) {
        FileKind::Text
    } else if IMAGE_EXTENSIONS.contains(&extension) {
        FileKind::Image
    } else {
        FileKind::Binary
    }
}

/// Windows paths are kept in their native form, only the separators are
/// unified so the frontend can compare and split them predictably.
pub fn normalise_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_descriptor_path_relative_to_the_user_directory() {
        let descriptor_directory = Path::new("C:/Users/x/Documents/Paradox Interactive/Hearts of Iron IV/mod");
        let folder = resolve_project_folder(descriptor_directory, "mod/my_mod");

        assert_eq!(
            normalise_path(&folder),
            "C:/Users/x/Documents/Paradox Interactive/Hearts of Iron IV/mod/my_mod"
        );
    }

    #[test]
    fn resolves_a_descriptor_path_relative_to_itself() {
        let descriptor_directory = Path::new("D:/projects");
        let folder = resolve_project_folder(descriptor_directory, "my_mod");

        assert_eq!(normalise_path(&folder), "D:/projects/my_mod");
    }

    #[test]
    fn falls_back_to_the_descriptor_folder_when_no_path_is_declared() {
        let descriptor_directory = Path::new("D:/projects/my_mod");
        let folder = resolve_project_folder(descriptor_directory, "");

        assert_eq!(normalise_path(&folder), "D:/projects/my_mod");
    }

    #[test]
    fn classifies_extensions() {
        assert_eq!(classify("txt"), FileKind::Text);
        assert_eq!(classify("yml"), FileKind::Text);
        assert_eq!(classify("dds"), FileKind::Image);
        assert_eq!(classify("bin"), FileKind::Binary);
    }
}
