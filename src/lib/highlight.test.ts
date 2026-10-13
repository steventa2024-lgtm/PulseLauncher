import { describe, expect, it } from "vitest";
import { highlightParts } from "./highlight";

describe("highlightParts", () => {
  it("groups consecutive hits", () => {
    expect(highlightParts("PulseCut", [0, 1, 2])).toEqual([
      { text: "Pul", hit: true },
      { text: "seCut", hit: false },
    ]);
  });

  it("ignores out of range indexes", () => {
    expect(highlightParts("ab", [9])).toEqual([{ text: "ab", hit: false }]);
  });
});
