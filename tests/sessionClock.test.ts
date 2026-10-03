import { describe, expect, it } from "vitest";
import { allowedActions, displayRemaining } from "../src/utils/sessionClock";

describe("displayRemaining", () => {
  it("counts down only while running", () => {
    expect(displayRemaining({ status: "RUNNING", remainingSeconds: 100 }, 0, 30_500)).toBe(70);
    expect(displayRemaining({ status: "PAUSED", remainingSeconds: 100 }, 0, 30_500)).toBe(100);
    expect(displayRemaining({ status: "WAITING", remainingSeconds: null }, 0, 30_500)).toBeNull();
  });
  it("never goes negative", () => {
    expect(displayRemaining({ status: "RUNNING", remainingSeconds: 5 }, 0, 60_000)).toBe(0);
  });
});

describe("allowedActions", () => {
  it("matches the server lifecycle", () => {
    expect(allowedActions("WAITING")).toEqual({ start: true, pause: false, resume: false, end: true });
    expect(allowedActions("RUNNING")).toEqual({ start: false, pause: true, resume: false, end: true });
    expect(allowedActions("PAUSED")).toEqual({ start: false, pause: false, resume: true, end: true });
    expect(allowedActions("ENDED")).toEqual({ start: false, pause: false, resume: false, end: false });
  });
});
