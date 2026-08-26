'use client';

import { create } from 'zustand';

import { api, describeError } from './ipc';
import type { ModProject, ScanResult, Settings, ThemeMode } from './types';

/** Lets a reload land back in the workspace the user was in. */
const LAST_PROJECT_KEY = 'hoi4ms.last-project';

interface AppState {
  settings: Settings | null;
  project: ModProject | null;
  scan: ScanResult | null;
  isScanning: boolean;
  /** Set while the app restores the previous session on first paint. */
  isRestoring: boolean;
  error: string | null;
  /**
   * What the open workspace would lose if it were left right now, e.g.
   * `focus editor`. Null when nothing is unsaved. Pages report their own
   * state here so the shell can guard navigation.
   */
  unsavedIn: string | null;

  initialise: () => Promise<void>;
  setTheme: (theme: ThemeMode) => Promise<void>;
  setGameRoot: (path: string) => Promise<void>;
  openProject: (modFilePath: string) => Promise<ModProject | null>;
  closeProject: () => void;
  refreshScan: () => Promise<void>;
  forgetRecent: (folderPath: string) => Promise<void>;
  setError: (message: string | null) => void;
  setUnsavedIn: (label: string | null) => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  settings: null,
  project: null,
  scan: null,
  isScanning: false,
  isRestoring: true,
  error: null,
  unsavedIn: null,

  async initialise() {
    try {
      const settings = await api.loadSettings();
      set({ settings });

      const remembered = typeof window === 'undefined' ? null : window.localStorage.getItem(LAST_PROJECT_KEY);

      if (remembered) {
        // A remembered project may have been moved or deleted between runs,
        // so a failure here just drops back to the start screen.
        try {
          const project = await api.readModProject(remembered);
          set({ project });
          void get().refreshScan();
        } catch {
          window.localStorage.removeItem(LAST_PROJECT_KEY);
        }
      }
    } catch (error) {
      set({ error: describeError(error) });
    } finally {
      set({ isRestoring: false });
    }
  },

  async setTheme(theme) {
    const settings = get().settings;
    if (!settings) return;

    const updated = { ...settings, theme };
    set({ settings: updated });
    await api.saveSettings(updated);
  },

  async setGameRoot(gameRootPath) {
    const settings = get().settings;
    if (!settings) return;

    const updated = { ...settings, gameRootPath };
    set({ settings: updated });
    await api.saveSettings(updated);
  },

  async openProject(modFilePath) {
    try {
      const project = await api.openModProject(modFilePath);
      window.localStorage.setItem(LAST_PROJECT_KEY, project.modFilePath);

      set({ project, scan: null, error: null, unsavedIn: null });
      void get().refreshScan();

      // The command also refreshes the recent list on disk.
      set({ settings: await api.loadSettings() });
      return project;
    } catch (error) {
      set({ error: describeError(error) });
      return null;
    }
  },

  closeProject() {
    window.localStorage.removeItem(LAST_PROJECT_KEY);
    set({ project: null, scan: null, unsavedIn: null });
  },

  async refreshScan() {
    const folder = get().project?.folderPath;
    if (!folder) return;

    // A scan of a big mod outlives a quick close-and-open, and the result
    // carries absolute paths. Landing one against a different project would
    // point the editors at files from the mod that is no longer open.
    const isCurrent = () => get().project?.folderPath === folder;

    set({ isScanning: true });
    try {
      const scan = await api.scanProjectFiles(folder);
      if (isCurrent()) set({ scan, error: null });
    } catch (error) {
      if (isCurrent()) set({ error: describeError(error) });
    } finally {
      if (isCurrent()) set({ isScanning: false });
    }
  },

  async forgetRecent(folderPath) {
    try {
      set({ settings: await api.forgetRecentProject(folderPath) });
    } catch (error) {
      set({ error: describeError(error) });
    }
  },

  setError(message) {
    set({ error: message });
  },

  setUnsavedIn(label) {
    set({ unsavedIn: label });
  },
}));
