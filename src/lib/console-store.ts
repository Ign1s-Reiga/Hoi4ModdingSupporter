'use client';

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { create } from 'zustand';

import { api, describeError, isDesktop } from '@/lib/ipc';
import type { ConsoleEntry, ConsoleLevel, ConsoleSource } from '@/lib/types';

/** Matches the backend's ring buffer, so the panel never holds more than it would send. */
const CAPACITY = 2000;

const ENTRY_EVENT = 'console://entry';
const CLEARED_EVENT = 'console://cleared';

interface ConsoleState {
  entries: ConsoleEntry[];
  isOpen: boolean;
  /** Errors logged while the panel was closed, cleared when it opens. */
  unseenErrors: number;
  sources: Record<ConsoleSource, boolean>;
  levels: Record<ConsoleLevel, boolean>;
  /** Id of the entry whose detail is expanded, if any. */
  expandedId: number | null;

  /** Loads what was logged before the window mounted and follows new entries. */
  initialise: () => Promise<void>;
  toggle: () => void;
  setSource: (source: ConsoleSource, on: boolean) => void;
  setLevel: (level: ConsoleLevel, on: boolean) => void;
  expand: (id: number | null) => void;
  clear: () => Promise<void>;
}

let subscribed: Promise<UnlistenFn[]> | null = null;

export const useConsoleStore = create<ConsoleState>((set) => ({
  entries: [],
  isOpen: false,
  unseenErrors: 0,
  sources: { app: true, files: true, mcp: true },
  levels: { info: true, warn: true, error: true },
  expandedId: null,

  async initialise() {
    if (!isDesktop() || subscribed) return;

    // Subscribed once for the life of the window: the shell calls this on
    // every mount and a second listener would double every entry.
    subscribed = Promise.all([
      listen<ConsoleEntry>(ENTRY_EVENT, (event) => {
        set((state) => {
          const entries = [...state.entries, event.payload];
          if (entries.length > CAPACITY) entries.splice(0, entries.length - CAPACITY);

          const isUnseenError = event.payload.level === 'error' && !state.isOpen;
          return { entries, unseenErrors: state.unseenErrors + (isUnseenError ? 1 : 0) };
        });
      }),
      listen(CLEARED_EVENT, () => set({ entries: [], expandedId: null })),
    ]);

    try {
      // Anything logged before the listener attached — the server starting,
      // the project restoring — is fetched after, so nothing is missed. An
      // entry that arrives in between appears in both; the id dedupes it.
      const initial = await api.consoleEntries();
      set((state) => {
        const seen = new Set(initial.map((entry) => entry.id));
        const late = state.entries.filter((entry) => !seen.has(entry.id));
        return { entries: [...initial, ...late].slice(-CAPACITY) };
      });
    } catch (error) {
      // The panel is a convenience; a failed load leaves it empty, not broken.
      console.error(describeError(error));
    }
  },

  toggle() {
    set((state) => ({ isOpen: !state.isOpen, unseenErrors: state.isOpen ? state.unseenErrors : 0 }));
  },

  setSource(source, on) {
    set((state) => ({ sources: { ...state.sources, [source]: on } }));
  },

  setLevel(level, on) {
    set((state) => ({ levels: { ...state.levels, [level]: on } }));
  },

  expand(id) {
    set({ expandedId: id });
  },

  async clear() {
    try {
      await api.consoleClear();
      // The cleared event follows, but the panel should not wait for it.
      set({ entries: [], expandedId: null, unseenErrors: 0 });
    } catch (error) {
      console.error(describeError(error));
    }
  },
}));

/** The entries the current filters let through. */
export function visibleEntries(state: Pick<ConsoleState, 'entries' | 'sources' | 'levels'>): ConsoleEntry[] {
  return state.entries.filter((entry) => state.sources[entry.source] && state.levels[entry.level]);
}
