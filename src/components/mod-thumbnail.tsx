'use client';

import * as React from 'react';

import { api } from '@/lib/ipc';
import { cn } from '@/lib/utils';

/**
 * Mod thumbnails live anywhere on disk and are often `.tga` or `.dds`, so the
 * backend hands them over already converted to a data URL.
 */
export function ModThumbnail({ path, name, className }: { path: string; name: string; className?: string }) {
  // Loaded art is stored with the path it came from, so a changed path shows
  // the placeholder again without a state reset on every render.
  const [loaded, setLoaded] = React.useState<{ path: string; url: string } | null>(null);

  React.useEffect(() => {
    if (!path) return;

    let cancelled = false;
    api
      .readImageDataUrl(path, 320)
      .then((url) => {
        if (!cancelled) setLoaded({ path, url });
      })
      .catch(() => {
        // A missing or unreadable thumbnail just falls back to the initials.
      });

    return () => {
      cancelled = true;
    };
  }, [path]);

  const source = loaded?.path === path ? loaded.url : null;

  if (!source) {
    return (
      <div
        className={cn(
          'flex items-center justify-center bg-surface-sunken text-lg font-semibold text-border-strong',
          className,
        )}
        aria-hidden
      >
        {initials(name)}
      </div>
    );
  }

  return (
    // Data URL from the backend, not a served asset, so next/image cannot help here.
    // oxlint-disable-next-line next/no-img-element
    <img src={source} alt='' className={cn('object-cover', className)} />
  );
}

function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return '?';
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[1][0]).toUpperCase();
}
