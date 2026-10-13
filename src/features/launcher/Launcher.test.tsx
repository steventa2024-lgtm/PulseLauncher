import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Launcher } from "./Launcher";
import type { SearchResult } from "../../lib/types";

const searchApps = vi.fn();
const launchApp = vi.fn();
const hideLauncher = vi.fn();
const copyText = vi.fn();
const openDetails = vi.fn();

vi.mock("../../lib/api", () => ({
  searchApps: (...args: unknown[]) => searchApps(...args),
  launchApp: (...args: unknown[]) => launchApp(...args),
  hideLauncher: (...args: unknown[]) => hideLauncher(...args),
  copyText: (...args: unknown[]) => copyText(...args),
  openDetails: (...args: unknown[]) => openDetails(...args),
  openUrl: vi.fn(),
  openPath: vi.fn(),
  showMain: vi.fn(),
  runWorkflow: vi.fn(),
  errorText: (e: unknown) => String(e),
}));

const rows: SearchResult[] = [
  {
    id: "app:1",
    category: "app",
    title: "Notepad",
    subtitle: "C:\\Windows\\notepad.exe",
    score: 10,
    matchIndices: [0],
    action: { type: "launchApp", appId: "1" },
    appId: "1",
    favorite: false,
  },
  {
    id: "calc",
    category: "calculator",
    title: "= 4",
    subtitle: "2+2",
    score: 9,
    matchIndices: [],
    action: { type: "copyText", text: "4" },
    favorite: false,
  },
];

describe("Launcher", () => {
  beforeEach(() => {
    searchApps.mockReset();
    launchApp.mockReset();
    hideLauncher.mockReset();
    copyText.mockReset();
    openDetails.mockReset();
    searchApps.mockResolvedValue(rows);
    launchApp.mockResolvedValue({ outcome: "accepted", message: "Windows accepted the launch request.", commandPreview: "notepad.exe" });
    hideLauncher.mockResolvedValue(undefined);
    copyText.mockResolvedValue(undefined);
    openDetails.mockResolvedValue(undefined);
  });

  it("moves with the keyboard and launches the selected app", async () => {
    const user = userEvent.setup();
    render(<Launcher />);
    await screen.findByRole("option", { name: /Notepad/ });
    await user.keyboard("{ArrowDown}{Enter}");
    await waitFor(() => expect(copyText).toHaveBeenCalledWith("4"));
    expect(launchApp).not.toHaveBeenCalled();
  });

  it("launches the first result on enter and reports a refusal", async () => {
    const user = userEvent.setup();
    launchApp.mockResolvedValue({ outcome: "missingTarget", message: "file not found: gone.exe", commandPreview: "gone.exe" });
    searchApps.mockResolvedValue([rows[0]]);
    render(<Launcher />);
    await user.click(await screen.findByRole("option", { name: /Notepad/ }));
    expect(await screen.findByText(/file not found/)).toBeInTheDocument();
    expect(hideLauncher).not.toHaveBeenCalled();
  });

  it("opens details with Alt+Enter", async () => {
    const user = userEvent.setup();
    searchApps.mockResolvedValue([rows[0]]);
    render(<Launcher />);
    await screen.findByRole("option", { name: /Notepad/ });
    await user.keyboard("{Alt>}{Enter}{/Alt}");
    await waitFor(() => expect(openDetails).toHaveBeenCalledWith("1"));
  });

  it("shows an empty state instead of invented apps", async () => {
    searchApps.mockResolvedValue([]);
    render(<Launcher />);
    expect(await screen.findByText(/No matches/)).toBeInTheDocument();
    expect(screen.queryByText("PulseCut")).not.toBeInTheDocument();
  });
});
