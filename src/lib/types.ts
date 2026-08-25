/**
 * Mirrors of the serde types exposed by the Rust backend. Keep the field names
 * in step with `src-tauri/src` — everything crosses the bridge as camelCase.
 */

export type FileKind = "text" | "image" | "binary";

export type ThemeMode = "system" | "light" | "dark";

/** How a file is stored on disk. Legacy scripts are sometimes Windows-1252. */
export type Encoding = "utf8" | "windows1252";

export interface ProjectFile {
  fullPath: string;
  /** Path relative to the scan root, always with forward slashes. */
  relativePath: string;
  name: string;
  extension: string;
  sizeBytes: number;
  modifiedMs: number;
  kind: FileKind;
}

export interface ScanResult {
  root: string;
  files: ProjectFile[];
  /** The scan stopped at its file limit. */
  truncated: boolean;
}

export interface ModProject {
  modFilePath: string;
  folderPath: string;
  name: string;
  version: string;
  supportedVersion: string;
  tags: string[];
  imagePath: string;
  replacePaths: string[];
}

export interface RecentProject {
  modFilePath: string;
  folderPath: string;
  displayName: string;
  imagePath: string;
  lastOpened: number;
}

export interface Settings {
  version: number;
  theme: ThemeMode;
  gameRootPath: string;
  recentProjects: RecentProject[];
}

export interface TextFile {
  path: string;
  content: string;
  hasBom: boolean;
  encoding: Encoding;
  sizeBytes: number;
}

export interface Focus {
  id: string;
  icon: string;
  x: string;
  y: string;
  cost: string;
  relativePositionId: string;
  /** Each inner list is one `prerequisite = { ... }` block. */
  prerequisites: string[][];
  mutuallyExclusive: string[];
  available: string;
  bypass: string;
  allowBranch: string;
  completionReward: string;
  aiWillDo: string;
  treeId: string;
  shared: boolean;
  line: number;
}

export interface FocusTree {
  id: string;
  country: string;
  default: boolean;
  focusCount: number;
  line: number;
}

export interface FocusFile {
  path: string;
  trees: FocusTree[];
  focuses: Focus[];
  hasBom: boolean;
  encoding: Encoding;
}

export type FocusUpdate = Omit<Focus, "treeId" | "shared" | "line">;

export interface LocalisationEntry {
  key: string;
  version: string;
  value: string;
  /** Source line, or null for an entry added in the app. */
  line: number | null;
}

export interface LocalisationFile {
  path: string;
  language: string;
  entries: LocalisationEntry[];
  hasBom: boolean;
  encoding: Encoding;
}

export interface LocalisationFileInfo {
  path: string;
  relativePath: string;
  language: string;
  entryCount: number;
  hasBom: boolean;
}

/** The editable fields of a focus, with everything blank. */
export function emptyFocusUpdate(): FocusUpdate {
  return {
    id: "",
    icon: "",
    x: "",
    y: "",
    cost: "",
    relativePositionId: "",
    prerequisites: [],
    mutuallyExclusive: [],
    available: "",
    bypass: "",
    allowBranch: "",
    completionReward: "",
    aiWillDo: "",
  };
}

export function toFocusUpdate(focus: Focus): FocusUpdate {
  const { treeId: _treeId, shared: _shared, line: _line, ...editable } = focus;
  return editable;
}
