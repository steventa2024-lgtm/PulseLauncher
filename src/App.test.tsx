import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { DEFAULT_PREFS, useUi } from "./lib/store";

const getPreferences = vi.hoisted(() => vi.fn());
const savePreferences = vi.hoisted(() => vi.fn());
const listWorkflows = vi.hoisted(() => vi.fn());
const environment = vi.hoisted(() => vi.fn());
const diagnostics = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));

vi.mock("./lib/api", () => ({
  api: {
    getPreferences,
    savePreferences,
    appInfo: vi.fn(),
    listApps: vi.fn(),
    listCollections: vi.fn(),
    appIcons: vi.fn(),
    showLauncher: vi.fn(),
    listWorkflows,
    environment,
    diagnostics,
  },
  errorText: (error: unknown) => (error instanceof Error ? error.message : String(error)),
}));

describe("App", () => {
  beforeEach(() => {
    useUi.setState({
      prefs: DEFAULT_PREFS,
      apps: [],
      collections: [],
      icons: {},
      notices: [],
      info: null,
      bridge: "unknown",
    });
    getPreferences.mockRejectedValue(new Error("bridge down"));
    listWorkflows.mockResolvedValue([]);
    environment.mockResolvedValue({ isWindows: false, downloads: null, documents: null, taskManager: null, terminals: [] });
    diagnostics.mockRejectedValue(new Error("bridge down"));
    savePreferences.mockResolvedValue({ requested: "Alt+Space", active: null, warning: null });
  });

  it("keeps the five surfaces reachable and does not fill the catalog", async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByText(/desktop bridge is not connected/i)).toBeInTheDocument();
    expect(screen.getByText("No apps yet")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Commands & Workflows" }));
    expect(await screen.findByText("No workflows stored.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(await screen.findByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Interface gallery" })).toBeInTheDocument();
  });
});
