import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { SearchResult } from "../../lib/types";
import { Launcher } from "./Launcher";

const search = vi.hoisted(() => vi.fn());
const launchApp = vi.hoisted(() => vi.fn());
const hideLauncher = vi.hoisted(() => vi.fn());
const appIcons = vi.hoisted(() => vi.fn());

vi.mock("../../lib/api", () => ({
  api: { search, launchApp, hideLauncher, appIcons, runWorkflow: vi.fn(), openIndexed: vi.fn(), copyText: vi.fn(), openUrl: vi.fn() },
  errorText: (error: unknown) => (error instanceof Error ? error.message : String(error)),
}));

function result(partial: Partial<SearchResult> & Pick<SearchResult, "key" | "title" | "appId">): SearchResult {
  return {
    kind: "app",
    subtitle: "Program",
    score: 1,
    highlights: [],
    workflowId: null,
    path: null,
    value: null,
    favorite: false,
    missing: false,
    iconKey: null,
    ...partial,
  };
}

describe("Launcher", () => {
  it("keeps the highlighted row when moving with the keyboard", async () => {
    search.mockResolvedValue([
      result({ key: "a", title: "Alpha", appId: "1" }),
      result({ key: "b", title: "Beta", appId: "2" }),
    ]);
    launchApp.mockResolvedValue({ accepted: true, processId: null, message: "Started", executed: "Alpha" });
    hideLauncher.mockResolvedValue(undefined);
    appIcons.mockResolvedValue([]);
    const user = userEvent.setup();
    render(<Launcher />);
    expect(await screen.findByRole("option", { name: /Alpha/ })).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowDown}{Enter}");
    expect(launchApp).toHaveBeenCalledWith("2");
    expect(hideLauncher).toHaveBeenCalled();
  });

  it("explains an empty catalog without filling it", async () => {
    search.mockResolvedValue([]);
    render(<Launcher />);
    expect(await screen.findByText(/Favorites and recent apps appear here/)).toBeInTheDocument();
  });
});
