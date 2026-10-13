export type AppKind = "shortcut" | "executable" | "packaged" | "uri";
export type DiscoveredSource = "start_menu_user" | "start_menu_common" | "desktop" | "packaged" | "manual";
export type TargetStatus = "ok" | "missing" | "unknown";

export interface ExecutableTarget {
  kind: AppKind;
  launchTarget: string;
  resolvedTarget: string | null;
  arguments: string[];
  linkArguments: string | null;
  workingDir: string | null;
}

export interface AppMetadata {
  version: string | null;
  publisher: string | null;
  description: string | null;
  productName: string | null;
  fileSize: number | null;
  modifiedAt: string | null;
}

export interface AppEntry {
  id: string;
  name: string;
  originalName: string;
  source: DiscoveredSource;
  target: ExecutableTarget;
  iconPath: string | null;
  iconIndex: number;
  favorite: boolean;
  hidden: boolean;
  portable: boolean;
  category: string | null;
  tags: string[];
  collectionIds: string[];
  status: TargetStatus;
  metadata: AppMetadata;
  launchCount: number;
  lastLaunchedAt: string | null;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  lastSeenAt: string | null;
  previewAssetId: string | null;
}

export interface Collection {
  id: string;
  name: string;
  systemKey: string | null;
  sortOrder: number;
  appIds: string[];
}

export interface LaunchEvent {
  id: number;
  appId: string | null;
  workflowId: string | null;
  label: string;
  at: string;
  success: boolean;
  processId: number | null;
  message: string;
}

export interface LaunchOutcome {
  accepted: boolean;
  processId: number | null;
  message: string;
  executed: string;
}

export interface PreviewView {
  id: string;
  source: string;
  createdAt: string;
  dataUrl: string | null;
}

export interface AppDetails {
  app: AppEntry;
  events: LaunchEvent[];
  previews: PreviewView[];
}

export interface ScanReport {
  startedAt: string;
  finishedAt: string;
  found: number;
  added: number;
  updated: number;
  removed: number;
  markedMissing: number;
  errors: string[];
  sources: string[];
}

export interface UserPreferences {
  theme: string;
  accent: string;
  density: string;
  reduceMotion: boolean;
  translucency: boolean;
  textScale: number;
  highContrast: boolean;
  launcherHotkey: string;
  launcherHotkeyFallbacks: string[];
  hideLauncherOnBlur: boolean;
  closeToTray: boolean;
  startMinimized: boolean;
  launchAtStartup: boolean;
  searchFiles: boolean;
  indexRoots: string[];
  indexExclusions: string[];
  indexMaxEntries: number;
  includeDesktopShortcuts: boolean;
  includePackagedApps: boolean;
  webSearchEnabled: boolean;
  webSearchTemplate: string;
  recordHistory: boolean;
  developerCommandsEnabled: boolean;
  ollamaEnabled: boolean;
  ollamaEndpoint: string;
  ollamaModel: string;
  pulseflowUrl: string;
  onboardingComplete: boolean;
  showAppIcons: boolean;
  showShortcuts: boolean;
}

export interface HotkeyStatus {
  requested: string;
  active: string | null;
  warning: string | null;
}

export type ResultKind = "app" | "command" | "workflow" | "file" | "folder" | "calculator" | "web_search";

export interface SearchResult {
  key: string;
  kind: ResultKind;
  title: string;
  subtitle: string;
  score: number;
  highlights: number[];
  appId: string | null;
  workflowId: string | null;
  path: string | null;
  value: string | null;
  favorite: boolean;
  missing: boolean;
  iconKey: string | null;
}

export type ParamKind = "text" | "folder" | "file" | "path" | "url" | "app" | "apps" | "choice";

export interface ParamDef {
  key: string;
  label: string;
  help: string;
  kind: ParamKind;
  required: boolean;
  defaultValue: string;
  choices: { value: string; label: string }[];
}

export interface Step {
  type: string;
  onError?: "stop" | "continue" | "skip";
  retries?: number;
  appId?: string;
  args?: string[];
  apps?: string;
  path?: string;
  url?: string;
  uri?: string;
  text?: string;
  millis?: number;
  title?: string;
  body?: string;
  folder?: string;
  prefix?: string;
  content?: string;
  message?: string;
  program?: string;
  thenSteps?: Step[];
  elseSteps?: Step[];
}

export interface WorkflowDefinition {
  schema: number;
  id: string;
  name: string;
  description: string;
  whatItDoes: string;
  prerequisites: string;
  kind: "command" | "workflow";
  templateKey: string | null;
  templateVersion: number | null;
  params: ParamDef[];
  values: Record<string, string>;
  steps: Step[];
  hotkey: string | null;
  trusted: boolean;
}

export interface Readiness {
  state: "ready" | "needs_setup" | "unavailable";
  details: string[];
}

export interface StepLog {
  index: string;
  action: string;
  status: string;
  message: string;
  detail: string;
}

export interface WorkflowRun {
  id: string;
  workflowId: string;
  name: string;
  status: "running" | "succeeded" | "failed" | "cancelled";
  startedAt: string;
  finishedAt: string | null;
  steps: StepLog[];
  error: string | null;
}

export interface WorkflowView extends WorkflowDefinition {
  readiness: Readiness;
  lastRun: WorkflowRun | null;
}

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
  windowsNative: boolean;
  dataDir: string;
  dbPath: string;
}

export interface Diagnostics {
  info: AppInfo;
  schemaVersion: number;
  appCount: number;
  portableCount: number;
  favoriteCount: number;
  fileIndexCount: number;
  lastScan: ScanReport | null;
  hotkey: HotkeyStatus;
  recentEvents: LaunchEvent[];
}

export interface Facts {
  isWindows: boolean;
  downloads: string | null;
  documents: string | null;
  taskManager: string | null;
  terminals: { label: string; path: string }[];
}

export interface Notice {
  id: number;
  title: string;
  body: string;
  tone?: "info" | "ok" | "warn" | "danger";
}
