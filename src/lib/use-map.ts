'use client';

import * as React from 'react';

import { api, describeError } from './ipc';
import { useAppStore } from './store';
import type { MapMode, MapSummary, ProvincePick } from './types';

interface LoadedMap {
  /** Folder, mode and generation the render belongs to, so a stale one is
   * never shown. */
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
  // A stable identity, so the effect does not re-run on every render of a
  // parent that rebuilds the project object.
  const replacePaths = React.useMemo(() => project?.replacePaths ?? [], [project?.replacePaths]);

  // Bumped to force a reload after something that changes what the map shows,
  // such as saving a state's owner.
  const [generation, setGeneration] = React.useState(0);

  const [loaded, setLoaded] = React.useState<LoadedMap | null>(null);
  const [failed, setFailed] = React.useState<{ key: string; message: string } | null>(null);

  const key = folder ? `${folder}::${mode}::${generation}` : '';

  React.useEffect(() => {
    if (!key) return;

    let cancelled = false;
    void (async () => {
      try {
        const summary = await api.loadMap(folder, replacePaths);
        const source = await api.renderMap(folder, replacePaths, mode);
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
  }, [folder, key, mode, replacePaths, setError]);

  const current = loaded?.key === key ? loaded : null;
  const error = failed?.key === key ? failed.message : null;

  const pick = React.useCallback(
    (x: number, y: number): Promise<ProvincePick | null> => api.pickProvince(folder, replacePaths, x, y),
    [folder, replacePaths],
  );

  const reload = React.useCallback(() => setGeneration((value) => value + 1), []);

  return {
    summary: current?.summary ?? null,
    source: current?.source ?? null,
    isLoading: Boolean(key) && !current && !error,
    error,
    pick,
    reload,
  };
}
