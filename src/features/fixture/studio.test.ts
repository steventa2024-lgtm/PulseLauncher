import { describe, expect, it } from "vitest";
import { fixtureSearch, studioFixtureEnabled } from "./studio";

describe("studio fixture", () => {
  it("stays off unless the screenshot query is present", () => {
    expect(studioFixtureEnabled()).toBe(false);
    expect(fixtureSearch("").map((row) => row.title)).toEqual([
      "PulseCut",
      "PulseFlow",
      "OpenPulse",
      "Cursor",
      "File Explorer",
    ]);
    expect(fixtureSearch("pulse").every((row) => row.subtitle.includes("fixture"))).toBe(true);
  });
});
