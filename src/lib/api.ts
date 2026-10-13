import { invoke } from "@tauri-apps/api/core";
import type {
  AppDetails,
  AppEntry,
  Collection,
  Diagnostics,
  ImportPreview,
  IntegrationSlot,
  LaunchResult,
  OllamaStatus,
  PreviewAsset,
  ScanReport,
  SearchResult,
  ShortcutStatus,
  UserPreferences,
  WorkflowCard,
  WorkflowDefinition,
  WorkflowRun,
} from "./types";

export function errorText(error: unknown): string {
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return error instanceof Error ? error.message : String(error);
}

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

export const listApps = (includeHidden = false) => call<AppEntry[]>("list_apps", { includeHidden });
export const appDetails = (id: string) => call<AppDetails>("app_details", { id });
export const searchApps = (query: string) => call<SearchResult[]>("search_apps", { query });
export const launchApp = (id: string, extraArgs: string[] = []) => call<LaunchResult>("launch_app", { id, extraArgs });
export const openPath = (path: string) => call<void>("open_path", { path });
export const openUrl = (url: string) => call<void>("open_url", { url });
export const revealPath = (path: string) => call<void>("reveal_path", { path });
export const copyText = (text: string) => call<void>("copy_text", { text });
export const addApp = (input: { name: string; path: string; args: string[]; workingDir?: string; category?: string }) =>
  call<AppEntry>("add_app", input);
export const updateApp = (id: string, patch: Record<string, unknown>) => call<AppEntry>("update_app", { id, patch });
export const removeApp = (id: string) => call<boolean>("remove_app", { id });
export const reorderApps = (ids: string[]) => call<void>("reorder_apps", { ids });
export const scan = () => call<ScanReport>("scan");
export const listCollections = () => call<Collection[]>("list_collections");
export const createCollection = (name: string) => call<Collection>("create_collection", { name });
export const renameCollection = (id: string, name: string) => call<Collection>("rename_collection", { id, name });
export const deleteCollection = (id: string) => call<void>("delete_collection", { id });
export const setMembership = (appId: string, collectionId: string, member: boolean) =>
  call<void>("set_membership", { appId, collectionId, member });
export const importPreview = (appId: string, sourcePath: string) => call<PreviewAsset>("import_preview", { appId, sourcePath });
export const deletePreview = (id: string) => call<void>("delete_preview", { id });
export const previewData = (id: string) => call<string | null>("preview_data", { id });
export const listWorkflows = () => call<WorkflowCard[]>("list_workflows");
export const saveWorkflow = (definition: WorkflowDefinition) => call<void>("save_workflow", { definition });
export const resetTemplate = (templateId: string) => call<WorkflowDefinition>("reset_template", { templateId });
export const deleteWorkflow = (id: string) => call<void>("delete_workflow", { id });
export const runWorkflow = (id: string, values: Record<string, string>) => call<WorkflowRun>("run_workflow", { id, values });
export const cancelWorkflow = () => call<void>("cancel_workflow");
export const confirmWorkflow = (accepted: boolean) => call<void>("confirm_workflow", { accepted });
export const listRuns = (workflowId?: string) => call<WorkflowRun[]>("list_runs", { workflowId: workflowId ?? null });
export const getPreferences = () => call<UserPreferences>("get_preferences");
export const setPreferences = (prefs: UserPreferences) => call<UserPreferences>("set_preferences", { prefs });
export const reindex = () => call<number>("reindex");
export const exportBackup = () => call<string>("export_backup");
export const previewBackup = (json: string) => call<ImportPreview>("preview_backup", { json });
export const applyBackup = (json: string) => call<ImportPreview>("apply_backup", { json });
export const clearHistory = () => call<number>("clear_history");
export const diagnostics = () => call<Diagnostics>("diagnostics");
export const readLog = () => call<string>("read_log");
export const ollamaStatus = () => call<OllamaStatus>("ollama_status");
export const explainWithOllama = (prompt: string) => call<string>("explain_with_ollama", { prompt });
export const integrationStatus = (probe: boolean) => call<IntegrationSlot[]>("integration_status", { probe });
export const completeOnboarding = () => call<void>("complete_onboarding");
export const shortcutStatus = () => call<ShortcutStatus>("shortcut_status");
export const openDetails = (id: string) => call<void>("open_details", { id });
export const hideLauncher = () => call<void>("hide_launcher");
export const showMain = () => call<void>("show_main");
export const quitApp = () => call<void>("quit");
