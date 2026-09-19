//! The MCP server: the app's editors, offered to an assistant.
//!
//! An assistant such as Claude Code connects over HTTP to the running window
//! and works on the mod that is open in it, through the same surgical editors
//! the forms use. It never writes a whole file to change one field, because
//! the tools it is given do not. Every call is recorded in the console, so
//! what it did is visible in the same place the user's own saves are.
//!
//! Two layers keep the endpoint to its owner. The transport refuses requests
//! whose `Host` is not loopback or that carry a browser `Origin`, which shuts
//! out DNS rebinding and any page that tries to reach `127.0.0.1` from inside
//! the user's browser. On top of that, every request must carry the bearer
//! token from Settings, so another local process cannot drive the editor
//! either.

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ContentBlock, Implementation, ServerCapabilities,
    ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

use crate::console::{self, Source};
use crate::error::{AppError, AppResult};
use crate::project::{self, normalise_path, ModProject};
use crate::{country_history, focus, localisation, settings, sprites, state, text_file};

/// Event carrying the path of a file an MCP client changed, so an editor
/// showing it can reload.
pub const FILE_CHANGED_EVENT: &str = "project://file-changed";

/// Longest result excerpt kept in a console entry.
const DETAIL_EXCERPT: usize = 600;
/// Default cap for `search_sprites`.
const SEARCH_LIMIT: usize = 50;
/// Cap for `list_files`; a mod folder can hold far more than an assistant
/// wants to read in one answer.
const LIST_LIMIT: usize = 20_000;

const INSTRUCTIONS: &str = "This server is the Hoi4 Modding Supporter desktop app. It edits the Hearts of Iron IV mod \
that is open in the app window; call `project_info` first to learn which mod that is. Paths given to tools \
are relative to the mod folder unless absolute. Reading may reach into the game folder by absolute path; \
writing is confined to the mod folder. Prefer the focus, state, localisation and country tools over \
`write_file`: they change one field and leave the rest of the file byte-for-byte, comments included. \
Sprite names (`GFX_...`) can be looked up with `search_sprites` before assigning a focus icon.";

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// The server, if it is running.
#[derive(Default)]
pub struct McpState(Mutex<Option<Running>>);

struct Running {
    address: String,
    cancel: CancellationToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub enabled: bool,
    pub running: bool,
    /// The URL a client connects to, when running.
    pub address: Option<String>,
    pub port: u16,
    pub token: String,
    pub tool_count: usize,
}

/// Starts the server when Settings says so. `Ok(None)` means it is disabled;
/// a port that cannot be bound is an error, reported rather than retried so
/// the user can pick another in Settings.
pub fn start(app: &AppHandle) -> AppResult<Option<String>> {
    let settings = settings::load(app)?;
    if !settings.mcp.enabled {
        return Ok(None);
    }

    let state = app.state::<McpState>();
    let mut slot = state
        .0
        .lock()
        .map_err(|_| AppError::message("the MCP server state was left in a broken state"))?;

    if let Some(running) = slot.as_ref() {
        return Ok(Some(running.address.clone()));
    }

    // Bound synchronously so a port already in use is reported to the caller
    // instead of to a log line nobody is watching.
    let bind = format!("127.0.0.1:{}", settings.mcp.port);
    let listener = bind_with_retry(&bind)?;
    listener.set_nonblocking(true).map_err(|error| {
        AppError::message(format!(
            "the MCP server could not listen on {bind}: {error}"
        ))
    })?;

    let address = format!("http://{bind}/mcp");
    let cancel = CancellationToken::new();
    let token = Arc::new(settings.mcp.token.clone());

    let service_app = app.clone();
    let task_app = app.clone();
    let service_cancel = cancel.child_token();
    let shutdown = cancel.clone();

    tauri::async_runtime::spawn(async move {
        let service = StreamableHttpService::new(
            move || Ok(Server::new(service_app.clone())),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default()
                // With no allowlist, any request carrying an Origin is refused:
                // MCP clients send none, browsers always do.
                .enforce_origin_validation()
                .with_cancellation_token(service_cancel),
        );

        let router = axum::Router::new()
            .nest_service("/mcp", service)
            .layer(axum::middleware::from_fn_with_state(token, require_token));

        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                console::error(
                    &task_app,
                    Source::Mcp,
                    format!("MCP server could not start: {error}"),
                    None,
                );
                return;
            }
        };

        let served = axum::serve(listener, router)
            .with_graceful_shutdown(async move { shutdown.cancelled().await })
            .await;

        if let Err(error) = served {
            console::error(
                &task_app,
                Source::Mcp,
                format!("MCP server stopped: {error}"),
                None,
            );
        }
    });

    *slot = Some(Running {
        address: address.clone(),
        cancel,
    });
    console::info(
        app,
        Source::Mcp,
        format!("MCP server listening at {address}"),
        None,
    );

    Ok(Some(address))
}

