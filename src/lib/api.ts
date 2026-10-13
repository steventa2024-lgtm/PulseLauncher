import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AppEntry,
  Collection,
  Diagnostics,
  ImportPreview,
  LaunchEvent,
  LaunchOutcome,
  LaunchPreview,
  OllamaReport,
  PreviewAsset,
  PulseFlowReport,
  ScanReport,
  SearchHit,
  StepPreview,
  Suggestion,
  UserPreferences,
  WorkflowRun,
  WorkflowView,
} from "./types";

export class DesktopError extends Error {
  code: string;

  constructor(code: string, message: string) {
    super(message);
    this.code = code;
  }
}

export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function explain(error: unknown): DesktopError {
  if (error instanceof DesktopError) return error;
  if (typeof error === "string") {
    try {
      const parsed = JSON.parse(error) as { code?: string; message?: string };
      if (parsed && typeof parsed.message === "string") {
        return new DesktopError(parsed.code ?? "ERROR", parsed.message);
      }
    } catch {
      return new DesktopError("ERROR", error);
    }
    return new DesktopError("ERROR", error);
  }
  if (error && typeof error === "object" && "message" in error) {
    const record = error as { code?: unknown; message?: unknown };
    return new DesktopError(String(record.code ?? "ERROR"), String(record.message));
  }
  return new DesktopError("ERROR", "The desktop command failed.");
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri()) {
    throw new DesktopError(
      "BRIDGE",
      "PulseLauncher commands run inside the desktop app. This page has no native shell bridge.",
    );
  }
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw explain(error);
  }
}

export const api = {
  diagnostics: () => call<Diagnostics>("command_diagnostics"),
  preferencesGet: () => call<UserPreferences>("command_preferences_get"),
  preferencesSet: (preferences: UserPreferences) => call<UserPreferences>("command_preferences_set", { preferences }),
  appsList: (includeHidden = false) => call<AppEntry[]>("command_apps_list", { includeHidden }),
  appsGet: (id: string) => call<AppEntry>("command_apps_get", { id }),
  appsScan: () => call<ScanReport>("command_apps_scan"),
  appsRegister: (request: {
    displayName: string;
    target: string;
    arguments: string[];
    workingDirectory?: string | null;
    tags: string[];
    collectionId?: string | null;
    notes?: string | null;
  }) => call<AppEntry>("command_apps_register", { request }),
  appsUpdate: (request: {
    id: string;
    displayName: string;
    target: string;
    arguments: string[];
    workingDirectory?: string | null;
    tags: string[];
    category: string;
    notes?: string | null;
  }) => call<AppEntry>("command_apps_update", { request }),
  appsHide: (id: string, hidden: boolean) => call<void>("command_apps_hide", { id, hidden }),
  appsDeleteManual: (id: string) => call<void>("command_apps_delete_manual", { id }),
  appsFavorite: (id: string, favorite: boolean) => call<void>("command_apps_favorite", { id, favorite }),
  appsPin: (id: string, pinned: boolean) => call<void>("command_apps_pin", { id, pinned }),
  appsLaunch: (id: string) => call<LaunchOutcome>("command_apps_launch", { id }),
  appsLaunchPreview: (id: string) => call<LaunchPreview>("command_apps_launch_preview", { id }),
  appsReveal: (id: string) => call<LaunchOutcome>("command_apps_reveal", { id }),
  appsCopyPath: (id: string) => call<void>("command_apps_copy_path", { id }),
  iconsRead: (path: string) => call<string | null>("command_icons_read", { path }),
  collectionsList: () => call<Collection[]>("command_collections_list"),
  collectionsCreate: (name: string) => call<Collection>("command_collections_create", { name }),
  collectionsRename: (id: string, name: string) => call<void>("command_collections_rename", { id, name }),
  collectionsDelete: (id: string) => call<void>("command_collections_delete", { id }),
  collectionsSetMembers: (id: string, appIds: string[]) => call<void>("command_collections_set_members", { id, appIds }),
  workflowsList: () => call<WorkflowView[]>("command_workflows_list"),
  workflowsGet: (id: string) => call<WorkflowView>("command_workflows_get", { id }),
  workflowsSave: (record: WorkflowView) => call<WorkflowView>("command_workflows_save", { record }),
  workflowsReset: (id: string) => call<WorkflowView>("command_workflows_reset", { id }),
  workflowsPreview: (id: string) => call<StepPreview[]>("command_workflows_preview", { id }),
  workflowsStart: (id: string) => call<WorkflowRun>("command_workflows_start", { id }),
  workflowsCancel: (runId: string) => call<void>("command_workflows_cancel", { runId }),
  workflowsConfirm: (runId: string, accepted: boolean) => call<void>("command_workflows_confirm", { runId, accepted }),
  workflowsRuns: (id: string) => call<WorkflowRun[]>("command_workflows_runs", { id }),
  search: (query: string) => call<SearchHit[]>("command_search", { query }),
  searchActivate: (query: string, hitId: string) => call<string>("command_search_activate", { query, hitId }),
  historyList: (appId?: string) => call<LaunchEvent[]>("command_history_list", { appId: appId ?? null }),
  historyClear: () => call<void>("command_history_clear"),
  backupPreview: (json: string) => call<ImportPreview>("command_backup_preview", { json }),
  backupApply: (json: string) => call<ImportPreview>("command_backup_apply", { json }),
  backupWrite: (path: string, includeHistory: boolean) => call<void>("command_backup_write", { path, includeHistory }),
  backupRead: (path: string) => call<string>("command_backup_read", { path }),
  previewImport: (appId: string, sourcePath: string) => call<PreviewAsset>("command_preview_import", { appId, sourcePath }),
  previewDelete: (id: string) => call<void>("command_preview_delete", { id }),
  previewsList: (appId: string) => call<PreviewAsset[]>("command_previews_list", { appId }),
  previewData: (id: string) => call<string>("command_preview_data", { id }),
  indexRun: () => call<number>("command_index_run"),
  ollamaTest: () => call<OllamaReport>("command_ollama_test"),
  ollamaSuggest: (prompt: string) => call<Suggestion>("command_ollama_suggest", { prompt }),
  pulseflowTest: () => call<PulseFlowReport>("command_pulseflow_test"),
  openpulseLaunch: () => call<LaunchOutcome>("command_openpulse_launch"),
  calculate: (expression: string) => call<string>("command_calculate", { expression }),
  showLauncher: () => call<void>("command_show_launcher"),
  hideLauncher: () => call<void>("command_hide_launcher"),
  openDetails: (id: string) => call<void>("command_open_details", { id }),
};

export async function pickPath(directory: boolean, filters?: { name: string; extensions: string[] }[]): Promise<string | null> {
  if (!inTauri()) {
    throw new DesktopError("BRIDGE", "File browsing is available in the desktop app.");
  }
  const selected = await open({ multiple: false, directory, filters });
  return typeof selected === "string" ? selected : null;
}

export async function pickSaveJson(defaultPath: string): Promise<string | null> {
  if (!inTauri()) {
    throw new DesktopError("BRIDGE", "Saving a backup is available in the desktop app.");
  }
  return save({ defaultPath, filters: [{ name: "JSON", extensions: ["json"] }] });
}
