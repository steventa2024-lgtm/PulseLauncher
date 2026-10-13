export type TargetKind = "executable" | "shortcut" | "packaged" | "uri";
export type AppSource = "startMenuUser" | "startMenuCommon" | "packaged" | "desktop" | "manual";
export type AppCategory = "none" | "developerTools" | "productivity";
export type TargetStatus = "ok" | "missingTarget" | "missingWorkingDir" | "unverified";
export type LaunchOutcome = "accepted" | "failed" | "missingTarget";

export interface ExecutableTarget {
  kind: TargetKind;
  path: string;
  args: string[];
  workingDir?: string | null;
  aumid?: string | null;
  uri?: string | null;
}

export interface AppEntry {
  id: string;
  name: string;
  userRenamed: boolean;
  source: AppSource;
  target: ExecutableTarget;
  resolvedPath?: string | null;
  identityKey: string;
  provenance: string;
  publisher?: string | null;
  version?: string | null;
  category: AppCategory;
  tags: string[];
  favorite: boolean;
  hidden: boolean;
  stale: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  collectionIds: string[];
  launchCount: number;
  lastLaunchedAt?: string | null;
}

export interface Collection {
  id: string;
  name: string;
  systemKey?: string | null;
  sortOrder: number;
  appCount: number;
}

export interface AppMetadata {
  version?: string | null;
  publisher?: string | null;
  description?: string | null;
  fileSize?: number | null;
  modifiedAt?: string | null;
}

export interface PreviewAsset {
  id: string;
  appId: string;
  fileName: string;
  caption?: string | null;
  source: string;
  createdAt: string;
}

export interface LaunchEvent {
  id: number;
  appId?: string | null;
  appName: string;
  at: string;
  outcome: LaunchOutcome;
  pid?: number | null;
  detail?: string | null;
  argsDisplay?: string | null;
}

export interface LaunchResult {
  outcome: LaunchOutcome;
  pid?: number | null;
  verifiedRunning?: boolean | null;
  message: string;
  commandPreview: string;
}

export interface AppDetails {
  app: AppEntry;
  status: TargetStatus;
  metadata: AppMetadata;
  events: LaunchEvent[];
  previews: PreviewAsset[];
  duplicates: AppEntry[];
  iconDataUrl?: string | null;
  commandPreview: string;
}

export type SearchCategory = "app" | "exe" | "file" | "folder" | "command" | "workflow" | "calculator" | "web";

export type ResultAction =
  | { type: "launchApp"; appId: string }
  | { type: "runWorkflow"; workflowId: string }
  | { type: "openPath"; path: string }
  | { type: "copyText"; text: string }
  | { type: "webSearch"; url: string };

export interface SearchResult {
  id: string;
  category: SearchCategory;
  title: string;
  subtitle: string;
  score: number;
  matchIndices: number[];
  action: ResultAction;
  appId?: string | null;
  favorite: boolean;
  section?: string | null;
}

export interface ScanIssue {
  path: string;
  reason: string;
}

export interface ScanReport {
  found: number;
  added: number;
  updated: number;
  markedStale: number;
  duplicatesMerged: number;
  skippedUninstallers: number;
  skippedNoTarget: number;
  issues: ScanIssue[];
  scannedRoots: string[];
  finishedAt: string;
  durationMs: number;
}

export interface UserPreferences {
  theme: string;
  accent: string;
  density: string;
  reduceMotion: boolean;
  translucency: boolean;
  textScale: number;
  highContrast: boolean;
  launcherShortcut: string;
  closeOnBlur: boolean;
  closeToTray: boolean;
  startWithWindows: boolean;
  showWebSearch: boolean;
  webSearchUrl: string;
  indexFiles: boolean;
  indexRoots: string[];
  indexExclusions: string[];
  recordHistory: boolean;
  developerCommandExecution: boolean;
  onboardingDone: boolean;
  desktopShortcutsScan: boolean;
  ollamaEnabled: boolean;
  ollamaEndpoint: string;
  ollamaModel: string;
  pulseflowEndpoint: string;
  openpulseUrl: string;
}

export interface ParamDef {
  name: string;
  label: string;
  kind: string;
  required: boolean;
  default?: string | null;
  choices: { value: string; label: string }[];
  help?: string | null;
}

export interface WorkflowStep {
  id: string;
  label: string;
  action: { type: string; [key: string]: unknown };
  when?: unknown;
  onError?: string;
  onErrorParam?: string | null;
  timeoutMs?: number | null;
  retries: number;
}

export interface WorkflowDefinition {
  id: string;
  kind: "command" | "workflow";
  name: string;
  description: string;
  templateId?: string | null;
  templateVersion?: number | null;
  params: ParamDef[];
  values: Record<string, string>;
  prerequisites: unknown[];
  steps: WorkflowStep[];
  enabled: boolean;
}

export interface StepResult {
  stepId: string;
  label: string;
  action: string;
  status: string;
  message: string;
  preview: string;
  attempts: number;
  durationMs: number;
  output?: string | null;
}

export interface WorkflowRun {
  id: string;
  workflowId: string;
  workflowName: string;
  startedAt: string;
  finishedAt?: string | null;
  status: string;
  steps: StepResult[];
}

export interface PrereqResult {
  label: string;
  ok: boolean;
  detail: string;
}

export interface WorkflowCard {
  definition: WorkflowDefinition;
  status: { state: "ready" | "needsSetup"; prerequisites: PrereqResult[]; lastRun?: WorkflowRun | null };
}

export interface ShortcutStatus {
  requested: string;
  active?: string | null;
  fallbackUsed: boolean;
  error?: string | null;
}

export interface Diagnostics {
  version: string;
  platform: string;
  dataDir: string;
  dbPath: string;
  schemaVersion: number;
  appCount: number;
  indexedFiles: number;
  lastScan?: string | null;
  shortcut: ShortcutStatus;
  packagedAppsSupported: boolean;
  developerCommands: boolean;
}

export interface OllamaStatus {
  enabled: boolean;
  reachable: boolean;
  models: string[];
  message: string;
}

export interface IntegrationSlot {
  label: string;
  registeredAppId?: string | null;
  registeredName?: string | null;
  endpoint: string;
  endpointOk?: boolean | null;
  detail: string;
}

export interface ImportPreview {
  schemaVersion: number;
  newApps: number;
  updatedApps: number;
  appsWithMissingTarget: string[];
  newCollections: number;
  updatedWorkflows: number;
  newWorkflows: number;
  workflowsWithCommands: string[];
  preferencesChanged: boolean;
  warnings: string[];
}

export function sourceLabel(source: AppSource): string {
  switch (source) {
    case "startMenuUser":
      return "Start Menu (you)";
    case "startMenuCommon":
      return "Start Menu (all users)";
    case "packaged":
      return "Packaged app";
    case "desktop":
      return "Desktop shortcut";
    case "manual":
      return "Registered by you";
  }
}

export function statusLabel(status: TargetStatus): string {
  switch (status) {
    case "ok":
      return "Available";
    case "missingTarget":
      return "Missing target";
    case "missingWorkingDir":
      return "Working folder missing";
    case "unverified":
      return "Not verified from disk";
  }
}

export function formatBytes(n?: number | null): string {
  if (n == null) return "Unknown";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}
