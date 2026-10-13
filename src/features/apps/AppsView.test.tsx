import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppEntry } from "../../lib/types";
import { AppsView } from "./AppsView";

const reorderApps = vi.hoisted(() => vi.fn(async () => undefined));

vi.mock("../../lib/api", () => ({
  api: { reorderApps },
  errorText: (error: unknown) => String(error),
}));

function app(id: string, name: string): AppEntry {
  return {
    id,
    name,
    originalName: name,
    source: "manual",
    target: {
      kind: "executable",
      launchTarget: `C:\\apps\\${id}.exe`,
      resolvedTarget: null,
      arguments: [],
      linkArguments: null,
      workingDir: null,
    },
    iconPath: null,
    iconIndex: 0,
    favorite: false,
    hidden: false,
    portable: true,
    category: null,
    tags: [],
    collectionIds: [],
    status: "ok",
    metadata: {
      version: null,
      publisher: null,
      description: null,
      productName: null,
      fileSize: null,
      modifiedAt: null,
    },
    launchCount: 0,
    lastLaunchedAt: null,
    sortOrder: 0,
    createdAt: "",
    updatedAt: "",
    lastSeenAt: null,
    previewAssetId: null,
  };
}

describe("AppsView", () => {
  beforeEach(() => {
    reorderApps.mockClear();
  });

  it("does not invent apps when the catalog is empty", () => {
    render(
      <AppsView apps={[]} collections={[]} icons={{}} onOpen={() => undefined} onChanged={() => undefined} onNotice={() => undefined} />,
    );
    expect(screen.getByText("No apps yet")).toBeInTheDocument();
    expect(screen.getByText(/Nothing is invented/)).toBeInTheDocument();
  });

  it("sends the dragged order to the catalog", () => {
    render(
      <AppsView
        apps={[app("1", "One"), app("2", "Two")]}
        collections={[]}
        icons={{}}
        onOpen={() => undefined}
        onChanged={() => undefined}
        onNotice={() => undefined}
      />,
    );
    fireEvent.dragStart(screen.getByRole("button", { name: "Drag Two" }));
    const one = screen.getByRole("heading", { name: "One" }).closest("article");
    expect(one).not.toBeNull();
    fireEvent.dragOver(one!);
    fireEvent.drop(one!);
    expect(reorderApps).toHaveBeenCalledWith(["2", "1"]);
  });

  it("moves the focused app with the arrow keys", () => {
    render(
      <AppsView
        apps={[app("1", "One"), app("2", "Two")]}
        collections={[]}
        icons={{}}
        onOpen={() => undefined}
        onChanged={() => undefined}
        onNotice={() => undefined}
      />,
    );
    fireEvent.keyDown(screen.getByRole("button", { name: "Drag One" }), { key: "ArrowDown" });
    expect(reorderApps).toHaveBeenCalledWith(["2", "1"]);
  });
});
