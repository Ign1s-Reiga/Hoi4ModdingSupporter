'use client';

import * as React from 'react';

import { api, isDesktop } from '@/lib/ipc';

/**
 * Key to its text. `null` means the backend was asked and no file of that
 * language defines the key; a key absent from the map has not been answered
 * for yet.
 */
export type TextMap = ReadonlyMap<string, string | null>;

const EMPTY: TextMap = new Map();

/**
 * Resolves localisation keys to the text behind them, one batched call per
 * new set of keys, the way `useSpriteIcons` resolves sprite names.
 *
 * Answers are kept under the mod folder and language they came from, so
 * switching language asks again and switching back costs nothing. A key
 * nothing defines is kept too: an editor asks on every keystroke while a key
 * is being typed.
 */
export function useLocalisedTexts(folderPath: string | undefined, language: string, keys: readonly string[]): TextMap {
  const [resolved, setResolved] = React.useState<ReadonlyMap<string, string | null>>(EMPTY);
  const asked = React.useRef(new Set<string>());

  const wanted = React.useMemo(
    () => [...new Set(keys.map((key) => key.trim()).filter(Boolean))].sort().join('\n'),
    [keys],
  );

  React.useEffect(() => {
    if (!folderPath || !language || !isDesktop()) return;

    const missing = wanted.split('\n').filter((key) => key && !asked.current.has(cacheKey(folderPath, language, key)));
    if (missing.length === 0) return;

    for (const key of missing) asked.current.add(cacheKey(folderPath, language, key));

    let cancelled = false;
    let answered = false;
    const forget = () => {
      for (const key of missing) asked.current.delete(cacheKey(folderPath, language, key));
    };

    api
      .localisedTexts(folderPath, language, missing)
      .then((texts) => {
        answered = true;
        if (cancelled) return;

        setResolved((current) => {
          const next = new Map(current);
          for (const key of missing) next.set(cacheKey(folderPath, language, key), texts[key] ?? null);
          return next;
        });
      })
      .catch(() => {
        // A failed lookup shows the keys themselves; forgetting them lets a
        // later render retry.
        forget();
      });

    return () => {
      cancelled = true;
      if (!answered) forget();
    };
  }, [folderPath, language, wanted]);

  return React.useMemo(() => {
    if (!folderPath) return EMPTY;

    const view = new Map<string, string | null>();
    for (const key of wanted.split('\n')) {
      const text = resolved.get(cacheKey(folderPath, language, key));
      if (text !== undefined) view.set(key, text);
    }
    return view;
  }, [folderPath, language, resolved, wanted]);
}

function cacheKey(folderPath: string, language: string, key: string): string {
  // NUL cannot appear in a path, a language or a key, so this is unambiguous.
  return `${folderPath}\0${language}\0${key}`;
}
