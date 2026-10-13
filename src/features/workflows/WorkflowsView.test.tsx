import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WorkflowsView } from "./WorkflowsView";
import type { WorkflowCard } from "../../lib/types";

vi.mock("../../lib/api", () => ({
  listWorkflows: vi.fn(),
  runWorkflow: vi.fn(),
  saveWorkflow: vi.fn(),
  resetTemplate: vi.fn(),
  cancelWorkflow: vi.fn(),
  errorText: (e: unknown) => String(e),
}));

const card: WorkflowCard = {
  definition: {
    id: "tpl.zeropulse_stack",
    kind: "workflow",
    name: "Launch ZeroPulse Stack",
    description: "Launches registered ZeroPulse apps.",
    templateId: "tpl.zeropulse_stack",
    params: [],
    values: {},
    prerequisites: [],
    steps: [{ id: "stack", label: "Launch collection", action: { type: "launchCollection" }, retries: 0 }],
    enabled: true,
  },
  status: {
    state: "needsSetup",
    prerequisites: [{ label: "ZeroPulse collection contains registered apps", ok: false, detail: "no apps registered in this collection yet" }],
    lastRun: null,
  },
};

describe("WorkflowsView", () => {
  it("does not mark an unconfigured template as working", () => {
    render(<WorkflowsView initial={[card]} />);
    expect(screen.getAllByText("Needs setup").length).toBeGreaterThan(0);
    expect(screen.queryByText("Working")).not.toBeInTheDocument();
    expect(screen.getByText(/no apps registered/)).toBeInTheDocument();
  });
});
