import type { ProjectFile } from "./types";
import { isInFolder } from "./utils";

export interface FileGroup {
  id: string;
  label: string;
  /** Folder prefixes relative to the mod root. Empty matches everything. */
  prefixes: string[];
}

/** The script areas a modder moves between most often. */
export const SCRIPT_GROUPS: FileGroup[] = [
  { id: "all", label: "All text files", prefixes: [] },
  { id: "focus", label: "National Focus", prefixes: ["common/national_focus"] },
  { id: "events", label: "Events", prefixes: ["events"] },
  { id: "history", label: "History", prefixes: ["history"] },
  { id: "ideas", label: "Ideas", prefixes: ["common/ideas"] },
  { id: "ideologies", label: "Ideologies", prefixes: ["common/ideologies"] },
  { id: "decisions", label: "Decisions", prefixes: ["common/decisions"] },
  { id: "characters", label: "Characters", prefixes: ["common/characters"] },
  { id: "countries", label: "Countries", prefixes: ["common/countries", "common/country_tags"] },
  { id: "effects", label: "Scripted Effects", prefixes: ["common/scripted_effects"] },
  { id: "triggers", label: "Scripted Triggers", prefixes: ["common/scripted_triggers"] },
  { id: "localisation", label: "Localisation", prefixes: ["localisation", "localization"] },
  { id: "interface", label: "Interface", prefixes: ["interface", "gfx"] },
];

/** Top level folders shown by the asset browsers. */
export const ASSET_AREAS: FileGroup[] = [
  { id: "gfx", label: "GFX", prefixes: ["gfx"] },
  { id: "interface", label: "Interface", prefixes: ["interface"] },
  { id: "common", label: "Common", prefixes: ["common"] },
  { id: "events", label: "Events", prefixes: ["events"] },
  { id: "history", label: "History", prefixes: ["history"] },
  { id: "map", label: "Map", prefixes: ["map"] },
  { id: "music", label: "Music", prefixes: ["music"] },
  { id: "portraits", label: "Portraits", prefixes: ["portraits"] },
  { id: "sound", label: "Sound", prefixes: ["sound"] },
  { id: "localisation", label: "Localisation", prefixes: ["localisation", "localization"] },
];

export function matchesGroup(file: ProjectFile, group: FileGroup): boolean {
  if (group.prefixes.length === 0) return true;
  return group.prefixes.some((prefix) => isInFolder(file.relativePath, prefix));
}

export function filterFiles(
  files: ProjectFile[],
  group: FileGroup,
  search: string,
): ProjectFile[] {
  const needle = search.trim().toLowerCase();

  return files.filter((file) => {
    if (!matchesGroup(file, group)) return false;
    if (!needle) return true;
    return file.relativePath.toLowerCase().includes(needle);
  });
}
