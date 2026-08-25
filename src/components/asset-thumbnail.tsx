"use client";

import * as React from "react";
import { FileQuestion, Loader2 } from "lucide-react";

import { api } from "@/lib/ipc";
import { cn } from "@/lib/utils";

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
  const [state, setState] = React.useState<"idle" | "loading" | "ready" | "failed">("idle");
  const [source, setSource] = React.useState<string | null>(null);

  React.useEffect(() => {
    setState("idle");
    setSource(null);
  }, [path]);

  React.useEffect(() => {
    const element = host.current;
    if (!element || state !== "idle") return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return;

        observer.disconnect();
        setState("loading");
        api
          .readImageDataUrl(path, size * 2)
          .then((url) => {
            setSource(url);
            setState("ready");
          })
          .catch(() => setState("failed"));
      },
      { rootMargin: "200px" },
    );

    observer.observe(element);
    return () => observer.disconnect();
  }, [path, size, state]);

  return (
    <div
      ref={host}
      className={cn(
        "flex items-center justify-center overflow-hidden bg-surface-sunken",
        className,
      )}
    >
      {state === "loading" ? (
        <Loader2 className="size-4 animate-spin text-border-strong" />
      ) : state === "failed" ? (
        <FileQuestion className="size-5 text-border-strong" />
      ) : source ? (
        // eslint-disable-next-line @next/next/no-img-element -- data URL produced by the backend
        <img src={source} alt="" className="max-h-full max-w-full object-contain" />
      ) : null}
    </div>
  );
}
