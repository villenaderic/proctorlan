import { describe, expect, it } from "vitest";
import { flagsFor, formatSpan, sortForAttention, timelineFor } from "../src/features/sessions/proctoring";
import type { RosterRow, SessionEvent } from "../src/types/session";

const row = (o: Partial<RosterRow>): RosterRow => ({
  attemptId: "a", studentNumber: "1", name: "Ana", status: "IN_PROGRESS", joinedAt: "", submittedAt: null, answered: 0,
  percentage: null, passed: null, online: true, focusLostCount: 0, focusLostMs: 0, disconnectCount: 0, ...o,
});
const ev = (id: string, attemptId: string): SessionEvent => ({ id, attemptId, studentName: "x", studentNumber: "1", eventType: "FOCUS_LOST", description: "", metadata: null, createdAt: "" });

describe("proctoring view helpers", () => {
  it("formats spans", () => {
    expect(formatSpan(45_000)).toBe("45 s");
    expect(formatSpan(125_000)).toBe("2 min 5 s");
    expect(formatSpan(180_000)).toBe("3 min");
    expect(formatSpan(3_720_000)).toBe("1 h 2 min");
  });
  it("builds flags only when something happened", () => {
    expect(flagsFor(row({}))).toEqual([]);
    const f = flagsFor(row({ focusLostCount: 3, focusLostMs: 45_000, disconnectCount: 1 }));
    expect(f.map((x) => x.text)).toEqual(["Left window ×3 (45 s)", "Disconnected ×1"]);
    expect(flagsFor(row({ focusLostCount: 6 }))[0].tone).toBe("red");
  });
  it("floats offline in-progress and flagged students to the top, without mutating the input", () => {
    const rows = [row({ attemptId: "1", name: "Zed" }), row({ attemptId: "2", name: "Bo", focusLostCount: 2 }), row({ attemptId: "3", name: "Cy", online: false })];
    const sorted = sortForAttention(rows);
    expect(sorted.map((r) => r.attemptId)).toEqual(["3", "2", "1"]);
    expect(rows[0].attemptId).toBe("1");
  });
  it("timeline is one student's events, oldest first", () => {
    const feed = [ev("3", "a"), ev("2", "b"), ev("1", "a")];
    expect(timelineFor(feed, "a").map((e) => e.id)).toEqual(["1", "3"]);
  });
});
