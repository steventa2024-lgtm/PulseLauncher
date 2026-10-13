export type AppKind = "win32" | "portable" | "packaged" | "uri" | "desktop" | "shortcut";
export type SourceKind =
  | "startMenuUser"
  | "startMenuCommon"
  | "desktopShortcuts"
  | "manual"
  | "desktopFile"
  | "packaged"
  | "uri";
export type Theme = "dark" | "light";
export type Density = "compact" | "comfortable";
export type ViewMode = "grid" | "list";
export type ErrorPolicy = "stop" | "skip";
export type TemplateStatus = "ready" | "needsSetup" | "unavailable";

export interface UserPreferences {
  theme: Theme;
  density: Density;
  blur: boolean;
  reduceMotion: boolean;
  highContrast: boolean;
  textScale: number;
  shortcut: string;
  closeOnBlur: boolean;
  minimizeToTray: boolean;
  closeToTray: boolean;
  startWithOs: boolean;
  searchFiles: boolean;
  indexRoots: string[];
  indexExclusions: string[];
  indexExtensions: string[];
  indexDesktopShortcuts: boolean;
  developerCommandsEnabled: boolean;
  ollamaEnabled: boolean;
  ollamaEndpoint: string;
  ollamaModel: string;
  onboardingComplete: boolean;
  webSearchEnabled: boolean;
  viewMode: ViewMode;
  activeShortcut: string;
  shortcutWarning: string;
  pulseflowExecutable: string;
  pulseflowEndpoint: string;
  openpulseTarget: string;
}

export interface AppEntry {
  id: string;
  displayName: string;
  kind: AppKind;
  source: SourceKind;
  launchTarget: string;
  resolvedTarget: string | null;
  arguments: string[];
  workingDirectory: string | null;
  iconPath: string | null;
  publisher: string | null;
  version: string | null;
  category: string;
  hidden: boolean;
  favorite: boolean;
  pinned: boolean;
  userModified: boolean;
  discovered: boolean;
  tags: string[];
  launchCount: number;
  lastLaunchedAt: string | null;
  missing: boolean;
  identityKey: string;
  notes: string | null;
  createdAt: string;
  updatedAt: string;
  sizeBytes: number | null;
}

export interface Collection {
  id: string;
  name: string;
  builtIn: boolean;
  position: number;
  createdAt: string;
  appIds: string[];
}

export interface LaunchEvent {
  id: string;
  appId: string | null;
  workflowId: string | null;
  outcome: string;
  message: string;
  createdAt: string;
}

export interface LaunchOutcome {
  accepted: boolean;
  pid: number | null;
  exitCode: number | null;
  waited: boolean;
  message: string;
}

export interface LaunchPreview {
  program: string;
  arguments: string[];
  workingDirectory: string | null;
  display: string;
  viaShell: boolean;
}

export interface ParameterDef {
  key: string;
  label: string;
  kind: "string" | "path" | "url" | "appId" | "enum" | "text";
  required: boolean;
  options: string[];
  help: string;
}

export type Step =
  | { type: "launchApp"; id: string; appId: string; args: string[]; onError: ErrorPolicy }
  | { type: "launchTrustedTool"; id: string; tool: "taskManager" | "terminal"; onError: ErrorPolicy }
  | { type: "openFolder"; id: string; path: string; skipIfBlank: boolean; onError: ErrorPolicy }
  | { type: "openKnownFolder"; id: string; folder: "downloads" | "documents"; onError: ErrorPolicy }
  | { type: "openUrl"; id: string; url: string; onError: ErrorPolicy }
  | { type: "copyText"; id: string; text: string; onError: ErrorPolicy }
  | { type: "delay"; id: string; ms: number; onError: ErrorPolicy }
  | { type: "notify"; id: string; title: string; body: string; onError: ErrorPolicy }
  | { type: "createNote"; id: string; directory: string; body: string; onError: ErrorPolicy }
  | { type: "checkExists"; id: string; path: string; thenId: string; elseId: string }
  | { type: "confirm"; id: string; message: string; thenId: string; elseId: string }
  | {
      type: "runCommand";
      id: string;
      program: string;
      args: string[];
      workingDirectory: string | null;
      timeoutMs: number | null;
      onError: ErrorPolicy;
    }
  | { type: "launchCollection"; id: string; collectionId: string; skipMissing: boolean; onError: ErrorPolicy };

export interface WorkflowDocument {
  whatItDoes: string;
  prerequisitesHelp: string;
  parameters: ParameterDef[];
  values: Record<string, string>;
  steps: Step[];
}

export interface WorkflowRun {
  id: string;
  workflowId: string;
  status: string;
  startedAt: string;
  finishedAt: string | null;
  summary: string;
  steps: { stepId: string; status: string; message: string; startedAt: string; finishedAt: string }[];
}

export interface WorkflowView {
  id: string;
  name: string;
  description: string;
  templateKey: string | null;
  templateVersion: number | null;
  customized: boolean;
  document: WorkflowDocument;
  createdAt: string;
  updatedAt: string;
  prerequisite: { status: TemplateStatus; message: string };
  lastRun: WorkflowRun | null;
}

export interface StepPreview {
  stepId: string;
  action: string;
  detail: string;
}

export interface SearchHit {
  id: string;
  kind: string;
  title: string;
  subtitle: string;
  score: number;
  indices: number[];
  appId: string | null;
  workflowId: string | null;
  path: string | null;
  calculatorValue: string | null;
  url: string | null;
}

export interface PreviewAsset {
  id: string;
  appId: string;
  path: string;
  source: string;
  createdAt: string;
}

export interface Diagnostics {
  version: string;
  os: string;
  dataDir: string;
  dbPath: string;
  appCount: number;
  visibleAppCount: number;
  lastScanAt: string | null;
  lastScanError: string | null;
  indexCount: number;
  lastIndexAt: string | null;
  activeShortcut: string;
  shortcutWarning: string;
  developerCommandsEnabled: boolean;
  notes: string[];
}

export interface ScanReport {
  added: number;
  updated: number;
  markedMissing: number;
  errors: string[];
  scannedAt: string;
}

export interface ImportPreview {
  apps: number;
  collections: number;
  workflows: number;
  events: number;
  warnings: string[];
}

export interface OllamaReport {
  enabled: boolean;
  reachable: boolean;
  endpoint: string;
  models: string[];
  configuredModelPresent: boolean | null;
  message: string;
}

export interface Suggestion {
  text: string;
  executed: boolean;
  model: string;
  message: string;
}

export interface PulseFlowReport {
  configured: boolean;
  executable: string;
  endpoint: string;
  reachable: boolean | null;
  httpStatus: number | null;
  message: string;
}

export const ZEROPULSE_COLLECTION_ID = "col-zeropulse";
