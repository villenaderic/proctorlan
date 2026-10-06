import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FocusTracker, isAway, type FocusReport } from "../src/features/student/focus";

let clock = 0;
let reports: FocusReport[];
let returns: number[];
const make = (debounceMs = 300) => new FocusTracker({ report: (r) => reports.push(r), onReturn: (ms) => returns.push(ms), debounceMs, now: () => clock });
const advance = (ms: number) => { clock += ms; vi.advanceTimersByTime(ms); };

beforeEach(() => { vi.useFakeTimers(); clock = 0; reports = []; returns = []; });
afterEach(() => vi.useRealTimers());

describe("FocusTracker", () => {
  it("ignores a flicker shorter than the debounce", () => {
    const t = make();
    t.setAway(true); advance(200); t.setAway(false); advance(1000);
    expect(reports).toEqual([]);
    expect(returns).toEqual([]);
  });

  it("reports loss after the debounce and the restore with the full absence", () => {
    const t = make();
    t.setAway(true); advance(300);
    expect(reports).toEqual([{ type: "FOCUS_LOST" }]);
    advance(11_700); t.setAway(false);
    expect(reports[1]).toEqual({ type: "FOCUS_RESTORED", lostForMs: 12_000 });
    expect(returns).toEqual([12_000]);
  });

  it("treats repeated blur/hidden signals as one absence", () => {
    const t = make();
    t.setAway(true); t.setAway(true); advance(300); t.setAway(true); advance(500);
    t.setAway(false); t.setAway(false);
    expect(reports.map((r) => r.type)).toEqual(["FOCUS_LOST", "FOCUS_RESTORED"]);
  });

  it("can report several separate absences", () => {
    const t = make();
    for (let i = 0; i < 3; i++) { t.setAway(true); advance(1000); t.setAway(false); advance(50); }
    expect(reports.filter((r) => r.type === "FOCUS_LOST")).toHaveLength(3);
    expect(reports.filter((r) => r.type === "FOCUS_RESTORED")).toHaveLength(3);
  });

  it("dispose while away closes an open absence and stays quiet otherwise", () => {
    const t = make();
    t.setAway(true); advance(2000); t.dispose();
    expect(reports.map((r) => r.type)).toEqual(["FOCUS_LOST", "FOCUS_RESTORED"]);
    reports = [];
    const u = make();
    u.setAway(true); advance(100); u.dispose(); advance(1000); // inside debounce: nothing was reported, so nothing to close
    expect(reports).toEqual([]);
  });

  it("isAway is true when hidden or unfocused", () => {
    expect(isAway({ hidden: false, hasFocus: () => true })).toBe(false);
    expect(isAway({ hidden: true, hasFocus: () => true })).toBe(true);
    expect(isAway({ hidden: false, hasFocus: () => false })).toBe(true);
  });
});
