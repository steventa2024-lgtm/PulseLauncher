import { create } from "zustand";
import type { UserPreferences } from "./types";

export type Page = "apps" | "commands" | "settings" | "details" | "gallery";

interface UiState {
  page: Page;
  selectedAppId: string | null;
  notice: string | null;
  prefs: UserPreferences | null;
  setPage: (page: Page) => void;
  openApp: (id: string) => void;
  setNotice: (notice: string | null) => void;
  setPrefs: (prefs: UserPreferences) => void;
}

export const useUi = create<UiState>((set) => ({
  page: "apps",
  selectedAppId: null,
  notice: null,
  prefs: null,
  setPage: (page) => set({ page }),
  openApp: (id) => set({ page: "details", selectedAppId: id }),
  setNotice: (notice) => set({ notice }),
  setPrefs: (prefs) => set({ prefs }),
}));

export function applyPrefs(prefs: UserPreferences) {
  const root = document.documentElement;
  root.dataset.theme = prefs.theme === "light" ? "light" : "dark";
  root.dataset.density = prefs.density === "compact" ? "compact" : "comfortable";
  root.dataset.motion = prefs.reduceMotion ? "reduce" : "full";
  root.dataset.contrast = prefs.highContrast ? "high" : "normal";
  root.style.setProperty("--accent", prefs.accent || "#4388ff");
  root.style.setProperty("--text-scale", String(prefs.textScale || 1));
}
