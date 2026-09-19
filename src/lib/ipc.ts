/**
 * Typed wrappers around the Rust commands.
 *
 * Every backend call goes through here so the argument names stay in one place
 * and a browser-only preview (plain `next dev`, no Tauri window) fails with a
 * readable message instead of an opaque bridge error.
 */

import { invoke } from '@tauri-apps/api/core';

import type {
  ConsoleEntry,
  CountryHistory,
  CountryHistoryInfo,
  CountryHistoryUpdate,
  Encoding,
  FocusFile,
  FocusUpdate,
  LocalisationEntry,
  LocalisationFile,
  LocalisationFileInfo,
  MapMode,
  MapSummary,
  McpStatus,
  ModProject,
  ProvincePick,
  ScanResult,
  Settings,
  SpriteIcon,
  StateFile,
  StateUpdate,
  TextFile,
} from '@/lib/types';

export function isDesktop(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop()) {
    throw new Error(
      'This view needs the desktop window. Run `pnpm desktop` instead of opening the dev server in a browser.',
    );
  }

  return invoke<T>(command, args);
}

export const api = {
  loadSettings: () => call<Settings>('load_settings'),

  saveSettings: (settings: Settings) => call<Settings>('save_settings', { settings }),

  /**
   * Tells the backend which project the window has open, so the MCP server
   * works on the same one. Null when the project is closed.
   */
  setOpenProject: (project: ModProject | null) => call<void>('set_open_project', { project }),

  /** Everything logged since the app started, oldest first. */
  consoleEntries: () => call<ConsoleEntry[]>('console_entries'),

  consoleClear: () => call<void>('console_clear'),

  mcpStatus: () => call<McpStatus>('mcp_status'),

  /** Mints a new token and restarts the server, cutting off clients holding the old one. */
  mcpRegenerateToken: () => call<McpStatus>('mcp_regenerate_token'),

  /** Reads a descriptor and moves the project to the top of the recent list. */
  openModProject: (modFilePath: string) => call<ModProject>('open_mod_project', { modFilePath }),

  readModProject: (modFilePath: string) => call<ModProject>('read_mod_project', { modFilePath }),

  forgetRecentProject: (folderPath: string) => call<Settings>('forget_recent_project', { folderPath }),

  scanProjectFiles: (folderPath: string) => call<ScanResult>('scan_project_files', { folderPath }),

  scanDirectory: (rootPath: string, maxFiles?: number) => call<ScanResult>('scan_directory', { rootPath, maxFiles }),

  assetAreas: () => call<[string, string][]>('asset_areas'),

  readTextFile: (path: string) => call<TextFile>('read_text_file', { path }),

  /** Returns the encoding actually written, which can differ when the text no
   * longer fits the original one. */
  writeTextFile: (path: string, content: string, encoding: Encoding, withBom: boolean) =>
    call<Encoding>('write_text_file', { path, content, encoding, withBom }),

  readFocusFile: (path: string) => call<FocusFile>('read_focus_file', { path }),

  /**
   * The same operations on text the window holds rather than on the file, for
   * the code view: the tree follows the buffer, and the file is written when
   * the user asks. `path` only names the file in error messages.
   */
  parseFocusSource: (path: string, source: string, hasBom: boolean, encoding: Encoding) =>
    call<FocusFile>('parse_focus_source', { path, source, hasBom, encoding }),

  updateFocusSource: (
    path: string,
    source: string,
    hasBom: boolean,
    encoding: Encoding,
    focusId: string,
    update: FocusUpdate,
  ) => call<FocusFile>('update_focus_source', { path, source, hasBom, encoding, focusId, update }),

  addFocusSource: (
    path: string,
    source: string,
    hasBom: boolean,
    encoding: Encoding,
    treeId: string,
    focus: FocusUpdate,
  ) => call<FocusFile>('add_focus_source', { path, source, hasBom, encoding, treeId, focus }),

  deleteFocusSource: (path: string, source: string, hasBom: boolean, encoding: Encoding, focusId: string) =>
    call<FocusFile>('delete_focus_source', { path, source, hasBom, encoding, focusId }),

  updateFocus: (path: string, focusId: string, update: FocusUpdate) =>
    call<FocusFile>('update_focus', { path, focusId, update }),

  addFocus: (path: string, treeId: string, focus: FocusUpdate) => call<FocusFile>('add_focus', { path, treeId, focus }),

  deleteFocus: (path: string, focusId: string) => call<FocusFile>('delete_focus', { path, focusId }),

  /**
   * Decodes provinces.bmp and joins it to the state files. Cached backend side.
   *
   * `replacePaths` comes from the descriptor: without it the base game's states
   * are loaded underneath the mod's, which is what the game does for any
   * directory the mod has not replaced.
   */
  loadMap: (folderPath: string, replacePaths: string[]) => call<MapSummary>('load_map', { folderPath, replacePaths }),

  renderMap: (folderPath: string, replacePaths: string[], mode: MapMode) =>
    call<string>('render_map', { folderPath, replacePaths, mode }),

  pickProvince: (folderPath: string, replacePaths: string[], x: number, y: number) =>
    call<ProvincePick | null>('pick_province', { folderPath, replacePaths, x, y }),

  readStateFile: (path: string) => call<StateFile>('read_state_file', { path }),

  updateState: (path: string, stateId: string, update: StateUpdate) =>
    call<StateFile>('update_state', { path, stateId, update }),

  listCountryHistory: (folderPath: string) => call<CountryHistoryInfo[]>('list_country_history', { folderPath }),

  readCountryHistory: (path: string) => call<CountryHistory>('read_country_history', { path }),

  updateCountryHistory: (path: string, update: CountryHistoryUpdate) =>
    call<CountryHistory>('update_country_history', { path, update }),

  listLocalisationFiles: (folderPath: string) =>
    call<LocalisationFileInfo[]>('list_localisation_files', { folderPath }),

  readLocalisationFile: (path: string) => call<LocalisationFile>('read_localisation_file', { path }),

  writeLocalisationFile: (path: string, language: string, entries: LocalisationEntry[]) =>
    call<LocalisationFile>('write_localisation_file', { path, language, entries }),

  /** Base64 data URL, converting `.tga`/`.dds` to PNG on the way. */
  readImageDataUrl: (path: string, maxDimension?: number) =>
    call<string>('read_image_data_url', { path, maxDimension }),

  /**
   * Resolves sprite names against the mod's `interface/*.gfx` and the game's.
   *
   * Asked in batches: the backend keeps the index and the decoded pictures, so
   * one call for a whole focus tree costs a great deal less than one per node.
   */
  spriteIcons: (folderPath: string, names: string[]) => call<SpriteIcon[]>('sprite_icons', { folderPath, names }),

  pathExists: (path: string) => call<boolean>('path_exists', { path }),

  validateGameRoot: (path: string) => call<boolean>('validate_game_root', { path }),
};

/** Turns a rejected command into the message it carries. */
export function describeError(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return 'Something went wrong.';
}
