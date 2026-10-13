import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DetailsView } from "./DetailsView";
import type { AppDetails } from "../../lib/types";

const launchApp = vi.fn();
vi.mock("../../lib/api", () => ({
  launchApp: (...args: unknown[]) => launchApp(...args),
  errorText: (e: unknown) => String(e),
  revealPath: vi.fn(),
  copyText: vi.fn(),
  updateApp: vi.fn(),
  removeApp: vi.fn(),
  listCollections: vi.fn(),
  previewData: vi.fn(),
  deletePreview: vi.fn(),
  importPreview: vi.fn(),
  setMembership: vi.fn(),
  appDetails: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const details: AppDetails = {
  app: {
    id: "1",
    name: "Portable Tool",
    userRenamed: true,
    source: "manual",
    target: { kind: "executable", path: "D:\\Apps\\tool.exe", args: ["--safe"] },
    identityKey: "x",
    provenance: "",
    category: "none",
    tags: ["dev"],
    favorite: false,
    hidden: false,
    stale: false,
    sortOrder: 1,
    createdAt: "",
    updatedAt: "",
    collectionIds: [],
    launchCount: 2,
    lastLaunchedAt: "2026-10-12T00:00:00Z",
  },
  status: "missingTarget",
  metadata: { fileSize: null, version: null, publisher: null },
  events: [],
  previews: [],
  duplicates: [],
  commandPreview: "\"D:\\Apps\\tool.exe\" --safe",
};

describe("DetailsView", () => {
  it("shows metadata and the no-preview state", () => {
    render(<DetailsView appId="1" fixture={details} />);
    expect(screen.getByRole("heading", { name: "Portable Tool" })).toBeInTheDocument();
    expect(screen.getByText("Missing target")).toBeInTheDocument();
    expect(screen.getByText("No preview available")).toBeInTheDocument();
    expect(screen.getByText("Unknown")).toBeInTheDocument();
  });

  it("launches from the details view", async () => {
    const user = userEvent.setup();
    launchApp.mockResolvedValue({ outcome: "missingTarget", message: "file not found", commandPreview: "x" });
    render(<DetailsView appId="1" fixture={details} />);
    await user.click(screen.getByRole("button", { name: /^Launch$/ }));
    expect(launchApp).toHaveBeenCalledWith("1", []);
  });
});
