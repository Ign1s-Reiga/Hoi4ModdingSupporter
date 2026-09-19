mod assets;
mod console;
mod country_history;
mod error;
mod focus;
mod localisation;
mod map;
mod mcp;
mod paradox;
mod project;
mod settings;
mod sprites;
mod state;
mod text_file;

use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use console::Source;
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
    let before = settings::load(&app)?;
    settings::save(&app, &settings)?;

    // The server reads its port and token once, at start, so a change to
    // either takes effect by starting it again. Left alone otherwise: a
    // theme change must not cut an assistant off mid-edit.
    if before.mcp != settings.mcp {
        if let Err(error) = mcp::restart(&app) {
            console::error(&app, Source::Mcp, error.to_string(), None);
        }
    }

    Ok(settings)
}

/// The window reports which project it has open, so the MCP server works on
/// the same one. `None` when the project is closed.
#[tauri::command]
fn set_open_project(app: AppHandle, project: Option<project::ModProject>) {
    let open = app.state::<project::Open>();

    // The window reports on every restore, which React runs twice in
    // development; the same folder twice over is not a second opening.
    let already = open.current().map(|current| current.folder_path).as_deref()
        == project.as_ref().map(|next| next.folder_path.as_str());

    if let (false, Some(project)) = (already, &project) {
        console::info(
            &app,
            Source::App,
            format!("Opened {} ({})", project.name, project.folder_path),
            None,
        );
    }
    open.set(project);
}

#[tauri::command]
fn console_entries(console: State<'_, console::Console>) -> Vec<console::Entry> {
    console.entries()
}

#[tauri::command]
fn console_clear(app: AppHandle) {
    console::clear(&app);
}

#[tauri::command]
fn mcp_status(app: AppHandle) -> AppResult<mcp::McpStatus> {
    mcp::status(&app)
}

/// Mints a new bearer token and restarts the server with it, so any client
/// still holding the old one is cut off.
#[tauri::command]
fn mcp_regenerate_token(app: AppHandle) -> AppResult<mcp::McpStatus> {
    let mut settings = settings::load(&app)?;
    settings.mcp.token = settings::new_token();
    settings::save(&app, &settings)?;
    console::info(&app, Source::Mcp, "MCP token regenerated", None);
    mcp::restart(&app)
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
    app: AppHandle,
    path: String,
    content: String,
    encoding: text_file::Encoding,
    with_bom: bool,
) -> AppResult<text_file::Encoding> {
    let written = text_file::write(std::path::Path::new(&path), &content, encoding, with_bom)?;

    // The scripts page writes any file through here, state definitions, the
    // map's own csv and `.gfx` sprite definitions included. Which of them the
    // caches are built from is not worth guessing: dropping them costs one
    // reload, and only if a page needing them is opened afterwards.
    forget_caches(&app);
    console::info(
        &app,
        Source::Files,
        format!("Wrote {path}"),
        Some(format!("{} bytes, {written:?}", content.len())),
    );

    Ok(written)
}

/// Forgets the decoded map and the sprite index, so the next call to either
/// rebuilds it from disk. Any write can invalidate both.
pub(crate) fn forget_caches(app: &AppHandle) {
    if let Ok(mut slot) = app.state::<MapCache>().0.lock() {
        *slot = None;
    }
    sprites::forget(app);
}

#[tauri::command]
fn read_focus_file(path: String) -> AppResult<focus::FocusFile> {
    focus::read(&path)
}

// The code view holds the file's text itself and edits that, writing the
// whole file when the user asks. These take the text in and hand it back
// re-read, so the tree follows every keystroke and every applied field
// without a disk round trip. `path` names the file for error messages only.

#[tauri::command]
fn parse_focus_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
) -> AppResult<focus::FocusFile> {
    focus::parse(&path, source, has_bom, encoding)
}

