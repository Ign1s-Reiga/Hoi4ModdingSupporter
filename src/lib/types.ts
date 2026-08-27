/**
 * Mirrors of the serde types exposed by the Rust backend. Keep the field names
 * in step with `src-tauri/src` — everything crosses the bridge as camelCase.
 */

export type FileKind = 'text' | 'image' | 'binary';

export type ThemeMode = 'system' | 'light' | 'dark';

/** How a file is stored on disk. Legacy scripts are sometimes Windows-1252. */
export type Encoding = 'utf8' | 'windows1252';

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

export type FocusUpdate = Omit<Focus, 'treeId' | 'shared' | 'line'>;

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
    id: '',
    icon: '',
    x: '',
    y: '',
    cost: '',
    relativePositionId: '',
    prerequisites: [],
    mutuallyExclusive: [],
    available: '',
    bypass: '',
    allowBranch: '',
    completionReward: '',
    aiWillDo: '',
  };
}

/** The editable fields of an existing focus, without its source position. */
export function toFocusUpdate(focus: Focus): FocusUpdate {
  return {
    id: focus.id,
    icon: focus.icon,
    x: focus.x,
    y: focus.y,
    cost: focus.cost,
    relativePositionId: focus.relativePositionId,
    prerequisites: focus.prerequisites,
    mutuallyExclusive: focus.mutuallyExclusive,
    available: focus.available,
    bypass: focus.bypass,
    allowBranch: focus.allowBranch,
    completionReward: focus.completionReward,
    aiWillDo: focus.aiWillDo,
  };
}

/** A map state from `history/states/*.txt`. */
export interface MapState {
  id: string;
  /** Localisation key, e.g. `STATE_1`. */
  name: string;
  manpower: string;
  stateCategory: string;
  localSupplies: string;
  buildingsMaxLevelFactor: string;
  provinces: string[];
  /** Body of `resources = { ... }`, kept as text. */
  resources: string;
  owner: string;
  controller: string;
  cores: string[];
  claims: string[];
  /** Body of `buildings = { ... }`, which nests province-keyed blocks. */
  buildings: string;
  /** One entry per `victory_points` block, as `province value`. */
  victoryPoints: string[];
  line: number;
}

export interface StateFile {
  path: string;
  states: MapState[];
  hasBom: boolean;
  encoding: Encoding;
  /** Blocks the file never closed. The game tolerates these. */
  unclosedBlocks: number;
}

export type StateUpdate = Omit<MapState, 'line'>;

export interface Politics {
  rulingParty: string;
  lastElection: string;
  electionFrequency: string;
  /** `yes`, `no`, or empty when the file does not say. */
  electionsAllowed: string;
}

export interface Popularity {
  ideology: string;
  value: string;
}

/** The starting setup of one country, from `history/countries/TAG - Name.txt`. */
export interface CountryHistory {
  path: string;
  tag: string;
  fileName: string;
  capital: string;
  oob: string;
  researchSlots: string;
  convoys: string;
  stability: string;
  warSupport: string;
  politics: Politics;
  popularities: Popularity[];
  hasBom: boolean;
  encoding: Encoding;
  /** Blocks the file never closed. The game tolerates these. */
  unclosedBlocks: number;
}

export interface CountryHistoryInfo {
  path: string;
  relativePath: string;
  tag: string;
  fileName: string;
}

export type CountryHistoryUpdate = Pick<
  CountryHistory,
  'capital' | 'oob' | 'researchSlots' | 'convoys' | 'stability' | 'warSupport' | 'politics' | 'popularities'
>;

export function toStateUpdate(state: MapState): StateUpdate {
  return {
    id: state.id,
    name: state.name,
    manpower: state.manpower,
    stateCategory: state.stateCategory,
    localSupplies: state.localSupplies,
    buildingsMaxLevelFactor: state.buildingsMaxLevelFactor,
    provinces: state.provinces,
    resources: state.resources,
    owner: state.owner,
    controller: state.controller,
    cores: state.cores,
    claims: state.claims,
    buildings: state.buildings,
    victoryPoints: state.victoryPoints,
  };
}

export function toCountryHistoryUpdate(country: CountryHistory): CountryHistoryUpdate {
  return {
    capital: country.capital,
    oob: country.oob,
    researchSlots: country.researchSlots,
    convoys: country.convoys,
    stability: country.stability,
    warSupport: country.warSupport,
    politics: country.politics,
    popularities: country.popularities,
  };
}
