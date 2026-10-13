import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { WorkflowView } from "../../lib/types";
import { WorkflowsView } from "./WorkflowsView";

const listWorkflows = vi.hoisted(() => vi.fn());
const environment = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));

vi.mock("../../lib/api", () => ({
  api: { listWorkflows, environment },
  errorText: (error: unknown) => String(error),
}));

const item: WorkflowView = {
  schema: 1,
  id: "template:open-settings",
  name: "Open Settings",
  description: "Opens Windows Settings.",
  whatItDoes: "Opens the Settings app.",
  prerequisites: "Windows.",
  kind: "command",
  templateKey: "open-settings",
  templateVersion: 1,
  params: [],
  values: {},
  steps: [],
  hotkey: null,
  trusted: true,
  readiness: { state: "unavailable", details: ["Windows Settings is not available on this computer."] },
  lastRun: null,
};

describe("WorkflowsView", () => {
  it("shows unavailable instead of a successful run", async () => {
    listWorkflows.mockResolvedValue([item]);
    environment.mockResolvedValue({ isWindows: false, downloads: null, documents: null, taskManager: null, terminals: [] });
    render(<WorkflowsView apps={[]} onNotice={() => undefined} />);
    expect(await screen.findByText("Unavailable")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run" })).toBeDisabled();
  });
});
