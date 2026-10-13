import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { EmptyState, StatusBadge } from "./ui";
import { ResultList } from "../views/Launcher";
import type { SearchHit } from "../lib/types";

const hit = (id: string, title: string): SearchHit => ({
  id,
  kind: "app",
  title,
  subtitle: "",
  score: 1,
  indices: [0],
  appId: id,
  workflowId: null,
  path: null,
  calculatorValue: null,
  url: null,
});

describe("interface states", () => {
  it("shows an empty state without inventing apps", () => {
    render(<EmptyState title="No applications yet" body="Register an executable that exists on this machine." />);
    expect(screen.getByRole("heading", { name: "No applications yet" })).toBeTruthy();
    expect(screen.queryByText("PulseCut")).toBeNull();
  });

  it("labels templates that still need setup", () => {
    render(<StatusBadge status="needsSetup" />);
    expect(screen.getByText("Needs setup")).toBeTruthy();
  });

  it("keeps the selected result highlighted after a reorder", () => {
    const { rerender } = render(<ResultList hits={[hit("a", "Alpha"), hit("b", "Beta")]} selectedId="b" />);
    expect(document.querySelector('[data-selected="true"]')?.textContent).toContain("Beta");
    rerender(<ResultList hits={[hit("b", "Beta"), hit("a", "Alpha")]} selectedId="b" />);
    expect(document.querySelector('[data-selected="true"]')?.textContent).toContain("Beta");
  });
});
