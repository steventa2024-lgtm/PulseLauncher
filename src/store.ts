import { create } from "zustand";
import type { UserPreferences, WorkflowRun } from "./lib/types";

export type ShellView = "apps" | "details" | "commands" | "settings" | "gallery";

interface ShellStore {
  view: ShellView;
  selectedAppId: string | null;
  toast: string | null;
  preferences: UserPreferences | null;
  liveRuns: Record<string, WorkflowRun>;
  confirm: { runId: string; message: string } | null;
  setView: (view: ShellView) => void;
  openApp: (id: string) => void;
  notify: (toast: string | null) => void;
  setPreferences: (preferences: UserPreferences) => void;
  setLiveRun: (run: WorkflowRun) => void;
  setConfirm: (confirm: { runId: string; message: string } | null) => void;
}

export const useShell = create<ShellStore>((set) => ({
  view: "apps",
  selectedAppId: null,
  toast: null,
  preferences: null,
  liveRuns: {},
  confirm: null,
  setView: (view) => set({ view }),
  openApp: (id) => set({ view: "details", selectedAppId: id }),
  notify: (toast) => set({ toast }),
  setPreferences: (preferences) => set({ preferences }),
  setLiveRun: (run) => set((state) => ({ liveRuns: { ...state.liveRuns, [run.workflowId]: run } })),
  setConfirm: (confirm) => set({ confirm }),
}));

export function applyPreferences(preferences: UserPreferences) {
  const root = document.documentElement;
  root.dataset.theme = preferences.theme;
  root.dataset.density = preferences.density;
  root.dataset.contrast = preferences.highContrast ? "high" : "normal";
  root.dataset.reduceMotion = preferences.reduceMotion ? "true" : "false";
  root.style.setProperty("--text-scale", String(preferences.textScale));
}
