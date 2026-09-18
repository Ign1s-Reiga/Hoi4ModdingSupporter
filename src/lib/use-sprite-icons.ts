'use client';

import * as React from 'react';

import { api, isDesktop } from '@/lib/ipc';
import type { SpriteIcon } from '@/lib/types';

/** Sprite name to what it resolved to. A name absent from the map is one the
 * backend has not answered for yet. */
export type SpriteMap = ReadonlyMap<string, SpriteIcon>;

const EMPTY: ReadonlyMap<string, SpriteIcon> = new Map();

/**
 * Resolves `GFX_...` names to pictures, one batched call per new set of names.
 *
 * Everything already answered for is kept, so editing a sprite name asks only
 * about the name being typed and the rest of the tree keeps its art. Entries
 * are stored under the mod folder they were resolved in: opening a second mod
 * can then never draw the first one's icons, even for a sprite both define.
 */
export function useSpriteIcons(folderPath: string | undefined, names: readonly string[]): SpriteMap {
  const [resolved, setResolved] = React.useState(EMPTY);
  // Names already sent. A render while a request is in flight must not ask
  // for the same sprites a second time.
  const asked = React.useRef(new Set<string>());

  // A stable key, so the effect ignores an array holding the same names.
  const wanted = React.useMemo(
    () => [...new Set(names.map((name) => name.trim()).filter(Boolean))].sort().join('\n'),
    [names],
  );

  React.useEffect(() => {
    if (!folderPath || !isDesktop()) return;

    const missing = wanted.split('\n').filter((name) => name && !asked.current.has(cacheKey(folderPath, name)));
    if (missing.length === 0) return;

    for (const name of missing) asked.current.add(cacheKey(folderPath, name));

    let cancelled = false;
    let answered = false;
    const forget = () => {
      for (const name of missing) asked.current.delete(cacheKey(folderPath, name));
    };

    api
      .spriteIcons(folderPath, missing)
      .then((icons) => {
        answered = true;
        if (cancelled) return;

        setResolved((current) => {
          const next = new Map(current);
          for (const icon of icons) next.set(cacheKey(folderPath, icon.name), icon);
          return next;
        });
      })
      .catch(() => {
        // Missing art is not worth an error banner over the editor: the nodes
        // fall back to a blank icon. Forgetting the names lets a later render
        // retry, which is what a repointed game folder needs.
        forget();
      });

    return () => {
      cancelled = true;
      // An answer that arrived is already in `resolved`; one that did not must
      // not leave its names looking asked for.
      if (!answered) forget();
    };
  }, [folderPath, wanted]);

  return React.useMemo(() => {
    if (!folderPath) return EMPTY;

    const view = new Map<string, SpriteIcon>();
    for (const name of wanted.split('\n')) {
      const icon = resolved.get(cacheKey(folderPath, name));
      if (icon) view.set(name, icon);
    }
    return view;
  }, [folderPath, resolved, wanted]);
}

function cacheKey(folderPath: string, name: string): string {
  // NUL cannot appear in a path or a sprite name, so the key is unambiguous.
  return `${folderPath}\0${name}`;
}
