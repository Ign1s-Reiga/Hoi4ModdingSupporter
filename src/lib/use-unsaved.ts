'use client';

import * as React from 'react';

import { useAppStore } from './store';

/**
 * Reports a workspace page's unsaved state to the shell.
 *
 * Each editor keeps its draft in component state, which React throws away the
 * moment a route change unmounts it. Publishing the state here lets the shell
 * ask before that happens, and clearing it on unmount keeps a page that has
 * already gone from blocking the next one.
 */
export function useUnsavedIn(label: string, isDirty: boolean): void {
  const setUnsavedIn = useAppStore((state) => state.setUnsavedIn);

  React.useEffect(() => {
    setUnsavedIn(isDirty ? label : null);
    return () => setUnsavedIn(null);
  }, [isDirty, label, setUnsavedIn]);
}
