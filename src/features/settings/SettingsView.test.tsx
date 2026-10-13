import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SettingsView } from "./SettingsView";
import type { UserPreferences } from "../../lib/types";

const setPreferences = vi.fn();
vi.mock("../../lib/api", () => ({
  setPreferences: (...args: unknown[]) => setPreferences(...args),
  getPreferences: vi.fn(),
  diagnostics: vi.fn(),
  integrationStatus: vi.fn(),
  clearHistory: vi.fn(),
  exportBackup: vi.fn(),
  previewBackup: vi.fn(),
  applyBackup: vi.fn(),
  readLog: vi.fn(),
  ollamaStatus: vi.fn(),
  errorText: (e: unknown) => String(e),
}));

const prefs: UserPreferences = {
  theme: "dark",
  accent: "#4388FF",
  density: "comfortable",
  reduceMotion: false,
  translucency: true,
  textScale: 1,
  highContrast: false,
  launcherShortcut: "Alt+Space",
  closeOnBlur: true,
  closeToTray: true,
  startWithWindows: false,
  showWebSearch: true,
  webSearchUrl: "https://duckduckgo.com/?q={query}",
  indexFiles: false,
  indexRoots: [],
  indexExclusions: [],
  recordHistory: true,
  developerCommandExecution: false,
  onboardingDone: true,
  desktopShortcutsScan: false,
  ollamaEnabled: false,
  ollamaEndpoint: "http://127.0.0.1:11434",
  ollamaModel: "",
  pulseflowEndpoint: "",
  openpulseUrl: "",
};

describe("SettingsView", () => {
  it("keeps developer command execution off until the user opts in", async () => {
    const user = userEvent.setup();
    setPreferences.mockImplementation(async (next: UserPreferences) => next);
    render(<SettingsView initial={prefs} />);
    const box = screen.getByRole("checkbox", { name: "Developer command execution" });
    expect(box).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(setPreferences).toHaveBeenCalledWith(expect.objectContaining({ developerCommandExecution: false }));
  });

  it("applies the light theme token", async () => {
    const user = userEvent.setup();
    render(<SettingsView initial={prefs} />);
    const themes = screen.getAllByRole("combobox", { name: /^Theme$/ });
    await user.selectOptions(themes[0], "light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
