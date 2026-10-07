import { describe, expect, it } from "vitest";
import { difficulty, duration, pct, resultText, sortRows } from "../src/features/results/format";
import type { ResultRow } from "../src/types/results";

const row = (o: Partial<ResultRow>): ResultRow => ({
  attemptId: "a", studentNumber: "1", name: "Ana", status: "SUBMITTED", startedAt: null, submittedAt: null, score: null, totalPoints: null,
  percentage: null, passed: null, answered: 0, timeTakenSeconds: null, focusLostCount: 0, focusLostMs: 0, disconnectCount: 0, ...o,
});

describe("results formatting", () => {
  it("formats percentages and durations", () => {
    expect(pct(null)).toBe("—");
    expect(pct(66.666)).toBe("66.7%");
    expect(pct(100)).toBe("100%");
    expect(duration(null)).toBe("—");
    expect(duration(45)).toBe("45 s");
    expect(duration(125)).toBe("2 min 5 s");
    expect(duration(3720)).toBe("1 h 2 min");
  });
  it("labels results", () => {
    expect([resultText(true), resultText(false), resultText(null)]).toEqual(["Passed", "Not passed", "—"]);
  });
  it("reads question difficulty from percent correct", () => {
    expect(difficulty({ attempts: 0, percentCorrect: 0 })).toBe("none");
    expect(difficulty({ attempts: 5, percentCorrect: 90 })).toBe("easy");
    expect(difficulty({ attempts: 5, percentCorrect: 50 })).toBe("medium");
    expect(difficulty({ attempts: 5, percentCorrect: 10 })).toBe("hard");
  });
  it("sorts by name, percentage and time, always leaving unfinished students last", () => {
    const rows = [row({ name: "Zed", percentage: 50 }), row({ name: "ana", percentage: 90 }), row({ name: "Bo", percentage: null })];
    expect(sortRows(rows, "name").map((r) => r.name)).toEqual(["ana", "Bo", "Zed"]);
    expect(sortRows(rows, "percentage", -1).map((r) => r.name)).toEqual(["ana", "Zed", "Bo"]);
    expect(sortRows(rows, "percentage", 1).map((r) => r.name)).toEqual(["Zed", "ana", "Bo"]);
    expect(sortRows(rows, "time").map((r) => r.name).length).toBe(3);
    expect(rows[0].name).toBe("Zed"); // input untouched
  });
});
