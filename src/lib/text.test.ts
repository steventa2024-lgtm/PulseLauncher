import { describe, expect, it } from "vitest";
import { argumentLines, formatBytes, highlightParts, matchesZeropulse, moveSelection, retainSelection, statusLabel } from "./text";

describe("search selection", () => {
  it("keeps the selected id when results reorder", () => {
    expect(retainSelection(["b", "a", "c"], "a")).toBe("a");
    expect(retainSelection(["b", "c"], "a")).toBe("b");
    expect(retainSelection([], "a")).toBeNull();
  });

  it("moves within the current order", () => {
    expect(moveSelection(["a", "b", "c"], "a", 1)).toBe("b");
    expect(moveSelection(["a", "b", "c"], "a", -1)).toBe("a");
  });
});

describe("highlight", () => {
  it("marks character indexes without splitting a match", () => {
    expect(highlightParts("Apps", [0, 2])).toEqual([
      { text: "A", hit: true },
      { text: "p", hit: false },
      { text: "p", hit: true },
      { text: "s", hit: false },
    ]);
  });
});

describe("labels", () => {
  it("does not call a missing prerequisite working", () => {
    expect(statusLabel("needsSetup")).toBe("Needs setup");
    expect(statusLabel("missing")).toBe("Missing");
  });

  it("formats unknown sizes honestly", () => {
    expect(formatBytes(null)).toBe("Size unavailable");
    expect(formatBytes(512)).toBe("512 B");
  });

  it("splits arguments into an array", () => {
    expect(argumentLines(" --one \n\n --two ")).toEqual(["--one", "--two"]);
  });

  it("detects real ZeroPulse name matches only", () => {
    const app = {
      id: "1",
      displayName: "Calculator",
      publisher: null,
      launchTarget: "/usr/bin/gnome-calculator",
      tags: [],
    };
    expect(matchesZeropulse(app, new Set())).toBe(false);
    expect(matchesZeropulse({ ...app, displayName: "PulseCut" }, new Set())).toBe(true);
  });
});
