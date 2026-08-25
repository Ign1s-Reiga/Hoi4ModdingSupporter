"use client";

import * as React from "react";
import { FileQuestion, Loader2 } from "lucide-react";

import { api } from "@/lib/ipc";
import { cn } from "@/lib/utils";

type Loaded = { path: string; url: string | null };

/**
 * A game asset preview that only decodes once it scrolls into view — a `gfx`
 * folder holds thousands of `.dds` files and converting them all up front
 * would lock the window.
 */
export function AssetThumbnail({
  path,
  className,
  size = 128,
}: {
  path: string;
  className?: string;
  size?: number;
}) {
  const host = React.useRef<HTMLDivElement>(null);
  const [visible, setVisible] = React.useState<string | null>(null);
  const [loaded, setLoaded] = React.useState<Loaded | null>(null);

  React.useEffect(() => {
    const element = host.current;
    if (!element) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return;
        observer.disconnect();
        setVisible(path);
      },
      { rootMargin: "200px" },
    );

    observer.observe(element);
    return () => observer.disconnect();
  }, [path]);

  React.useEffect(() => {
    if (visible !== path) return;

    let cancelled = false;
    api
      .readImageDataUrl(path, size * 2)
      .then((url) => {
        if (!cancelled) setLoaded({ path, url });
      })
      .catch(() => {
        if (!cancelled) setLoaded({ path, url: null });
      });

    return () => {
      cancelled = true;
    };
  }, [path, size, visible]);

  const result = loaded?.path === path ? loaded : null;
  const isLoading = visible === path && !result;

  return (
    <div
      ref={host}
      className={cn(
        "flex items-center justify-center overflow-hidden bg-surface-sunken",
        className,
      )}
    >
      {isLoading ? (
        <Loader2 className="size-4 animate-spin text-border-strong" />
      ) : result?.url ? (
        // eslint-disable-next-line @next/next/no-img-element -- data URL produced by the backend
        <img src={result.url} alt="" className="max-h-full max-w-full object-contain" />
      ) : result ? (
        <FileQuestion className="size-5 text-border-strong" />
      ) : null}
    </div>
  );
}
