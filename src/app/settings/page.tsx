"use client";

import * as React from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, FolderSearch, Loader2, Monitor, Moon, Sun, TriangleAlert } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/form";
import { Panel, PanelBody, PanelHeader } from "@/components/ui/panel";
import { api, describeError, isDesktop } from "@/lib/ipc";
import { useAppStore } from "@/lib/store";
import type { ThemeMode } from "@/lib/types";

const THEMES: Array<{ value: ThemeMode; label: string; icon: React.ComponentType<{ className?: string }> }> = [
  { value: "system", label: "System", icon: Monitor },
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
];

export default function SettingsPage() {
  const { settings, setTheme, setGameRoot, setError } = useAppStore();

  const [gamePath, setGamePath] = React.useState("");
  const [checking, setChecking] = React.useState(false);
  const [valid, setValid] = React.useState<boolean | null>(null);

  React.useEffect(() => {
    setGamePath(settings?.gameRootPath ?? "");
  }, [settings?.gameRootPath]);

  async function browse() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select the Hearts of Iron IV folder",
      });
      if (typeof selected === "string") {
        setGamePath(selected);
        await apply(selected);
      }
    } catch (error) {
      setError(describeError(error));
    }
  }

  async function apply(path: string) {
    setChecking(true);
    try {
      const looksRight = path ? await api.validateGameRoot(path) : true;
      setValid(looksRight);
      await setGameRoot(path);
      toast.success(
        looksRight ? "Game folder saved" : "Saved, but this folder has no common/ or gfx/",
      );
    } catch (error) {
      setValid(false);
      setError(describeError(error));
    } finally {
      setChecking(false);
    }
  }

  if (!isDesktop()) {
    return (
      <div className="p-8 text-sm text-muted">
        Settings are stored by the desktop backend. Start the app with <code>pnpm desktop</code>.
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto">
      <div className="mx-auto flex max-w-2xl flex-col gap-4 px-8 py-8">
        <h1 className="text-lg font-semibold tracking-tight">Settings</h1>

        <Panel>
          <PanelHeader title="Appearance" subtitle="How the window follows your desktop theme" />
          <PanelBody className="p-4">
            <div className="flex gap-2">
              {THEMES.map((option) => {
                const Icon = option.icon;
                const active = (settings?.theme ?? "system") === option.value;

                return (
                  <Button
                    key={option.value}
                    variant={active ? "primary" : "default"}
                    onClick={() => void setTheme(option.value)}
                  >
                    <Icon />
                    {option.label}
                  </Button>
                );
              })}
            </div>
          </PanelBody>
        </Panel>

        <Panel>
          <PanelHeader
            title="Hearts of Iron IV folder"
            subtitle="Needed to browse the game's own art and scripts"
          />
          <PanelBody className="flex flex-col gap-3 p-4">
            <div className="flex gap-2">
              <Input
                value={gamePath}
                onChange={(event) => {
                  setGamePath(event.target.value);
                  setValid(null);
                }}
                placeholder="C:/Program Files (x86)/Steam/steamapps/common/Hearts of Iron IV"
                className="font-mono text-xs"
              />
              <Button onClick={() => void browse()} title="Browse">
                <FolderSearch />
              </Button>
              <Button
                variant="primary"
                onClick={() => void apply(gamePath)}
                disabled={checking || gamePath === (settings?.gameRootPath ?? "")}
              >
                {checking ? <Loader2 className="animate-spin" /> : null}
                Save
              </Button>
            </div>

            {valid === true ? (
              <p className="flex items-center gap-1.5 text-xs text-success">
                <Check className="size-3.5" />
                Found common/ and gfx/ — this looks like a game install.
              </p>
            ) : valid === false ? (
              <p className="flex items-center gap-1.5 text-xs text-warning">
                <TriangleAlert className="size-3.5" />
                No common/ or gfx/ in that folder. Asset browsing may come up empty.
              </p>
            ) : (
              <p className="text-xs text-muted">
                Usually inside your Steam library under steamapps/common.
              </p>
            )}
          </PanelBody>
        </Panel>

        <Panel>
          <PanelHeader title="About" />
          <PanelBody className="space-y-1 p-4 text-xs text-muted">
            <p>
              Hoi4 Modding Supporter — a Tauri and Next.js rebuild of the original WinUI 3 tool.
            </p>
            <p data-selectable>
              Settings live in your local app data folder as settings.json.
            </p>
          </PanelBody>
        </Panel>
      </div>
    </div>
  );
}
