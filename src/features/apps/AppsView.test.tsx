import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AppsView } from "./AppsView";

vi.mock("../../lib/api", () => ({
  api: {},
  errorText: (error: unknown) => String(error),
}));

describe("AppsView", () => {
  it("does not invent apps when the catalog is empty", () => {
    render(
      <AppsView apps={[]} collections={[]} icons={{}} onOpen={() => undefined} onChanged={() => undefined} onNotice={() => undefined} />,
    );
    expect(screen.getByText("No apps yet")).toBeInTheDocument();
    expect(screen.getByText(/Nothing is invented/)).toBeInTheDocument();
  });
});
