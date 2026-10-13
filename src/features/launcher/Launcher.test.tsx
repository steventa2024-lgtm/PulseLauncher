import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SearchResult } from "../../lib/types";
import { Launcher } from "./Launcher";

const search = vi.hoisted(() => vi.fn());
const launchApp = vi.hoisted(() => vi.fn());
const hideLauncher = vi.hoisted(() => vi.fn());
const appIcons = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
  emit: vi.fn(async () => undefined),
}));

vi.mock("../../lib/api", () => ({
  api: { search, launchApp, hideLauncher, appIcons, runWorkflow: vi.fn(), openIndexed: vi.fn(), copyText: vi.fn(), openUrl: vi.fn(), getPreferences: vi.fn(async () => { throw new Error("no bridge"); }), showMain: vi.fn() },
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

  it("keeps the same result selected when a newer list still contains it", async () => {
    search.mockImplementation(async (query: string) => query
      ? [result({ key: "b", title: "Beta", appId: "2" }), result({ key: "g", title: "Gamma", appId: "3" })]
      : [result({ key: "a", title: "Alpha", appId: "1" }), result({ key: "b", title: "Beta", appId: "2" })]);
    appIcons.mockResolvedValue([]);
    const user = userEvent.setup();
    render(<Launcher />);
    expect(await screen.findByRole("option", { name: /Alpha/ })).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowDown}");
    await user.type(screen.getByLabelText("Search PulseLauncher"), "z");
    expect(await screen.findByRole("option", { name: /Beta/ })).toHaveAttribute("aria-selected", "true");
  });

  it("ignores a search result that arrives after a newer query", async () => {
    vi.useFakeTimers();
    let resolveStale: (rows: SearchResult[]) => void = () => undefined;
    search.mockImplementation((query: string) => {
      if (query === "") {
        return Promise.resolve([
          result({ key: "a", title: "Alpha", appId: "1" }),
          result({ key: "b", title: "Beta", appId: "2" }),
        ]);
      }
      if (query === "b") return new Promise<SearchResult[]>((resolve) => { resolveStale = resolve; });
      return Promise.resolve([
        result({ key: "b", title: "Beta", appId: "2" }),
        result({ key: "g", title: "Gamma", appId: "3" }),
      ]);
    });
    appIcons.mockResolvedValue([]);
    render(<Launcher />);
    await act(async () => { await vi.advanceTimersByTimeAsync(40); });
    fireEvent.keyDown(screen.getByLabelText("Search PulseLauncher"), { key: "ArrowDown" });
    fireEvent.change(screen.getByLabelText("Search PulseLauncher"), { target: { value: "b" } });
    await act(async () => { await vi.advanceTimersByTimeAsync(40); });
    fireEvent.change(screen.getByLabelText("Search PulseLauncher"), { target: { value: "be" } });
    await act(async () => { await vi.advanceTimersByTimeAsync(40); });
    expect(screen.getByRole("option", { name: /Beta/ })).toHaveAttribute("aria-selected", "true");
    await act(async () => { resolveStale([result({ key: "a", title: "Alpha", appId: "1" })]); });
    expect(screen.getByRole("option", { name: /Beta/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByRole("option", { name: /Alpha/ })).not.toBeInTheDocument();
  });
});

afterEach(() => {
  vi.useRealTimers();
});