/// A restart rebinds the port the previous server is still letting go of,
/// so a few short retries are allowed before "in use" is taken at its word.
fn bind_with_retry(bind: &str) -> AppResult<std::net::TcpListener> {
    let mut attempt = 0;
    loop {
        match std::net::TcpListener::bind(bind) {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse && attempt < 10 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(error) => {
                return Err(AppError::message(format!(
                    "the MCP server could not listen on {bind}: {error}"
                )))
            }
        }
    }
}

/// Stops the server if it is running. Sessions are cut; a client will see
/// its next request fail and reconnect when the server is back.
pub fn stop(app: &AppHandle) {
    let state = app.state::<McpState>();
    let running = state.0.lock().ok().and_then(|mut slot| slot.take());

    if let Some(running) = running {
        running.cancel.cancel();
        console::info(
            app,
            Source::Mcp,
            format!("MCP server stopped ({})", running.address),
            None,
        );
    }
}

/// Applies the current Settings: stops a server whose port or token changed,
/// starts one when enabled. Called after Settings are saved.
pub fn restart(app: &AppHandle) -> AppResult<McpStatus> {
    stop(app);
    start(app)?;
    status(app)
}

pub fn status(app: &AppHandle) -> AppResult<McpStatus> {
    let settings = settings::load(app)?;
    let address = app
        .state::<McpState>()
        .0
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|running| running.address.clone()));

    Ok(McpStatus {
        enabled: settings.mcp.enabled,
        running: address.is_some(),
        address,
        port: settings.mcp.port,
        token: settings.mcp.token,
        tool_count: Server::tool_router().list_all().len(),
    })
}

/// Rejects any request without the bearer token from Settings.
async fn require_token(
    State(expected): State<Arc<String>>,
    request: Request,
    next: Next,
) -> Response {
    let presented = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);

    match presented {
        Some(token) if same_token(token, &expected) => next.run(request).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            "this endpoint needs the bearer token shown in the app's Settings",
        )
            .into_response(),
    }
}

