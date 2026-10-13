import { describe, expect, it } from "vitest";
import { formatBytes, moveSelection, readinessLabel } from "./text";

describe("moveSelection", () => {
  it("stays inside the list", () => {
    expect(moveSelection(0, -1, 3)).toBe(0);
    expect(moveSelection(2, 1, 3)).toBe(2);
    expect(moveSelection(1, 1, 3)).toBe(2);
    expect(moveSelection(4, 1, 0)).toBe(0);
  });
});

describe("readinessLabel", () => {
  it("uses the three honest states", () => {
    expect(readinessLabel("ready")).toBe("Ready");
    expect(readinessLabel("needs_setup")).toBe("Needs setup");
    expect(readinessLabel("unavailable")).toBe("Unavailable");
  });
});

describe("formatBytes", () => {
  it("says when size is unknown", () => {
    expect(formatBytes(null)).toBe("Not available");
    expect(formatBytes(512)).toBe("512 B");
  });
});
