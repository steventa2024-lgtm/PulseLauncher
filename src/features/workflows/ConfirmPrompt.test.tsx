import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ConfirmPrompt } from "./ConfirmPrompt";

const confirmWorkflow = vi.hoisted(() => vi.fn(async () => undefined));
const handlers = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());

vi.mock("../../lib/api", () => ({
  api: { confirmWorkflow },
  errorText: (error: unknown) => (error instanceof Error ? error.message : String(error)),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
    handlers.set(event, handler);
    return () => handlers.delete(event);
  }),
}));

describe("ConfirmPrompt", () => {
  it("answers a confirmation from any window and closes when another window already did", async () => {
    confirmWorkflow.mockClear();
    const user = userEvent.setup();
    render(<ConfirmPrompt />);
    await act(async () => {
      handlers.get("workflow-confirm")?.({ payload: { workflowId: "w1", message: "Open the folder?" } });
    });
    expect(await screen.findByRole("dialog", { name: "Confirm this step" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(confirmWorkflow).toHaveBeenCalledWith("w1", true);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await act(async () => {
      handlers.get("workflow-confirm")?.({ payload: { workflowId: "w1", message: "Open the folder?" } });
    });
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    await act(async () => {
      handlers.get("workflow-confirm-settled")?.({ payload: { workflowId: "w1" } });
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });
});
