import { invoke } from "@tauri-apps/api/core";
import type {
  AppDetails,
  AppEntry,
  AppInfo,
  Collection,
  Diagnostics,
  Facts,
  HotkeyStatus,
  LaunchOutcome,
  PreviewView,
  ScanReport,
  SearchResult,
  Step,
  UserPreferences,
  WorkflowDefinition,
  WorkflowRun,
  WorkflowView,
} from "./types";

export function errorText(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return error instanceof Error ? error.message : String(error);
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (error) {
    throw new Error(errorText(error), { cause: error });
  }
}

export const api = {
  listApps: (includeHidden = false) => call<AppEntry[]>("list_apps", { includeHidden }),
  getApp: (id: string) => call<AppDetails>("get_app", { id }),
  appIcons: (ids: string[]) => call<[string, string | null][]>("app_icons", { ids }),
  scanApps: () => call<ScanReport>("scan_apps"),
  addApp: (app: Record<string, unknown>, collectionId?: string) => call<AppEntry>("add_app", { app, collectionId }),
  updateApp: (id: string, update: Record<string, unknown>) => call<AppEntry>("update_app", { id, update }),
  removeApp: (id: string) => call<{ deleted: boolean; message: string }>("remove_app", { id }),
  reorderApps: (ids: string[]) => call<void>("reorder_apps", { ids }),
  listCollections: () => call<Collection[]>("list_collections"),
  createCollection: (name: string) => call<Collection>("create_collection", { name }),
  renameCollection: (id: string, name: string) => call<void>("rename_collection", { id, name }),
  deleteCollection: (id: string) => call<void>("delete_collection", { id }),
  setMembership: (collectionId: string, appId: string, member: boolean) =>
    call<void>("set_collection_membership", { collectionId, appId, member }),
  launchApp: (id: string, args?: string[]) => call<LaunchOutcome>("launch_registered_app", { id, args }),
  revealApp: (id: string) => call<LaunchOutcome>("reveal_app", { id }),
  importPreview: (appId: string, sourcePath: string) => call<PreviewView>("import_preview", { appId, sourcePath }),
  clearPreviews: (appId: string) => call<void>("clear_previews", { appId }),
  listWorkflows: () => call<WorkflowView[]>("list_workflows"),
  configureWorkflow: (id: string, values: Record<string, string>) => call<WorkflowView>("configure_workflow", { id, values }),
  saveWorkflowSteps: (id: string, edit: { name: string; description: string; params: WorkflowView["params"]; steps: Step[] }) =>
    call<WorkflowView>("save_workflow_steps", { id, edit }),
  createWorkflow: (name: string) => call<WorkflowView>("create_workflow", { name }),
  resetTemplate: (id: string) => call<WorkflowView>("reset_template", { id }),
  deleteWorkflow: (id: string) => call<void>("delete_workflow", { id }),
  trustWorkflow: (id: string) => call<WorkflowView>("trust_workflow", { id }),
  describeWorkflow: (id: string) => call<string[]>("describe_workflow", { id }),
  runWorkflow: (id: string) => call<WorkflowRun>("run_workflow", { id }),
  confirmWorkflow: (workflowId: string, accepted: boolean) => call<void>("confirm_workflow", { workflowId, accepted }),
  cancelWorkflow: (workflowId: string) => call<void>("cancel_workflow", { workflowId }),
  appInfo: () => call<AppInfo>("app_info"),
  getPreferences: () => call<UserPreferences>("get_preferences"),
  savePreferences: (prefs: UserPreferences) => call<HotkeyStatus>("save_preferences", { prefs }),
  search: (query: string) => call<SearchResult[]>("search", { query }),
  openUrl: (url: string) => call<LaunchOutcome>("open_url", { url }),
  openIndexed: (path: string) => call<LaunchOutcome>("open_indexed", { path }),
  copyText: (text: string) => call<void>("copy_text", { text }),
  clearHistory: () => call<void>("clear_history"),
  reindexFiles: () => call<{ indexed: number; truncated: boolean; errors: string[] }>("reindex_files"),
  diagnostics: () => call<Diagnostics>("diagnostics"),
  environment: () => call<Facts>("environment"),
  ollamaStatus: () => call<{ enabled: boolean; endpoint: string; reachable: boolean; models: string[]; message: string }>("ollama_status"),
  ollamaSuggest: (prompt: string) => call<{ text: string; executed: boolean; notice: string }>("ollama_suggest", { prompt }),
  pulseflowStatus: () => call<{ configured: boolean; reachable: boolean; message: string }>("pulseflow_status"),
  showLauncher: () => call<void>("show_launcher"),
  hideLauncher: () => call<void>("hide_launcher"),
  showMain: () => call<void>("show_main"),
  quit: () => call<void>("quit_app"),
  exportBundle: () => call<string>("export_bundle"),
  importBundle: (json: string, apply: boolean) =>
    call<{ applied: boolean; appsAdded: number; appsSkipped: string[]; workflows: number; collections: number; notes: string[] }>(
      "import_bundle",
      { json, apply },
    ),
  templateCatalog: () => call<WorkflowDefinition[]>("template_catalog"),
};

export type Api = typeof api;
