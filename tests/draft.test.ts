import { describe, expect, it } from "vitest";
import {
  addChoice, addQuestion, changeType, deleteQuestion, duplicateQuestion, fromExam, issuesByQuestion, MAX_CHOICES,
  moveQuestion, newDraft, newQuestion, setCorrect, toPayload, totalPoints, updateChoice, updateQuestion,
} from "../src/features/exams/draft";
import type { ExamFull } from "../src/types/exam";

const withThree = () => {
  let d = newDraft();
  for (const t of ["multiple_choice", "true_false", "identification"] as const) d = addQuestion(d, t);
  return d;
};

describe("question defaults", () => {
  it("creates sensible choices per type", () => {
    expect(newQuestion("multiple_choice").choices).toHaveLength(4);
    expect(newQuestion("true_false").choices.map((c) => c.choiceText)).toEqual(["True", "False"]);
    expect(newQuestion("identification").choices.every((c) => c.isCorrect)).toBe(true);
  });
  it("never pre-selects a correct answer for choice questions", () => {
    expect(newQuestion("multiple_choice").choices.some((c) => c.isCorrect)).toBe(false);
    expect(newQuestion("true_false").choices.some((c) => c.isCorrect)).toBe(false);
  });
});

describe("correct-answer semantics", () => {
  it("multiple choice is exclusive", () => {
    let q = newQuestion("multiple_choice");
    q = setCorrect(q, q.choices[0].uid, true);
    q = setCorrect(q, q.choices[2].uid, true);
    expect(q.choices.filter((c) => c.isCorrect).map((c) => c.uid)).toEqual([q.choices[2].uid]);
  });
  it("multiple select allows several and can untick", () => {
    let q = newQuestion("multiple_select");
    q = setCorrect(q, q.choices[0].uid, true);
    q = setCorrect(q, q.choices[1].uid, true);
    expect(q.choices.filter((c) => c.isCorrect)).toHaveLength(2);
    q = setCorrect(q, q.choices[0].uid, false);
    expect(q.choices.filter((c) => c.isCorrect)).toHaveLength(1);
  });
});

describe("changing type", () => {
  it("multiple select -> multiple choice keeps only the first correct answer", () => {
    let q = newQuestion("multiple_select");
    q = updateChoice(q, q.choices[0].uid, "A");
    q = setCorrect(setCorrect(q, q.choices[0].uid, true), q.choices[1].uid, true);
    const mc = changeType(q, "multiple_choice");
    expect(mc.choices.filter((c) => c.isCorrect)).toHaveLength(1);
    expect(mc.choices[0].choiceText).toBe("A");
  });
  it("to identification keeps the correct texts as accepted answers", () => {
    let q = newQuestion("multiple_choice");
    q = updateChoice(updateChoice(q, q.choices[0].uid, "Paris"), q.choices[1].uid, "Rome");
    q = setCorrect(q, q.choices[0].uid, true);
    const id = changeType(q, "identification");
    expect(id.choices.map((c) => c.choiceText)).toEqual(["Paris"]);
    expect(id.choices[0].isCorrect).toBe(true);
  });
  it("to true/false resets choices; same type is a no-op", () => {
    const q = newQuestion("multiple_choice");
    expect(changeType(q, "true_false").choices.map((c) => c.choiceText)).toEqual(["True", "False"]);
    expect(changeType(q, "multiple_choice")).toBe(q);
  });
});

describe("question list operations", () => {
  it("adds, moves, duplicates and deletes", () => {
    let d = withThree();
    const [a, b, c] = d.questions.map((q) => q.uid);
    d = moveQuestion(d, a, 1);
    expect(d.questions.map((q) => q.uid)).toEqual([b, a, c]);
    d = moveQuestion(d, b, -1); // already first: unchanged
    expect(d.questions[0].uid).toBe(b);
    d = moveQuestion(d, c, 1); // already last: unchanged
    expect(d.questions[2].uid).toBe(c);
    d = duplicateQuestion(d, a);
    expect(d.questions).toHaveLength(4);
    expect(d.questions[2].uid).not.toBe(a);
    expect(d.questions[2].choices[0].uid).not.toBe(d.questions[1].choices[0].uid);
    d = deleteQuestion(d, a);
    expect(d.questions.map((q) => q.uid)).not.toContain(a);
  });
  it("duplicate is independent of the original", () => {
    let d = addQuestion(newDraft());
    d = duplicateQuestion(d, d.questions[0].uid);
    d = updateQuestion(d, d.questions[1].uid, (q) => ({ ...q, questionText: "changed" }));
    expect(d.questions[0].questionText).toBe("");
  });
  it("caps choices", () => {
    let q = newQuestion("multiple_choice");
    for (let i = 0; i < 20; i++) q = addChoice(q);
    expect(q.choices).toHaveLength(MAX_CHOICES);
  });
  it("sums points", () => {
    let d = withThree();
    d = updateQuestion(d, d.questions[0].uid, (q) => ({ ...q, points: 2.5 }));
    expect(totalPoints(d)).toBe(4.5);
  });
});

describe("payload mapping", () => {
  it("strips uids and nulls blank explanations", () => {
    const d = addQuestion(newDraft());
    const p = toPayload(d) as { questions: Array<Record<string, unknown>> };
    expect(JSON.stringify(p)).not.toContain("uid");
    expect(p.questions[0].explanation).toBeNull();
    expect(p).toHaveProperty("durationMinutes", 60);
  });
  it("round-trips through the backend shape", () => {
    const full = {
      id: "e1", title: "T", description: "", instructions: "", durationMinutes: 30, passingScore: 50,
      randomizeQuestions: true, randomizeChoices: false, allowReview: true, autoSubmit: true, showResults: false,
      status: "inactive", createdAt: "", updatedAt: "", sessionCount: 0,
      questions: [{ id: "q1", questionText: "Q", questionType: "multiple_choice", points: 2, sortOrder: 0, required: true, explanation: null,
        choices: [{ id: "c1", questionId: "q1", choiceText: "A", isCorrect: true, sortOrder: 0 }, { id: "c2", questionId: "q1", choiceText: "B", isCorrect: false, sortOrder: 1 }] }],
    } as ExamFull;
    const d = fromExam(full);
    expect(d.questions[0].choices[0]).toMatchObject({ choiceText: "A", isCorrect: true });
    expect(toPayload(d)).toMatchObject({ title: "T", durationMinutes: 30, randomizeQuestions: true });
  });
});

describe("issue grouping", () => {
  it("groups by question index and ignores exam-level issues", () => {
    const m = issuesByQuestion([{ path: "title" }, { path: "questions.1.text" }, { path: "questions.1.correct" }, { path: "questions.0.points" }, { path: "questions" }]);
    expect(m.get(1)).toHaveLength(2);
    expect(m.get(0)).toHaveLength(1);
    expect(m.size).toBe(2);
  });
});
