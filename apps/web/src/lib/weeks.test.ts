import { describe, expect, it } from "vitest";
import { weekKey, weeksEnding } from "./weeks";

describe("ISO weeks", () => {
  it("numbers weeks like the server", () => {
    expect(weekKey(new Date("2026-09-13T00:00:00Z"))).toBe("2026-W37");
    expect(weekKey(new Date("2027-01-01T00:00:00Z"))).toBe("2026-W53");
    expect(weekKey(new Date("2025-12-29T00:00:00Z"))).toBe("2026-W01");
  });

  it("fills a continuous range across a year boundary", () => {
    const weeks = weeksEnding("2027-W02", 4).map((w) => w.key);
    expect(weeks).toEqual(["2026-W52", "2026-W53", "2027-W01", "2027-W02"]);
  });
});