/// Compares without short-circuiting on the first differing byte, so timing
/// does not leak how much of a guess was right.
fn same_token(presented: &str, expected: &str) -> bool {
    let presented = presented.as_bytes();
    let expected = expected.as_bytes();
    if presented.len() != expected.len() {
        return false;
    }

    presented
        .iter()
        .zip(expected)
        .fold(0u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

// ---------------------------------------------------------------------------
// The handler
// ---------------------------------------------------------------------------

/// One MCP session. Holds nothing of its own: the open project, the caches
/// and the console all live in Tauri's managed state behind the handle.
#[derive(Clone)]
struct Server {
    app: AppHandle,
    tool_router: ToolRouter<Self>,
}

/// Where a session may read and write, taken from the project open in the
/// window at the moment of each call, so switching mods in the app switches
/// the assistant too.
struct Scope {
    project: ModProject,
    game_root: String,
}

impl Scope {
    fn current(app: &AppHandle) -> Result<Self, String> {
        let project = app.state::<project::Open>().current().ok_or_else(|| {
            "No mod is open in the app. Open one from its Home screen first.".to_string()
        })?;
        let game_root = settings::load(app)
            .map(|settings| settings.game_root_path)
            .map_err(|error| error.to_string())?;

        Ok(Self { project, game_root })
    }

    fn folder(&self) -> &Path {
        Path::new(&self.project.folder_path)
    }

    /// A path to read: inside the mod, or inside the game folder by absolute
    /// path. Vanilla files are what a modder compares against.
    fn read_path(&self, given: &str) -> Result<PathBuf, String> {
        let candidate = self.candidate(given)?;

        if within(self.folder(), &candidate) {
            return Ok(candidate);
        }
        if !self.game_root.trim().is_empty() && within(Path::new(&self.game_root), &candidate) {
            return Ok(candidate);
        }

        Err(format!(
            "{given} is outside the mod folder and the game folder; only those can be read"
        ))
    }

    /// A path to write: inside the mod, nowhere else. The game install is
    /// never touched, whatever the assistant is asked.
    fn write_path(&self, given: &str) -> Result<PathBuf, String> {
        let candidate = self.candidate(given)?;

        if within(self.folder(), &candidate) {
            return Ok(candidate);
        }

        Err(format!(
            "{given} is outside the mod folder; only the mod's own files can be written"
        ))
    }

    fn candidate(&self, given: &str) -> Result<PathBuf, String> {
        let given = given.trim();
        if given.is_empty() {
            return Err("a path is required".to_string());
        }

        let path = Path::new(given);
        Ok(if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.folder().join(path)
        })
    }
}

/// Whether `candidate` lies under `root`, decided on the spelled path with
/// `.` and `..` resolved — not on the disk, because a file about to be
/// created does not exist yet. Case-insensitive, as Windows paths are.
fn within(root: &Path, candidate: &Path) -> bool {
    let root = normalise_path(&lexical(root))
        .to_lowercase()
        .trim_end_matches('/')
        .to_string();
    let candidate = normalise_path(&lexical(candidate)).to_lowercase();

    candidate == root || candidate.starts_with(&format!("{root}/"))
}

fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Runs file work off the async runtime and turns the outcome into the JSON
/// text a tool returns. `AppError` becomes a tool error the assistant can
/// read and act on, not a protocol failure.
async fn blocking<T: Serialize + Send + 'static>(
    work: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> Result<String, String> {
    let outcome = tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("the tool's work panicked: {error}"))?;

    let value = outcome.map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

// ---------------------------------------------------------------------------
// Tool inputs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct PathInput {
    /// Path of the file, relative to the mod folder or absolute.
    path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ListFilesInput {
    /// Folder inside the mod to list, e.g. `common/national_focus`. Empty for
    /// the whole mod.
    #[serde(default)]
    folder: String,
    /// Only files with this extension, e.g. `txt` or `yml`. Empty for all.
    #[serde(default)]
    extension: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct WriteFileInput {
    /// Path of the file inside the mod, relative to the mod folder or absolute.
    path: String,
    /// The complete new text of the file.
    content: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateFocusInput {
    /// The focus file, relative to the mod folder or absolute.
    path: String,
    /// Id of the focus to change.
    focus_id: String,
    /// Every editable field. Send the current value for fields to keep; an
    /// empty string removes a field from the focus.
    update: focus::FocusUpdate,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct AddFocusInput {
    /// The focus file, relative to the mod folder or absolute.
    path: String,
    /// Id of the `focus_tree` to append to. Empty means the file's first
    /// tree; a file with no `focus_tree` block is refused.
    #[serde(default)]
    tree_id: String,
    /// The new focus. `id`, `icon`, `x`, `y` and `cost` are what the game needs.
    focus: focus::FocusUpdate,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct DeleteFocusInput {
    /// The focus file, relative to the mod folder or absolute.
    path: String,
    /// Id of the focus to remove.
    focus_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SearchSpritesInput {
    /// Text the sprite name must contain, case-insensitive, e.g. `generic_army`.
    query: String,
    /// Most names to return. Defaults to 50.
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SpriteInput {
    /// The sprite name as a script uses it, e.g. `GFX_focus_generic_industry`.
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct WriteLocalisationInput {
    /// The `.yml` file, relative to the mod folder or absolute.
    path: String,
    /// Language the file is for, e.g. `english`.
    language: String,
    /// The complete list of entries the file should hold, in order.
    entries: Vec<localisation::LocalisationEntry>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateStateInput {
    /// The state file, relative to the mod folder or absolute.
    path: String,
    /// Id of the state to change.
    state_id: String,
    /// Every editable field. Send the current value for fields to keep.
    update: state::StateUpdate,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateCountryInput {
    /// The `history/countries` file, relative to the mod folder or absolute.
    path: String,
    /// Every editable field. Send the current value for fields to keep.
    update: country_history::CountryHistoryUpdate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectInfo {
    name: String,
    version: String,
    supported_version: String,
    folder_path: String,
    mod_file_path: String,
    replace_paths: Vec<String>,
    /// Empty when the game folder is not set in Settings.
    game_root_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListedFile {
    /// Relative to the mod folder, with forward slashes.
    relative_path: String,
    size_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WrittenFile {
    path: String,
    encoding: text_file::Encoding,
    bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpriteTexture {
    name: String,
    /// Absolute path of the texture that resolved.
    path: String,
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

#[tool_router]
impl Server {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    /// Tells the window a file changed underneath it. Failing to deliver is
    /// harmless: the editor simply shows the old text until it is reopened.
    fn changed(&self, path: &Path) {
        let _ = self.app.emit(
            FILE_CHANGED_EVENT,
            serde_json::json!({ "path": normalise_path(path) }),
        );
        crate::forget_caches(&self.app);
    }

    #[tool(
        description = "The mod open in the app: name, folder, descriptor, replace_path entries and the game folder. Call this first; paths given to the other tools are relative to the mod folder.",
        annotations(read_only_hint = true)
    )]
    async fn project_info(&self) -> Result<String, String> {
        let scope = Scope::current(&self.app)?;
        let info = ProjectInfo {
            name: scope.project.name,
            version: scope.project.version,
            supported_version: scope.project.supported_version,
            folder_path: scope.project.folder_path,
            mod_file_path: scope.project.mod_file_path,
            replace_paths: scope.project.replace_paths,
            game_root_path: scope.game_root,
        };

        serde_json::to_string_pretty(&info).map_err(|error| error.to_string())
    }

    #[tool(
        description = "Files in the open mod, optionally under one folder and with one extension. Paths come back relative to the mod folder.",
        annotations(read_only_hint = true)
    )]
    async fn list_files(
        &self,
        Parameters(input): Parameters<ListFilesInput>,
    ) -> Result<String, String> {
        let scope = Scope::current(&self.app)?;
        let root = scope.folder().to_path_buf();
        let folder = input
            .folder
            .trim()
            .replace('\\', "/")
            .trim_matches('/')
            .to_lowercase();
        let extension = input
            .extension
            .trim()
            .trim_start_matches('.')
            .to_lowercase();

        blocking(move || {
            let scan = project::scan(&normalise_path(&root), LIST_LIMIT)?;
            let files: Vec<ListedFile> = scan
                .files
                .into_iter()
                .filter(|file| {
                    let relative = file.relative_path.to_lowercase();
                    let in_folder = folder.is_empty()
                        || relative == folder
                        || relative.starts_with(&format!("{folder}/"));
                    let has_extension = extension.is_empty() || file.extension == extension;
                    in_folder && has_extension
                })
                .map(|file| ListedFile {
                    relative_path: file.relative_path,
                    size_bytes: file.size_bytes,
                })
                .collect();

            Ok(files)
        })
        .await
    }

    #[tool(
        description = "The text of a script file in the mod or, by absolute path, in the game folder.",
        annotations(read_only_hint = true)
    )]
    async fn read_file(&self, Parameters(input): Parameters<PathInput>) -> Result<String, String> {
        let path = Scope::current(&self.app)?.read_path(&input.path)?;

        let file = tokio::task::spawn_blocking(move || text_file::read(&path))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;

        Ok(file.content)
    }

    #[tool(
        description = "Replaces the whole text of a file inside the mod folder, creating it and its folders if needed. An existing file keeps its encoding and byte-order mark. Prefer the focus, state, localisation and country tools for the edits they cover: they change one field and leave the rest of the file untouched.",
        annotations(destructive_hint = true)
    )]
    async fn write_file(
        &self,
        Parameters(input): Parameters<WriteFileInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.write_path(&input.path)?;
        let content = input.content;
        let written_path = path.clone();

        let result = blocking(move || {
            // A file the game already loads keeps the bytes it loads; a new
            // one is UTF-8, with the BOM localisation needs when it is one.
            let (encoding, with_bom) = match text_file::read(&path) {
                Ok(existing) => (existing.encoding, existing.has_bom),
                Err(_) => (
                    text_file::Encoding::Utf8,
                    matches!(
                        path.extension().and_then(|value| value.to_str()),
                        Some("yml") | Some("yaml")
                    ),
                ),
            };
            let encoding = text_file::write(&path, &content, encoding, with_bom)?;

            Ok(WrittenFile {
                path: normalise_path(&path),
                encoding,
                bytes: content.len(),
            })
        })
        .await;

        if result.is_ok() {
            self.changed(&written_path);
        }
        result
    }

    #[tool(
        description = "A national focus file: its focus trees and every focus with id, icon, position, cost, prerequisites, mutual exclusions and script blocks, plus the line each is on and the file's full text as `source`.",
        annotations(read_only_hint = true)
    )]
    async fn read_focus_file(
        &self,
        Parameters(input): Parameters<PathInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.read_path(&input.path)?;
        blocking(move || focus::read(&normalise_path(&path))).await
    }

    #[tool(
        description = "Rewrites one focus's fields in place, leaving everything else in the file exactly as written. Returns the file as re-read."
    )]
    async fn update_focus(
        &self,
        Parameters(input): Parameters<UpdateFocusInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.write_path(&input.path)?;
        let changed = path.clone();
        let result =
            blocking(move || focus::update(&normalise_path(&path), &input.focus_id, &input.update))
                .await;

        if result.is_ok() {
            self.changed(&changed);
        }
        result
    }

    #[tool(
        description = "Appends a new focus to a focus_tree in the file: the one named by treeId, or the file's first when treeId is empty. Files holding only shared_focus blocks are refused. Returns the file as re-read."
    )]
    async fn add_focus(
        &self,
        Parameters(input): Parameters<AddFocusInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.write_path(&input.path)?;
        let changed = path.clone();
        let result =
            blocking(move || focus::add(&normalise_path(&path), &input.tree_id, &input.focus))
                .await;

        if result.is_ok() {
            self.changed(&changed);
        }
        result
    }

    #[tool(
        description = "Removes one focus from the file. Other focuses that name it as a prerequisite or exclusion are left for you to fix. Returns the file as re-read.",
        annotations(destructive_hint = true)
    )]
    async fn delete_focus(
        &self,
        Parameters(input): Parameters<DeleteFocusInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.write_path(&input.path)?;
        let changed = path.clone();
        let result = blocking(move || focus::delete(&normalise_path(&path), &input.focus_id)).await;

        if result.is_ok() {
            self.changed(&changed);
        }
        result
    }

    #[tool(
        description = "Sprite (GFX_) names defined by the game and the mod whose name contains the query, with the texture each points at. Use it to choose a focus icon.",
        annotations(read_only_hint = true)
    )]
    async fn search_sprites(
        &self,
        Parameters(input): Parameters<SearchSpritesInput>,
    ) -> Result<String, String> {
        let scope = Scope::current(&self.app)?;
        let app = self.app.clone();
        let folder = scope.project.folder_path;
        let limit = input.limit.unwrap_or(SEARCH_LIMIT).clamp(1, 500);

        blocking(move || {
            sprites::with_index(&app, &folder, |index, _, _| {
                index.search(&input.query, limit)
            })
        })
        .await
    }

    #[tool(
        description = "Where a sprite's texture is on disk, resolved the way the game does: the mod's copy first, then the game's. An error means the name is undefined or the texture is missing.",
        annotations(read_only_hint = true)
    )]
    async fn sprite_texture(
        &self,
        Parameters(input): Parameters<SpriteInput>,
    ) -> Result<String, String> {
        let scope = Scope::current(&self.app)?;
        let app = self.app.clone();
        let folder = scope.project.folder_path;
        let name = input.name.clone();

        let found = tokio::task::spawn_blocking(move || {
            sprites::with_index(&app, &folder, |index, folder, game_root| {
                index.texture_path(&input.name, folder, game_root)
            })
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;

        match found {
            Some(path) => serde_json::to_string_pretty(&SpriteTexture {
                name,
                path: normalise_path(&path),
            })
            .map_err(|error| error.to_string()),
            None => Err(format!(
                "{name} is not defined in any interface/*.gfx file, or its texture is not on disk"
            )),
        }
    }

    #[tool(
        description = "A localisation .yml file: its language and every key with its version and text.",
        annotations(read_only_hint = true)
    )]
    async fn read_localisation_file(
        &self,
        Parameters(input): Parameters<PathInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.read_path(&input.path)?;
        blocking(move || localisation::read(&normalise_path(&path))).await
    }

    #[tool(
        description = "Writes a localisation .yml file with the given entries, as UTF-8 with the byte-order mark the game requires. Entries whose text did not change are copied through untouched. Returns the file as re-read."
    )]
    async fn write_localisation_file(
        &self,
        Parameters(input): Parameters<WriteLocalisationInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.write_path(&input.path)?;
        let changed = path.clone();
        let result = blocking(move || {
            localisation::write(&normalise_path(&path), &input.language, &input.entries)
        })
        .await;

        if result.is_ok() {
            self.changed(&changed);
        }
        result
    }

    #[tool(
        description = "A history/states file: every state with its provinces, owner, cores, buildings and victory points.",
        annotations(read_only_hint = true)
    )]
    async fn read_state_file(
        &self,
        Parameters(input): Parameters<PathInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.read_path(&input.path)?;
        blocking(move || state::read(&normalise_path(&path))).await
    }

    #[tool(
        description = "Rewrites one state's fields in place, leaving the rest of the file exactly as written. Returns the file as re-read."
    )]
    async fn update_state(
        &self,
        Parameters(input): Parameters<UpdateStateInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.write_path(&input.path)?;
        let changed = path.clone();
        let result =
            blocking(move || state::update(&normalise_path(&path), &input.state_id, &input.update))
                .await;

        if result.is_ok() {
            self.changed(&changed);
        }
        result
    }

    #[tool(
        description = "A history/countries file: capital, starting setup, politics and party popularities.",
        annotations(read_only_hint = true)
    )]
    async fn read_country_history(
        &self,
        Parameters(input): Parameters<PathInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.read_path(&input.path)?;
        blocking(move || country_history::read(&normalise_path(&path))).await
    }

    #[tool(
        description = "Rewrites a country's starting setup in place, leaving the rest of the file exactly as written. Returns the file as re-read."
    )]
    async fn update_country_history(
        &self,
        Parameters(input): Parameters<UpdateCountryInput>,
    ) -> Result<String, String> {
        let path = Scope::current(&self.app)?.write_path(&input.path)?;
        let changed = path.clone();
        let result =
            blocking(move || country_history::update(&normalise_path(&path), &input.update)).await;

        if result.is_ok() {
            self.changed(&changed);
        }
        result
    }
}

