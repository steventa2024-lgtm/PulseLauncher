import { create } from "zustand";
import type { AppEntry, AppInfo, Collection, Notice, UserPreferences } from "./types";

export const DEFAULT_PREFS: UserPreferences = {
  theme: "dark",
  accent: "#4388FF",
  density: "comfortable",
  reduceMotion: false,
  translucency: true,
  textScale: 1,
  highContrast: false,
  launcherHotkey: "Alt+Space",
  launcherHotkeyFallbacks: ["Ctrl+Alt+Space"],
  hideLauncherOnBlur: true,
  closeToTray: true,
  startMinimized: false,
  launchAtStartup: false,
  searchFiles: false,
  indexRoots: [],
  indexExclusions: ["node_modules", ".git", "target", "AppData"],
  indexMaxEntries: 20000,
  includeDesktopShortcuts: false,
  includePackagedApps: true,
  webSearchEnabled: false,
  webSearchTemplate: "https://duckduckgo.com/?q={query}",
  recordHistory: true,
  developerCommandsEnabled: false,
  ollamaEnabled: false,
  ollamaEndpoint: "http://127.0.0.1:11434",
  ollamaModel: "",
  pulseflowUrl: "",
  onboardingComplete: false,
};

interface UiState {
  prefs: UserPreferences;
  apps: AppEntry[];
  collections: Collection[];
  icons: Record<string, string | null>;
  notices: Notice[];
  info: AppInfo | null;
  bridge: "unknown" | "connected" | "unavailable";
  setPrefs: (prefs: UserPreferences) => void;
  setApps: (apps: AppEntry[]) => void;
  setCollections: (collections: Collection[]) => void;
  mergeIcons: (pairs: [string, string | null][]) => void;
  setInfo: (info: AppInfo | null) => void;
  setBridge: (bridge: UiState["bridge"]) => void;
  push: (title: string, body: string) => void;
  dismiss: (id: number) => void;
}

let noticeId = 1;

export const useUi = create<UiState>((set) => ({
  prefs: DEFAULT_PREFS,
  apps: [],
  collections: [],
  icons: {},
  notices: [],
  info: null,
  bridge: "unknown",
  setPrefs: (prefs) => set({ prefs }),
  setApps: (apps) => set({ apps }),
  setCollections: (collections) => set({ collections }),
  mergeIcons: (pairs) =>
    set((state) => {
      const icons = { ...state.icons };
      for (const [id, icon] of pairs) icons[id] = icon;
      return { icons };
    }),
  setInfo: (info) => set({ info }),
  setBridge: (bridge) => set({ bridge }),
  push: (title, body) =>
    set((state) => ({
      notices: [...state.notices, { id: noticeId++, title, body }].slice(-4),
    })),
  dismiss: (id) => set((state) => ({ notices: state.notices.filter((n) => n.id !== id) })),
}));

export function notify(title: string, body: string) {
  useUi.getState().push(title, body);
}
