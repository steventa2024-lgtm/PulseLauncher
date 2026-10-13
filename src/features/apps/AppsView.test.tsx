import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AppsView } from "./AppsView";
import type { AppEntry } from "../../lib/types";

const launchApp = vi.fn();
vi.mock("../../lib/api", () => ({
  launchApp: (...args: unknown[]) => launchApp(...args),
  listApps: vi.fn(),
  listCollections: vi.fn(),
  scan: vi.fn(),
  updateApp: vi.fn(),
  removeApp: vi.fn(),
  reorderApps: vi.fn(),
  setMembership: vi.fn(),
  addApp: vi.fn(),
  errorText: (e: unknown) => String(e),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

function app(partial: Partial<AppEntry>): AppEntry {
  return {
    id: "1",
    name: "Tool",
    userRenamed: true,
    source: "manual",
    target: { kind: "executable", path: "C:\\Tools\\My Tool\\tool.exe", args: [] },
    identityKey: "exe:c:\\tools\\my tool\\tool.exe",
    provenance: "",
    category: "developerTools",
    tags: [],
    favorite: false,
    hidden: false,
    stale: false,
    sortOrder: 1,
    createdAt: "",
    updatedAt: "",
    collectionIds: [],
    launchCount: 0,
    ...partial,
  };
}

describe("AppsView", () => {
  it("shows an empty state with a real add action", () => {
    render(<AppsView initialApps={[]} initialCollections={[]} />);
    expect(screen.getByText("No applications yet")).toBeInTheDocument();
    expect(screen.queryByText("PulseCut")).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Add App" }).length).toBeGreaterThan(0);
  });

  it("launches the registered program from its card", async () => {
    const user = userEvent.setup();
    launchApp.mockResolvedValue({ outcome: "accepted", message: "Started", commandPreview: "tool.exe" });
    render(<AppsView initialApps={[app({})]} initialCollections={[]} />);
    expect(screen.getByText(/tool\.exe/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Run" }));
    expect(launchApp).toHaveBeenCalledWith("1");
  });
});
