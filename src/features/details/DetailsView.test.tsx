import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { AppDetails } from "../../lib/types";
import { DetailsView } from "./DetailsView";

const getApp = vi.hoisted(() => vi.fn());

vi.mock("../../lib/api", () => ({
  api: { getApp },
  errorText: (error: unknown) => String(error),
}));

const details: AppDetails = {
  app: {
    id: "app-1",
    name: "Registered Tool",
    originalName: "Registered Tool",
    source: "manual",
    target: { kind: "executable", launchTarget: "C:\\Tools\\tool.exe", resolvedTarget: null, arguments: [], linkArguments: null, workingDir: null },
    iconPath: null,
    iconIndex: 0,
    favorite: false,
    hidden: false,
    portable: true,
    category: "Developer Tools",
    tags: [],
    collectionIds: [],
    status: "ok",
    metadata: { version: null, publisher: null, description: null, productName: null, fileSize: null, modifiedAt: null },
    launchCount: 0,
    lastLaunchedAt: null,
    sortOrder: 0,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    lastSeenAt: null,
    previewAssetId: null,
  },
  events: [],
  previews: [],
};

describe("DetailsView", () => {
  it("says there is no preview until one is imported", async () => {
    getApp.mockResolvedValue(details);
    const user = userEvent.setup();
    render(
      <DetailsView id="app-1" collections={[]} icon={null} onBack={() => undefined} onChanged={() => undefined} onNotice={() => undefined} />,
    );
    expect(await screen.findByRole("heading", { name: "Registered Tool" })).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Preview" }));
    expect(screen.getByRole("heading", { name: "No preview available" })).toBeInTheDocument();
    expect(screen.getByText(/does not capture other windows/)).toBeInTheDocument();
  });
});