#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        // `from_build_env` would name the server after the crate that compiled
        // it, which is rmcp, not this app.
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "hoi4-modding-supporter",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }

    /// The generated dispatcher, wrapped so every call lands in the console
    /// with its arguments, how it went and how long it took.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let name = request.name.to_string();
        let arguments = request
            .arguments
            .as_ref()
            .map(|arguments| serde_json::Value::Object(arguments.clone()).to_string());

        let started = Instant::now();
        let outcome = self
            .tool_router
            .call(ToolCallContext::new(self, request, context))
            .await;
        let elapsed = started.elapsed().as_millis();

        match &outcome {
            Ok(CallToolResponse::Complete(result)) if result.is_error == Some(true) => {
                console::warn(
                    &self.app,
                    Source::Mcp,
                    format!("{name} failed ({elapsed} ms)"),
                    Some(detail(arguments.as_deref(), first_text(&result.content))),
                );
            }
            Ok(CallToolResponse::Complete(result)) => {
                console::info(
                    &self.app,
                    Source::Mcp,
                    format!("{name} ({elapsed} ms)"),
                    Some(detail(arguments.as_deref(), first_text(&result.content))),
                );
            }
            Ok(_) => {
                console::info(
                    &self.app,
                    Source::Mcp,
                    format!("{name} ({elapsed} ms)"),
                    Some(detail(arguments.as_deref(), None)),
                );
            }
            Err(error) => {
                console::error(
                    &self.app,
                    Source::Mcp,
                    format!("{name}: {error}"),
                    Some(detail(arguments.as_deref(), None)),
                );
            }
        }

        outcome
    }
}

