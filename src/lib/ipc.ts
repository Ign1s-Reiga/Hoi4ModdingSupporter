/**
 * Typed wrappers around the Rust commands.
 *
 * Every backend call goes through here so the argument names stay in one place
 * and a browser-only preview (plain `next dev`, no Tauri window) fails with a
 * readable message instead of an opaque bridge error.
 */

import { invoke } from '@tauri-apps/api/core';

import type {
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
  ModProject,
  ProvincePick,
  ScanResult,
  Settings,
  StateFile,
  StateUpdate,
  TextFile,
} from './types';

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

  updateFocus: (path: string, focusId: string, update: FocusUpdate) =>
    call<FocusFile>('update_focus', { path, focusId, update }),

  addFocus: (path: string, treeId: string, focus: FocusUpdate) => call<FocusFile>('add_focus', { path, treeId, focus }),

  deleteFocus: (path: string, focusId: string) => call<FocusFile>('delete_focus', { path, focusId }),

  /** Decodes provinces.bmp and joins it to the state files. Cached backend side. */
  loadMap: (folderPath: string) => call<MapSummary>('load_map', { folderPath }),

  renderMap: (folderPath: string, mode: MapMode) => call<string>('render_map', { folderPath, mode }),

  pickProvince: (folderPath: string, x: number, y: number) =>
    call<ProvincePick | null>('pick_province', { folderPath, x, y }),

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

  pathExists: (path: string) => call<boolean>('path_exists', { path }),

  validateGameRoot: (path: string) => call<boolean>('validate_game_root', { path }),
};

/** Turns a rejected command into the message it carries. */
export function describeError(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return 'Something went wrong.';
}
