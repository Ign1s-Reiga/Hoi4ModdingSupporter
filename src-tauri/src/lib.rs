mod assets;
mod characters;
mod console;
mod country_history;
mod error;
mod events;
mod focus;
mod ideologies;
mod localisation;
mod map;
mod mcp;
mod paradox;
mod project;
mod settings;
mod sprites;
mod state;
mod technologies;
mod text_file;

use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use console::Source;
use error::{AppError, AppResult};

/// Runs file work on a worker thread and hands the outcome back.
///
/// A command that is not `async` runs on the main thread, and holds up the
/// window and every other command for as long as it takes: indexing the
/// game's sprites or its localisation is minutes on a cold disk. Anything
/// that walks a folder or decodes a picture goes through here.
async fn off_thread<T: Send + 'static>(
    work: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::message(format!("the work panicked: {error}")))?
}

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
async fn scan_project_files(folder_path: String) -> AppResult<project::ScanResult> {
    off_thread(move || project::scan(&folder_path, MAX_PROJECT_FILES)).await
}

/// Lists one folder, used by the mod and game asset browsers.
#[tauri::command]
async fn scan_directory(
    root_path: String,
    max_files: Option<usize>,
) -> AppResult<project::ScanResult> {
    off_thread(move || project::scan(&root_path, max_files.unwrap_or(MAX_BROWSE_FILES))).await
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
    forget_caches(&app, std::path::Path::new(&path));
    console::info(
        &app,
        Source::Files,
        format!("Wrote {path}"),
        Some(format!("{} bytes, {written:?}", content.len())),
    );

    Ok(written)
}

/// Forgets the caches a write to `path` may have invalidated, so the next
/// call rebuilds them from disk. The decoded map and the sprite index go on
/// any write; the localisation index only for a `.yml`, since rebuilding it
/// reads every localisation file the game ships and no other file can
/// change what a key says.
pub(crate) fn forget_caches(app: &AppHandle, path: &std::path::Path) {
    if let Ok(mut slot) = app.state::<MapCache>().0.lock() {
        *slot = None;
    }
    sprites::forget(app);
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("yml"))
    {
        localisation::forget(app);
    }
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
async fn list_localisation_files(
    folder_path: String,
) -> AppResult<Vec<localisation::LocalisationFileInfo>> {
    off_thread(move || localisation::scan(&folder_path)).await
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
    localisation::forget(&app);
    console::info(
        &app,
        Source::Files,
        format!("Wrote {} localisation entries to {path}", entries.len()),
        None,
    );
    Ok(saved)
}

// Characters follow the focus editor's shape: the page holds the text and
// asks for it to be parsed and edited, writing the file itself.

#[tauri::command]
fn read_character_file(path: String) -> AppResult<characters::CharacterFile> {
    characters::read(&path)
}

#[tauri::command]
fn parse_character_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
) -> AppResult<characters::CharacterFile> {
    characters::parse(&path, source, has_bom, encoding)
}

#[tauri::command]
fn update_character_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    character_id: String,
    update: characters::CharacterUpdate,
) -> AppResult<characters::CharacterFile> {
    let updated = characters::update_source(&path, &source, &character_id, &update)?;
    characters::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn add_character_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    character: characters::CharacterUpdate,
) -> AppResult<characters::CharacterFile> {
    let updated = characters::add_source(&path, &source, &character)?;
    characters::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn delete_character_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    character_id: String,
) -> AppResult<characters::CharacterFile> {
    let updated = characters::delete_source(&path, &source, &character_id)?;
    characters::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn read_event_file(path: String) -> AppResult<events::EventFile> {
    events::read(&path)
}

#[tauri::command]
fn parse_event_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
) -> AppResult<events::EventFile> {
    events::parse(&path, source, has_bom, encoding)
}

#[tauri::command]
fn update_event_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    event_id: String,
    update: events::EventUpdate,
) -> AppResult<events::EventFile> {
    let updated = events::update_source(&path, &source, &event_id, &update)?;
    events::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn add_event_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    event: events::EventUpdate,
) -> AppResult<events::EventFile> {
    let updated = events::add_source(&path, &source, &event)?;
    events::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn delete_event_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    event_id: String,
) -> AppResult<events::EventFile> {
    let updated = events::delete_source(&path, &source, &event_id)?;
    events::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn read_ideology_file(path: String) -> AppResult<ideologies::IdeologyFile> {
    ideologies::read(&path)
}

#[tauri::command]
fn parse_ideology_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
) -> AppResult<ideologies::IdeologyFile> {
    ideologies::parse(&path, source, has_bom, encoding)
}

