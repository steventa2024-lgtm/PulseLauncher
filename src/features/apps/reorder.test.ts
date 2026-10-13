import { describe, expect, it } from "vitest";
import { reorderVisible } from "./reorder";

describe("reorderVisible", () => {
  it("lands after the target when dragging downward", () => {
    expect(reorderVisible(["a", "b", "c", "d"], ["a", "b", "c", "d"], "b", "d")).toEqual(["a", "c", "d", "b"]);
  });

  it("lands before the target when dragging upward", () => {
    expect(reorderVisible(["a", "b", "c", "d"], ["a", "b", "c", "d"], "d", "b")).toEqual(["a", "d", "b", "c"]);
  });

  it("swaps adjacent neighbors", () => {
    expect(reorderVisible(["a", "b", "c"], ["a", "b", "c"], "b", "c")).toEqual(["a", "c", "b"]);
    expect(reorderVisible(["a", "b", "c"], ["a", "b", "c"], "c", "b")).toEqual(["a", "c", "b"]);
  });

  it("keeps apps outside the current filter in place", () => {
    expect(reorderVisible(["a", "b", "c", "d", "e"], ["a", "c", "e"], "e", "a")).toEqual(["e", "b", "a", "d", "c"]);
    expect(reorderVisible(["a", "b", "c", "d", "e"], ["a", "c", "e"], "a", "e")).toEqual(["c", "b", "e", "d", "a"]);
  });

  it("returns the same ids when the drop is a no-op", () => {
    expect(reorderVisible(["a", "b"], ["a", "b"], "a", "a")).toEqual(["a", "b"]);
    expect(reorderVisible(["a", "b"], ["a", "b"], "missing", "a")).toEqual(["a", "b"]);
  });
});
