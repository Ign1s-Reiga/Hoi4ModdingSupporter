"use client";

import * as React from "react";

import { api } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/**
 * Mod thumbnails live anywhere on disk and are often `.tga` or `.dds`, so the
 * backend hands them over already converted to a data URL.
 */
export function ModThumbnail({
  path,
  name,
  className,
}: {
  path: string;
  name: string;
  className?: string;
}) {
  const [source, setSource] = React.useState<string | null>(null);

  React.useEffect(() => {
    let cancelled = false;
    setSource(null);

    if (!path) return;

    api
      .readImageDataUrl(path, 320)
      .then((url) => {
        if (!cancelled) setSource(url);
      })
      .catch(() => {
        if (!cancelled) setSource(null);
      });

    return () => {
      cancelled = true;
    };
  }, [path]);

  if (!source) {
    return (
      <div
        className={cn(
          "flex items-center justify-center bg-surface-sunken text-lg font-semibold text-border-strong",
          className,
        )}
        aria-hidden
      >
        {initials(name)}
      </div>
    );
  }

  return (
    // eslint-disable-next-line @next/next/no-img-element -- data URL from the backend, not a served asset
    <img src={source} alt="" className={cn("object-cover", className)} />
  );
}

function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[1][0]).toUpperCase();
}
