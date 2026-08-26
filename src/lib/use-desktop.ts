'use client';

import * as React from 'react';

import { isDesktop } from './ipc';

/** The bridge is either there or it is not; it never appears later. */
const subscribe = () => () => {};

/**
 * Whether the app is running inside the Tauri window.
 *
 * Reading the bridge during render would make the prerendered HTML disagree
 * with the first client render, which React reports as a hydration mismatch
 * and repairs by throwing the tree away. `useSyncExternalStore` keeps the two
 * in step instead: prerendering assumes the desktop window — the only place
 * the app actually ships — so the exported HTML matches what the window
 * renders, and a browser preview corrects itself once hydrated.
 */
export function useIsDesktop(): boolean {
  return React.useSyncExternalStore(subscribe, isDesktop, () => true);
}
