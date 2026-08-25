"use client";

import * as React from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { ExternalLink, Images, Loader2, Search } from "lucide-react";

import { AssetThumbnail } from "@/components/asset-thumbnail";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/form";
import {
  Badge,
  EmptyState,
  ListRow,
  Panel,
  PanelBody,
  PanelHeader,
} from "@/components/ui/panel";
import { ASSET_AREAS } from "@/lib/groups";
import { api, describeError } from "@/lib/ipc";
import { useAppStore } from "@/lib/store";
import type { ProjectFile } from "@/lib/types";
import { basename, formatBytes, isInFolder } from "@/lib/utils";

const MAX_TILES = 300;

type Source = "mod" | "game";

export default function AssetsPage() {
  const { project, scan, settings, setError } = useAppStore();

  const [source, setSource] = React.useState<Source>("mod");
  const [areaId, setAreaId] = React.useState(ASSET_AREAS[0].id);
  const [search, setSearch] = React.useState("");
  const [gameScan, setGameScan] = React.useState<{
    key: string;
    files: ProjectFile[];
    truncated: boolean;
  } | null>(null);
  const [selected, setSelected] = React.useState<ProjectFile | null>(null);
  const [preview, setPreview] = React.useState<{ path: string; url: string | null } | null>(
    null,
  );

  const gameRoot = settings?.gameRootPath ?? "";
  const area = ASSET_AREAS.find((entry) => entry.id === areaId) ?? ASSET_AREAS[0];

  // Game folders are scanned per area, on demand. Each result is kept together
  // with the folder it came from, so switching areas falls back to the loading
  // state by itself instead of being reset from an effect.
  const gameScanKey =
    source === "game" && gameRoot
      ? `${gameRoot.replace(/[\\/]+$/, "")}/${area.prefixes[0]}`
      : null;

  React.useEffect(() => {
    if (!gameScanKey) return;

    let cancelled = false;
    api
      .scanDirectory(gameScanKey)
      .then((result) => {
        if (cancelled) return;
        setGameScan({ key: gameScanKey, files: result.files, truncated: result.truncated });
      })
      .catch((error) => {
        if (cancelled) return;
        setGameScan({ key: gameScanKey, files: [], truncated: false });
        setError(describeError(error));
      });

    return () => {
      cancelled = true;
    };
  }, [gameScanKey, setError]);

  const currentScan = gameScan?.key === gameScanKey ? gameScan : null;
  const isLoadingGame = Boolean(gameScanKey) && !currentScan;
  const gameTruncated = currentScan?.truncated ?? false;
  const gameFiles = React.useMemo(() => currentScan?.files ?? [], [currentScan]);

  const files = React.useMemo(() => {
    const pool =
      source === "mod"
        ? (scan?.files ?? []).filter((file) =>
            area.prefixes.some((prefix) => isInFolder(file.relativePath, prefix)),
          )
        : gameFiles;

    const needle = search.trim().toLowerCase();
    return needle
      ? pool.filter((file) => file.relativePath.toLowerCase().includes(needle))
      : pool;
  }, [area, gameFiles, scan, search, source]);

  const images = files.filter((file) => file.kind === "image");
  const others = files.filter((file) => file.kind !== "image");

  React.useEffect(() => {
    if (!selected || selected.kind !== "image") return;

    const path = selected.fullPath;
    let cancelled = false;
    api
      .readImageDataUrl(path, 1024)
      .then((url) => {
        if (!cancelled) setPreview({ path, url });
      })
      .catch(() => {
        if (!cancelled) setPreview({ path, url: null });
      });

    return () => {
      cancelled = true;
    };
  }, [selected]);

  const currentPreview = selected && preview?.path === selected.fullPath ? preview : null;

  return (
    <div className="grid h-full grid-cols-[13rem_minmax(0,1fr)_18rem] gap-3 p-3">
      <Panel>
        <PanelHeader title="Areas" subtitle={source === "mod" ? project?.name : "Game files"} />
        <div className="flex gap-1 border-b border-border p-2">
          <Button
            variant={source === "mod" ? "primary" : "ghost"}
            size="sm"
            className="flex-1"
            onClick={() => setSource("mod")}
          >
            Mod
          </Button>
          <Button
            variant={source === "game" ? "primary" : "ghost"}
            size="sm"
            className="flex-1"
            onClick={() => setSource("game")}
            disabled={!gameRoot}
            title={gameRoot ? undefined : "Set the game folder in Settings"}
          >
            Game
          </Button>
        </div>
        <PanelBody>
          <ul className="py-1">
            {ASSET_AREAS.map((entry) => (
              <li key={entry.id}>
                <ListRow active={entry.id === areaId} onClick={() => setAreaId(entry.id)}>
                  <span className="truncate">{entry.label}</span>
                </ListRow>
              </li>
            ))}
          </ul>
        </PanelBody>
      </Panel>

      <Panel>
        <PanelHeader
          title={area.label}
          subtitle={`${images.length} images · ${others.length} other files`}
          actions={gameTruncated && source === "game" ? <Badge tone="danger">Partial listing</Badge> : null}
        />
        <div className="border-b border-border p-2">
          <div className="relative">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted" />
            <Input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Filter by path"
              className="pl-8"
            />
          </div>
        </div>
        <PanelBody className="p-3">
          {source === "game" && !gameRoot ? (
            <EmptyState
              icon={<Images />}
              title="Game folder not set"
              description="Point Settings at your Hearts of Iron IV installation to browse its art and scripts alongside the mod."
            />
          ) : isLoadingGame ? (
            <div className="flex h-full items-center justify-center gap-2 text-sm text-muted">
              <Loader2 className="size-4 animate-spin" />
              Reading {area.label}…
            </div>
          ) : files.length === 0 ? (
            <EmptyState title="Nothing here" description={`No files under ${area.prefixes[0]}.`} />
          ) : (
            <>
              {images.length > 0 ? (
                <div className="grid grid-cols-[repeat(auto-fill,minmax(7rem,1fr))] gap-2">
                  {images.slice(0, MAX_TILES).map((file) => (
                    <button
                      key={file.fullPath}
                      type="button"
                      onClick={() => setSelected(file)}
                      className={`flex flex-col overflow-hidden rounded-md border text-left transition-colors ${
                        selected?.fullPath === file.fullPath
                          ? "border-accent"
                          : "border-border hover:border-border-strong"
                      }`}
                      title={file.relativePath}
                    >
                      <AssetThumbnail path={file.fullPath} className="h-24 w-full" />
                      <span className="truncate px-1.5 py-1 text-[0.6875rem] text-muted">
                        {file.name}
                      </span>
                    </button>
                  ))}
                </div>
              ) : null}

              {others.length > 0 ? (
                <ul className="mt-3 space-y-px">
                  {others.slice(0, MAX_TILES).map((file) => (
                    <li key={file.fullPath}>
                      <ListRow
                        active={selected?.fullPath === file.fullPath}
                        onClick={() => setSelected(file)}
                        title={file.relativePath}
                      >
                        <span className="truncate">{file.relativePath}</span>
                        <span className="ml-auto shrink-0 text-[0.6875rem] text-muted">
                          {formatBytes(file.sizeBytes)}
                        </span>
                      </ListRow>
                    </li>
                  ))}
                </ul>
              ) : null}

              {files.length > MAX_TILES ? (
                <p className="mt-3 text-xs text-muted">
                  Showing the first {MAX_TILES} entries of {files.length}. Narrow the filter to
                  see more.
                </p>
              ) : null}
            </>
          )}
        </PanelBody>
      </Panel>

      <Panel>
        <PanelHeader title="Preview" subtitle={selected ? basename(selected.fullPath) : undefined} />
        <PanelBody className="p-3">
          {!selected ? (
            <EmptyState title="No file selected" description="Pick an asset to inspect it." />
          ) : (
            <div className="flex flex-col gap-3">
              {selected.kind === "image" ? (
                <div className="flex min-h-40 items-center justify-center rounded-md border border-border bg-surface-sunken p-2">
                  {currentPreview?.url ? (
                    // eslint-disable-next-line @next/next/no-img-element -- data URL from the backend
                    <img
                      src={currentPreview.url}
                      alt=""
                      className="max-h-64 max-w-full object-contain"
                    />
                  ) : currentPreview ? (
                    <span className="text-xs text-muted">Preview not available</span>
                  ) : (
                    <Loader2 className="size-4 animate-spin text-border-strong" />
                  )}
                </div>
              ) : null}

              <dl className="space-y-2 text-xs">
                <div>
                  <dt className="text-muted">Path</dt>
                  <dd className="break-all font-mono text-[0.6875rem]" data-selectable>
                    {selected.relativePath}
                  </dd>
                </div>
                <div className="flex gap-4">
                  <div>
                    <dt className="text-muted">Size</dt>
                    <dd>{formatBytes(selected.sizeBytes)}</dd>
                  </div>
                  <div>
                    <dt className="text-muted">Type</dt>
                    <dd>{selected.extension || "none"}</dd>
                  </div>
                </div>
              </dl>

              <Button
                size="sm"
                onClick={() => {
                  void revealItemInDir(selected.fullPath).catch((error) =>
                    setError(describeError(error)),
                  );
                }}
              >
                <ExternalLink />
                Show in Explorer
              </Button>
            </div>
          )}
        </PanelBody>
      </Panel>
    </div>
  );
}