fn first_text(content: &[ContentBlock]) -> Option<&str> {
    content
        .iter()
        .find_map(|block| block.as_text().map(|text| text.text.as_str()))
}

/// The expandable part of a console entry: the arguments as sent and the
/// start of what came back.
fn detail(arguments: Option<&str>, result: Option<&str>) -> String {
    let mut text = format!("arguments: {}", arguments.unwrap_or("{}"));
    if let Some(result) = result {
        text.push_str("\n\nresult: ");
        text.push_str(&excerpt(result));
    }
    text
}

fn excerpt(text: &str) -> String {
    if text.len() <= DETAIL_EXCERPT {
        return text.to_string();
    }

    let mut cut = DETAIL_EXCERPT;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}… ({} more bytes)", &text[..cut], text.len() - cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(folder: &str, game: &str) -> Scope {
        Scope {
            project: ModProject {
                mod_file_path: format!("{folder}.mod"),
                folder_path: folder.to_string(),
                name: "Test".to_string(),
                version: String::new(),
                supported_version: String::new(),
                tags: Vec::new(),
                image_path: String::new(),
                replace_paths: Vec::new(),
            },
            game_root: game.to_string(),
        }
    }

    #[test]
    fn relative_paths_resolve_inside_the_mod() {
        let scope = scope("C:/mods/alpha", "D:/game");

        let path = scope
            .write_path("common/national_focus/a.txt")
            .expect("inside");

        assert_eq!(
            normalise_path(&path),
            "C:/mods/alpha/common/national_focus/a.txt"
        );
    }

    #[test]
    fn dot_dot_cannot_climb_out_of_the_mod() {
        let scope = scope("C:/mods/alpha", "D:/game");

        assert!(scope.write_path("../beta/descriptor.mod").is_err());
        assert!(scope.write_path("common/../../../Windows/win.ini").is_err());
        // Climbing out and back in is still inside.
        assert!(scope.write_path("common/../common/a.txt").is_ok());
    }

    #[test]
    fn the_game_folder_can_be_read_but_never_written() {
        let scope = scope("C:/mods/alpha", "D:/game");

        assert!(scope
            .read_path("D:/game/common/countries/colors.txt")
            .is_ok());
        assert!(scope
            .write_path("D:/game/common/countries/colors.txt")
            .is_err());
    }

    #[test]
    fn anywhere_else_is_refused_even_to_read() {
        let scope = scope("C:/mods/alpha", "D:/game");

        assert!(scope.read_path("C:/mods/beta/a.txt").is_err());
        assert!(
            scope.read_path("C:/mods/alphabet/a.txt").is_err(),
            "a sibling sharing a prefix is not inside"
        );
        assert!(scope.read_path("").is_err());
    }

    #[test]
    fn an_unset_game_folder_grants_nothing_extra() {
        let scope = scope("C:/mods/alpha", "");

        assert!(scope.read_path("D:/anything/at/all.txt").is_err());
    }

    #[test]
    fn paths_match_regardless_of_case_and_separators() {
        assert!(within(
            Path::new("C:/Mods/Alpha"),
            Path::new("c:\\mods\\alpha\\common\\a.txt")
        ));
        assert!(within(
            Path::new("C:/mods/alpha/"),
            Path::new("C:/mods/alpha")
        ));
    }

    #[test]
    fn token_comparison_needs_an_exact_match() {
        assert!(same_token("abc123", "abc123"));
        assert!(!same_token("abc124", "abc123"));
        assert!(!same_token("abc12", "abc123"));
        assert!(!same_token("", "abc123"));
    }

    #[test]
    fn excerpts_cut_on_a_character_boundary() {
        // Two bytes per character, so the byte limit lands mid-character.
        let long = "é".repeat(DETAIL_EXCERPT);

        let cut = excerpt(&long);

        assert!(cut.ends_with("more bytes)"));
        assert_eq!(
            cut.chars().filter(|c| *c == 'é').count(),
            DETAIL_EXCERPT / 2
        );
        assert!(excerpt("short").eq("short"));
    }

    /// The tools' metadata, read from the functions `#[tool]` generates
    /// beside each handler. Building the router instead would reference the
    /// handlers, and through their `AppHandle` the whole windowing stack — a
    /// test binary linking that fails to load, because it lacks the manifest
    /// the app binary gets from tauri-build.
    fn tool_metadata() -> Vec<rmcp::model::Tool> {
        vec![
            Server::project_info_tool_attr(),
            Server::list_files_tool_attr(),
            Server::read_file_tool_attr(),
            Server::write_file_tool_attr(),
            Server::read_focus_file_tool_attr(),
            Server::update_focus_tool_attr(),
            Server::add_focus_tool_attr(),
            Server::delete_focus_tool_attr(),
            Server::search_sprites_tool_attr(),
            Server::sprite_texture_tool_attr(),
            Server::read_localisation_file_tool_attr(),
            Server::write_localisation_file_tool_attr(),
            Server::read_state_file_tool_attr(),
            Server::update_state_tool_attr(),
            Server::read_country_history_tool_attr(),
            Server::update_country_history_tool_attr(),
        ]
    }

    #[test]
    fn every_tool_describes_itself_and_its_input() {
        for tool in tool_metadata() {
            assert!(
                tool.description
                    .as_deref()
                    .is_some_and(|text| !text.is_empty()),
                "{} has no description",
                tool.name
            );
            // The schema is what the assistant reads to fill the call in; a
            // tool taking no input still advertises an (empty) object.
            assert_eq!(
                tool.input_schema
                    .get("type")
                    .and_then(|value| value.as_str()),
                Some("object"),
                "{} has no object schema",
                tool.name
            );
        }
    }

    #[test]
    fn writing_tools_say_so_and_reading_tools_say_so() {
        let by_name: std::collections::HashMap<String, rmcp::model::Tool> = tool_metadata()
            .into_iter()
            .map(|tool| (tool.name.to_string(), tool))
            .collect();

        let read_only = |name: &str| {
            by_name[name]
                .annotations
                .as_ref()
                .and_then(|annotations| annotations.read_only_hint)
                .unwrap_or(false)
        };

        assert!(read_only("project_info"));
        assert!(read_only("read_focus_file"));
        assert!(read_only("search_sprites"));
        assert!(!read_only("update_focus"));
        assert!(!read_only("write_file"));
    }

    #[test]
    fn update_inputs_take_the_same_shape_the_forms_send() {
        // The nested update is the serde type the window already posts, so
        // its schema must spell the fields the same way: camelCase.
        let schema =
            serde_json::Value::Object((*Server::update_focus_tool_attr().input_schema).clone());
        let text = schema.to_string();

        assert!(text.contains("focusId"), "{text}");
        assert!(text.contains("relativePositionId"), "{text}");
        assert!(text.contains("mutuallyExclusive"), "{text}");
        assert!(!text.contains("relative_position_id"), "{text}");
    }
}
