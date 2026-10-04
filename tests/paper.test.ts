import { describe, expect, it } from "vitest";
import { answeredCount, createSequencer, isAnswered, mergeAnswers, timerAnnouncement, timerTone, unansweredIndexes } from "../src/features/student/paper";
import type { Paper } from "../src/types/paper";

const paper: Paper = {
  allowReview: true, totalPoints: 3,
  questions: ["a", "b", "c"].map((id) => ({ id, text: id, type: "multiple_choice" as const, points: 1, required: true, choices: [] })),
  answers: {},
};

describe("answered detection", () => {
  it("treats blanks, whitespace and empty selections as unanswered", () => {
    expect(isAnswered(undefined)).toBe(false);
    expect(isAnswered("")).toBe(false);
    expect(isAnswered("   ")).toBe(false);
    expect(isAnswered([])).toBe(false);
    expect(isAnswered("x")).toBe(true);
    expect(isAnswered(["c1"])).toBe(true);
  });
  it("counts and lists unanswered questions", () => {
    const answers = { a: "c1", b: "", c: [] as string[] };
    expect(answeredCount(paper, answers)).toBe(1);
    expect(unansweredIndexes(paper, answers)).toEqual([1, 2]);
  });
});

describe("sequencer", () => {
  it("always increases, even if the clock stalls or goes backwards", () => {
    let t = 1000;
    const next = createSequencer(() => t);
    const a = next(); const b = next(); t = 500; const c = next();
    expect(b).toBeGreaterThan(a);
    expect(c).toBeGreaterThan(b);
  });
  it("jumps forward with real time so a restart still beats earlier answers", () => {
    let t = 1000;
    const next = createSequencer(() => t);
    next(); t = 5000;
    expect(next()).toBe(5000);
  });
});

describe("mergeAnswers", () => {
  it("lets unacknowledged local answers win over the server's copy", () => {
    expect(mergeAnswers({ a: "old", b: "keep" }, { a: { answer: "new" } })).toEqual({ a: "new", b: "keep" });
  });
});

describe("timer helpers", () => {
  it("changes tone near the end", () => {
    expect(timerTone(null)).toBe("normal");
    expect(timerTone(3600)).toBe("normal");
    expect(timerTone(300)).toBe("warning");
    expect(timerTone(60)).toBe("danger");
  });
  it("announces only the milestones", () => {
    expect(timerAnnouncement(3000)).toBeNull();
    expect(timerAnnouncement(240)).toBe("Five minutes remaining.");
    expect(timerAnnouncement(30)).toBe("One minute remaining.");
    expect(timerAnnouncement(0)).toBe("Time is up.");
  });
});
