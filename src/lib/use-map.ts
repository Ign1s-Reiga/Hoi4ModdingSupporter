'use client';

import * as React from 'react';

import { api, describeError } from './ipc';
import { useAppStore } from './store';
import type { MapMode, MapSummary, ProvincePick } from './types';

interface LoadedMap {
  /** Folder and mode the render belongs to, so a stale one is never shown. */
  key: string;
  summary: MapSummary;
  source: string;
}

/**
 * Loads the province map for the open project and paints it in `mode`.
 *
 * Decoding happens once in the backend and is cached there; changing mode only
 * repaints, which is why the summary and the image are fetched together.
 */
export function useProvinceMap(mode: MapMode) {
  const project = useAppStore((state) => state.project);
  const setError = useAppStore((state) => state.setError);

  const folder = project?.folderPath ?? '';
  const key = folder ? `${folder}::${mode}` : '';

  const [loaded, setLoaded] = React.useState<LoadedMap | null>(null);
  const [failed, setFailed] = React.useState<{ key: string; message: string } | null>(null);

  React.useEffect(() => {
    if (!key) return;

    let cancelled = false;
    void (async () => {
      try {
        const summary = await api.loadMap(folder);
        const source = await api.renderMap(folder, mode);
        if (!cancelled) setLoaded({ key, summary, source });
      } catch (error) {
        if (cancelled) return;
        const message = describeError(error);
        setFailed({ key, message });
        setError(message);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [folder, key, mode, setError]);

  const current = loaded?.key === key ? loaded : null;
  const error = failed?.key === key ? failed.message : null;

  const pick = React.useCallback(
    (x: number, y: number): Promise<ProvincePick | null> => api.pickProvince(folder, x, y),
    [folder],
  );

  return {
    summary: current?.summary ?? null,
    source: current?.source ?? null,
    isLoading: Boolean(key) && !current && !error,
    error,
    pick,
  };
}