#[tauri::command]
fn update_focus_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    focus_id: String,
    update: focus::FocusUpdate,
) -> AppResult<focus::FocusFile> {
    let updated = focus::update_source(&path, &source, &focus_id, &update)?;
    focus::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn add_focus_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    tree_id: String,
    focus: focus::FocusUpdate,
) -> AppResult<focus::FocusFile> {
    let updated = focus::add_source(&path, &source, &tree_id, &focus)?;
    focus::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn delete_focus_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    focus_id: String,
) -> AppResult<focus::FocusFile> {
    let updated = focus::delete_source(&path, &source, &focus_id)?;
    focus::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn update_focus(
    app: AppHandle,
    path: String,
    focus_id: String,
    update: focus::FocusUpdate,
) -> AppResult<focus::FocusFile> {
    let saved = focus::update(&path, &focus_id, &update)?;
    console::info(
        &app,
        Source::Files,
        format!("Updated focus {focus_id} in {path}"),
        None,
    );
    Ok(saved)
}

#[tauri::command]
fn add_focus(
    app: AppHandle,
    path: String,
    tree_id: String,
    focus: focus::FocusUpdate,
) -> AppResult<focus::FocusFile> {
    let saved = focus::add(&path, &tree_id, &focus)?;
    console::info(
        &app,
        Source::Files,
        format!("Added focus {} to {path}", focus.id),
        None,
    );
    Ok(saved)
}

#[tauri::command]
fn delete_focus(app: AppHandle, path: String, focus_id: String) -> AppResult<focus::FocusFile> {
    let saved = focus::delete(&path, &focus_id)?;
    console::info(
        &app,
        Source::Files,
        format!("Deleted focus {focus_id} from {path}"),
        None,
    );
    Ok(saved)
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
    app: AppHandle,
    path: String,
    language: String,
    entries: Vec<localisation::LocalisationEntry>,
) -> AppResult<localisation::LocalisationFile> {
    let saved = localisation::write(&path, &language, &entries)?;
    console::info(
        &app,
        Source::Files,
        format!("Wrote {} localisation entries to {path}", entries.len()),
        None,
    );
    Ok(saved)
}

#[tauri::command]
fn read_state_file(path: String) -> AppResult<state::StateFile> {
    state::read(&path)
}

#[tauri::command]
fn update_state(
    app: AppHandle,
    path: String,
    state_id: String,
    update: state::StateUpdate,
) -> AppResult<state::StateFile> {
    let saved = state::update(&path, &state_id, &update)?;

    // Owners, ids and province lists all feed the map. Keeping the cache
    // because the folder has not changed would leave the old colours up and,
    // worse, resolve a click to the owner the state used to have.
    forget_caches(&app);
    console::info(
        &app,
        Source::Files,
        format!("Updated state {state_id} in {path}"),
        None,
    );

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
    app: AppHandle,
    path: String,
    update: country_history::CountryHistoryUpdate,
) -> AppResult<country_history::CountryHistory> {
    let saved = country_history::update(&path, &update)?;
    console::info(
        &app,
        Source::Files,
        format!("Updated country setup in {path}"),
        None,
    );
    Ok(saved)
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
        let started = std::time::Instant::now();
        let data = map::load(folder, &key.game_root, replace_paths)?;
        let summary = data.summary();
        console::info(
            app,
            Source::App,
            format!(
                "Loaded map: {} provinces, {} states in {} ms",
                summary.province_count,
                summary.state_count,
                started.elapsed().as_millis()
            ),
            None,
        );
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

/// Resolves `GFX_...` names to pictures the window can draw.
#[tauri::command]
fn sprite_icons(
    app: AppHandle,
    folder_path: String,
    names: Vec<String>,
) -> AppResult<Vec<sprites::SpriteIcon>> {
    sprites::icons(&app, &folder_path, names)
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
        .manage(sprites::SpriteCache::default())
        .manage(console::Console::default())
        .manage(project::Open::default())
        .manage(mcp::McpState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // A port in use is not worth refusing to open the window over:
            // the editors work without the server, and Settings shows why.
            if let Err(error) = mcp::start(app.handle()) {
                console::error(app.handle(), Source::Mcp, error.to_string(), None);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            set_open_project,
            console_entries,
            console_clear,
            mcp_status,
            mcp_regenerate_token,
            open_mod_project,
            read_mod_project,
            forget_recent_project,
            scan_project_files,
            scan_directory,
            asset_areas,
            read_text_file,
            write_text_file,
            read_focus_file,
            parse_focus_source,
            update_focus_source,
            add_focus_source,
            delete_focus_source,
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
            sprite_icons,
            path_exists,
            validate_game_root,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Hoi4 Modding Supporter window");
}