#[tauri::command]
fn update_ideology_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    ideology_id: String,
    update: ideologies::IdeologyUpdate,
) -> AppResult<ideologies::IdeologyFile> {
    let updated = ideologies::update_source(&path, &source, &ideology_id, &update)?;
    ideologies::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn add_ideology_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    ideology: ideologies::IdeologyUpdate,
) -> AppResult<ideologies::IdeologyFile> {
    let updated = ideologies::add_source(&path, &source, &ideology)?;
    ideologies::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn delete_ideology_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    ideology_id: String,
) -> AppResult<ideologies::IdeologyFile> {
    let updated = ideologies::delete_source(&path, &source, &ideology_id)?;
    ideologies::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn read_technology_file(path: String) -> AppResult<technologies::TechnologyFile> {
    technologies::read(&path)
}

#[tauri::command]
fn parse_technology_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
) -> AppResult<technologies::TechnologyFile> {
    technologies::parse(&path, source, has_bom, encoding)
}

#[tauri::command]
fn update_technology_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    tech_id: String,
    update: technologies::TechnologyUpdate,
) -> AppResult<technologies::TechnologyFile> {
    let updated = technologies::update_source(&path, &source, &tech_id, &update)?;
    technologies::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn add_technology_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    technology: technologies::TechnologyUpdate,
) -> AppResult<technologies::TechnologyFile> {
    let updated = technologies::add_source(&path, &source, &technology)?;
    technologies::parse(&path, updated, has_bom, encoding)
}

#[tauri::command]
fn delete_technology_source(
    path: String,
    source: String,
    has_bom: bool,
    encoding: text_file::Encoding,
    tech_id: String,
) -> AppResult<technologies::TechnologyFile> {
    let updated = technologies::delete_source(&path, &source, &tech_id)?;
    technologies::parse(&path, updated, has_bom, encoding)
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
    forget_caches(&app, std::path::Path::new(&path));
    console::info(
        &app,
        Source::Files,
        format!("Updated state {state_id} in {path}"),
        None,
    );

    Ok(saved)
}

#[tauri::command]
async fn list_country_history(
    folder_path: String,
) -> AppResult<Vec<country_history::CountryHistoryInfo>> {
    off_thread(move || country_history::scan(&folder_path)).await
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
async fn load_map(
    app: AppHandle,
    folder_path: String,
    replace_paths: Vec<String>,
) -> AppResult<map::MapSummary> {
    off_thread(move || {
        with_map(
            &app,
            &app.state::<MapCache>(),
            &folder_path,
            &replace_paths,
            |data| Ok(data.summary()),
        )
    })
    .await
}

#[tauri::command]
async fn render_map(
    app: AppHandle,
    folder_path: String,
    replace_paths: Vec<String>,
    mode: map::MapMode,
) -> AppResult<String> {
    off_thread(move || {
        with_map(
            &app,
            &app.state::<MapCache>(),
            &folder_path,
            &replace_paths,
            |data| data.render(mode),
        )
    })
    .await
}

#[tauri::command]
async fn pick_province(
    app: AppHandle,
    folder_path: String,
    replace_paths: Vec<String>,
    x: u32,
    y: u32,
) -> AppResult<Option<map::ProvincePick>> {
    off_thread(move || {
        with_map(
            &app,
            &app.state::<MapCache>(),
            &folder_path,
            &replace_paths,
            |data| Ok(data.pick(x, y)),
        )
    })
    .await
}

#[tauri::command]
async fn read_image_data_url(path: String, max_dimension: Option<u32>) -> AppResult<String> {
    off_thread(move || assets::image_data_url(&path, max_dimension)).await
}

/// The text behind localisation keys, for showing what a key will say.
#[tauri::command]
async fn localised_texts(
    app: AppHandle,
    folder_path: String,
    language: String,
    keys: Vec<String>,
) -> AppResult<std::collections::HashMap<String, String>> {
    off_thread(move || localisation::texts(&app, &folder_path, &language, keys)).await
}

/// Resolves `GFX_...` names to pictures the window can draw.
#[tauri::command]
async fn sprite_icons(
    app: AppHandle,
    folder_path: String,
    names: Vec<String>,
) -> AppResult<Vec<sprites::SpriteIcon>> {
    off_thread(move || sprites::icons(&app, &folder_path, names)).await
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
        .manage(localisation::TextCache::default())
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
            read_character_file,
            parse_character_source,
            update_character_source,
            add_character_source,
            delete_character_source,
            read_event_file,
            parse_event_source,
            update_event_source,
            add_event_source,
            delete_event_source,
            read_ideology_file,
            parse_ideology_source,
            update_ideology_source,
            add_ideology_source,
            delete_ideology_source,
            read_technology_file,
            parse_technology_source,
            update_technology_source,
            add_technology_source,
            delete_technology_source,
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
            localised_texts,
            sprite_icons,
            path_exists,
            validate_game_root,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Hoi4 Modding Supporter window");
}
