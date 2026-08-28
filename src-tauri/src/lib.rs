mod assets;
mod country_history;
mod error;
mod focus;
mod localisation;
mod map;
mod paradox;
mod project;
mod settings;
mod state;
mod text_file;

use std::sync::Mutex;

use tauri::{AppHandle, State};

use error::{AppError, AppResult};

/// Upper bound for a full mod scan. Large total conversions land well under
/// this; anything past it is reported as truncated instead of freezing the UI.
const MAX_PROJECT_FILES: usize = 20_000;
/// Game folders such as `gfx/` hold far more files than a mod ever will.
const MAX_BROWSE_FILES: usize = 8_000;

#[tauri::command]
fn load_settings(app: AppHandle) -> AppResult<settings::Settings> {
    settings::load(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: settings::Settings) -> AppResult<settings::Settings> {
    settings::save(&app, &settings)?;
    Ok(settings)
}

/// Reads a `.mod` descriptor and moves the project to the top of the recents.
#[tauri::command]
fn open_mod_project(app: AppHandle, mod_file_path: String) -> AppResult<project::ModProject> {
    let mod_project = project::read_descriptor(&mod_file_path)?;

    let mut current = settings::load(&app)?;
    settings::remember_project(
        &mut current,
        settings::RecentProject {
            mod_file_path: mod_project.mod_file_path.clone(),
            folder_path: mod_project.folder_path.clone(),
            display_name: mod_project.name.clone(),
            image_path: mod_project.image_path.clone(),
            last_opened: settings::now_ms(),
        },
    );
    settings::save(&app, &current)?;

    Ok(mod_project)
}

/// Reads a descriptor without touching the recent project list.
#[tauri::command]
fn read_mod_project(mod_file_path: String) -> AppResult<project::ModProject> {
    project::read_descriptor(&mod_file_path)
}

#[tauri::command]
fn forget_recent_project(app: AppHandle, folder_path: String) -> AppResult<settings::Settings> {
    let mut current = settings::load(&app)?;
    settings::forget_project(&mut current, &folder_path);
    settings::save(&app, &current)?;
    Ok(current)
}

#[tauri::command]
fn scan_project_files(folder_path: String) -> AppResult<project::ScanResult> {
    project::scan(&folder_path, MAX_PROJECT_FILES)
}

/// Lists one folder, used by the mod and game asset browsers.
#[tauri::command]
fn scan_directory(root_path: String, max_files: Option<usize>) -> AppResult<project::ScanResult> {
    project::scan(&root_path, max_files.unwrap_or(MAX_BROWSE_FILES))
}

#[tauri::command]
fn asset_areas() -> Vec<(String, String)> {
    project::ASSET_AREAS
        .iter()
        .map(|(area, label)| (area.to_string(), label.to_string()))
        .collect()
}

#[tauri::command]
fn read_text_file(path: String) -> AppResult<text_file::TextFile> {
    text_file::read(std::path::Path::new(&path))
}

#[tauri::command]
fn write_text_file(
    cache: State<'_, MapCache>,
    path: String,
    content: String,
    encoding: text_file::Encoding,
    with_bom: bool,
) -> AppResult<text_file::Encoding> {
    let written = text_file::write(std::path::Path::new(&path), &content, encoding, with_bom)?;

    // The scripts page writes any file through here, state definitions and the
    // map's own csv included. Which of them the map is built from is not worth
    // guessing: dropping the cache costs one reload, and only if a map page is
    // opened afterwards.
    drop_map_cache(&cache);

    Ok(written)
}

/// Forgets the decoded map, so the next map call rebuilds it from disk.
fn drop_map_cache(cache: &MapCache) {
    if let Ok(mut slot) = cache.0.lock() {
        *slot = None;
    }
}

#[tauri::command]
fn read_focus_file(path: String) -> AppResult<focus::FocusFile> {
    focus::read(&path)
}

#[tauri::command]
fn update_focus(
    path: String,
    focus_id: String,
    update: focus::FocusUpdate,
) -> AppResult<focus::FocusFile> {
    focus::update(&path, &focus_id, &update)
}

#[tauri::command]
fn add_focus(
    path: String,
    tree_id: String,
    focus: focus::FocusUpdate,
) -> AppResult<focus::FocusFile> {
    focus::add(&path, &tree_id, &focus)
}

#[tauri::command]
fn delete_focus(path: String, focus_id: String) -> AppResult<focus::FocusFile> {
    focus::delete(&path, &focus_id)
}

#[tauri::command]
fn list_localisation_files(
    folder_path: String,
) -> AppResult<Vec<localisation::LocalisationFileInfo>> {
    localisation::scan(&folder_path)
}

#[tauri::command]
fn read_localisation_file(path: String) -> AppResult<localisation::LocalisationFile> {
    localisation::read(&path)
}

#[tauri::command]
fn write_localisation_file(
    path: String,
    language: String,
    entries: Vec<localisation::LocalisationEntry>,
) -> AppResult<localisation::LocalisationFile> {
    localisation::write(&path, &language, &entries)
}

#[tauri::command]
fn read_state_file(path: String) -> AppResult<state::StateFile> {
    state::read(&path)
}

#[tauri::command]
fn update_state(
    cache: State<'_, MapCache>,
    path: String,
    state_id: String,
    update: state::StateUpdate,
) -> AppResult<state::StateFile> {
    let saved = state::update(&path, &state_id, &update)?;

    // Owners, ids and province lists all feed the map. Keeping the cache
    // because the folder has not changed would leave the old colours up and,
    // worse, resolve a click to the owner the state used to have.
    drop_map_cache(&cache);

    Ok(saved)
}

#[tauri::command]
fn list_country_history(
    folder_path: String,
) -> AppResult<Vec<country_history::CountryHistoryInfo>> {
    country_history::scan(&folder_path)
}

#[tauri::command]
fn read_country_history(path: String) -> AppResult<country_history::CountryHistory> {
    country_history::read(&path)
}

#[tauri::command]
fn update_country_history(
    path: String,
    update: country_history::CountryHistoryUpdate,
) -> AppResult<country_history::CountryHistory> {
    country_history::update(&path, &update)
}

/// The decoded province map, kept between calls because rebuilding it means
/// reading a forty megabyte bitmap and thirteen million pixels.
#[derive(Default)]
struct MapCache(Mutex<Option<CachedMap>>);

struct CachedMap {
    key: MapKey,
    data: map::MapData,
}

/// Everything a load reads from, so the cache cannot answer with a map built
/// from different inputs: the game folder can be repointed in Settings, and
/// two descriptors for the same mod folder can declare different replacements.
#[derive(PartialEq, Eq)]
struct MapKey {
    folder: String,
    game_root: String,
    replace_paths: Vec<String>,
}

/// Runs `action` against the map of `folder`, loading it if needed.
fn with_map<T>(
    app: &AppHandle,
    cache: &MapCache,
    folder: &str,
    replace_paths: &[String],
    action: impl FnOnce(&map::MapData) -> AppResult<T>,
) -> AppResult<T> {
    let mut slot = cache
        .0
        .lock()
        .map_err(|_| AppError::message("the map cache was left in a broken state"))?;

    let key = MapKey {
        folder: folder.to_string(),
        game_root: settings::load(app)?.game_root_path,
        replace_paths: replace_paths.to_vec(),
    };

    if slot.as_ref().map(|cached| &cached.key) != Some(&key) {
        let data = map::load(folder, &key.game_root, replace_paths)?;
        *slot = Some(CachedMap { key, data });
    }

    let cached = slot
        .as_ref()
        .ok_or_else(|| AppError::message("the map failed to load"))?;
    action(&cached.data)
}

#[tauri::command]
fn load_map(
    app: AppHandle,
    cache: State<'_, MapCache>,
    folder_path: String,
    replace_paths: Vec<String>,
) -> AppResult<map::MapSummary> {
    with_map(&app, &cache, &folder_path, &replace_paths, |data| {
        Ok(data.summary())
    })
}

#[tauri::command]
fn render_map(
    app: AppHandle,
    cache: State<'_, MapCache>,
    folder_path: String,
    replace_paths: Vec<String>,
    mode: map::MapMode,
) -> AppResult<String> {
    with_map(&app, &cache, &folder_path, &replace_paths, |data| {
        data.render(mode)
    })
}

#[tauri::command]
fn pick_province(
    app: AppHandle,
    cache: State<'_, MapCache>,
    folder_path: String,
    replace_paths: Vec<String>,
    x: u32,
    y: u32,
) -> AppResult<Option<map::ProvincePick>> {
    with_map(&app, &cache, &folder_path, &replace_paths, |data| {
        Ok(data.pick(x, y))
    })
}

#[tauri::command]
fn read_image_data_url(path: String, max_dimension: Option<u32>) -> AppResult<String> {
    assets::image_data_url(&path, max_dimension)
}

#[tauri::command]
fn path_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

/// Checks that a folder looks like a Hearts of Iron IV installation.
#[tauri::command]
fn validate_game_root(path: String) -> AppResult<bool> {
    let root = std::path::Path::new(&path);
    if !root.is_dir() {
        return Err(AppError::message(format!("{path} is not a folder")));
    }

    Ok(root.join("common").is_dir() && root.join("gfx").is_dir())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(MapCache::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            open_mod_project,
            read_mod_project,
            forget_recent_project,
            scan_project_files,
            scan_directory,
            asset_areas,
            read_text_file,
            write_text_file,
            read_focus_file,
            update_focus,
            add_focus,
            delete_focus,
            read_state_file,
            update_state,
            list_country_history,
            read_country_history,
            update_country_history,
            load_map,
            render_map,
            pick_province,
            list_localisation_files,
            read_localisation_file,
            write_localisation_file,
            read_image_data_url,
            path_exists,
            validate_game_root,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Hoi4 Modding Supporter window");
}
