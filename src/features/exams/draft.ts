//! Pure functions for editing an exam draft. No React here, so they are unit-tested directly.
import type { DraftChoice, DraftQuestion, ExamDraft, ExamFull, QuestionType } from "@/types/exam";
import { uid } from "@/utils/uid";

export const MAX_CHOICES = 10; // mirrors MAX_CHOICES_PER_QUESTION in src-tauri/src/config.rs

export const newChoice = (choiceText = "", isCorrect = false): DraftChoice => ({ uid: uid(), choiceText, isCorrect });

const blankChoices = (n: number) => Array.from({ length: n }, () => newChoice());

function defaultChoices(type: QuestionType): DraftChoice[] {
  switch (type) {
    case "true_false": return [newChoice("True"), newChoice("False")];
    case "identification": return [newChoice("", true)];
    default: return blankChoices(4);
  }
}

export function newQuestion(type: QuestionType = "multiple_choice"): DraftQuestion {
  return { uid: uid(), questionText: "", questionType: type, points: 1, required: true, explanation: "", choices: defaultChoices(type) };
}

export function newDraft(): ExamDraft {
  return {
    title: "", description: "", instructions: "", durationMinutes: 60, passingScore: 75,
    randomizeQuestions: false, randomizeChoices: false, allowReview: true, autoSubmit: true, showResults: false,
    questions: [],
  };
}

/** Switches a question's type, carrying over what makes sense and resetting the rest. */
export function changeType(q: DraftQuestion, type: QuestionType): DraftQuestion {
  if (q.questionType === type) return q;
  const filled = q.choices.filter((c) => c.choiceText.trim());
  let choices: DraftChoice[];
  switch (type) {
    case "true_false":
      choices = defaultChoices(type);
      break;
    case "identification": {
      const accepted = q.questionType === "multiple_choice" || q.questionType === "multiple_select"
        ? filled.filter((c) => c.isCorrect) : filled;
      choices = accepted.length ? accepted.map((c) => ({ ...c, isCorrect: true })) : defaultChoices(type);
      break;
    }
    case "multiple_choice":
    case "multiple_select": {
      const keep = q.questionType === "multiple_choice" || q.questionType === "multiple_select";
      choices = keep ? [...q.choices] : defaultChoices(type);
      if (type === "multiple_choice") {
        const first = choices.findIndex((c) => c.isCorrect);
        choices = choices.map((c, i) => ({ ...c, isCorrect: i === first }));
      }
      break;
    }
  }
  return { ...q, questionType: type, choices };
}

/** Radio semantics for multiple choice / true-false, checkbox semantics for multiple select. */
export function setCorrect(q: DraftQuestion, choiceUid: string, checked: boolean): DraftQuestion {
  const exclusive = q.questionType === "multiple_choice" || q.questionType === "true_false";
  return {
    ...q,
    choices: q.choices.map((c) => exclusive ? { ...c, isCorrect: c.uid === choiceUid } : c.uid === choiceUid ? { ...c, isCorrect: checked } : c),
  };
}

export function addChoice(q: DraftQuestion): DraftQuestion {
  if (q.choices.length >= MAX_CHOICES) return q;
  return { ...q, choices: [...q.choices, newChoice("", q.questionType === "identification")] };
}

export const removeChoice = (q: DraftQuestion, choiceUid: string): DraftQuestion =>
  ({ ...q, choices: q.choices.filter((c) => c.uid !== choiceUid) });

export const updateChoice = (q: DraftQuestion, choiceUid: string, text: string): DraftQuestion =>
  ({ ...q, choices: q.choices.map((c) => (c.uid === choiceUid ? { ...c, choiceText: text } : c)) });

export const addQuestion = (d: ExamDraft, type: QuestionType = "multiple_choice"): ExamDraft =>
  ({ ...d, questions: [...d.questions, newQuestion(type)] });

export const updateQuestion = (d: ExamDraft, questionUid: string, fn: (q: DraftQuestion) => DraftQuestion): ExamDraft =>
  ({ ...d, questions: d.questions.map((q) => (q.uid === questionUid ? fn(q) : q)) });

export function duplicateQuestion(d: ExamDraft, questionUid: string): ExamDraft {
  const i = d.questions.findIndex((q) => q.uid === questionUid);
  if (i < 0) return d;
  const src = d.questions[i];
  const copy: DraftQuestion = { ...src, uid: uid(), choices: src.choices.map((c) => ({ ...c, uid: uid() })) };
  return { ...d, questions: [...d.questions.slice(0, i + 1), copy, ...d.questions.slice(i + 1)] };
}

export const deleteQuestion = (d: ExamDraft, questionUid: string): ExamDraft =>
  ({ ...d, questions: d.questions.filter((q) => q.uid !== questionUid) });

export function moveQuestion(d: ExamDraft, questionUid: string, direction: -1 | 1): ExamDraft {
  const i = d.questions.findIndex((q) => q.uid === questionUid);
  const j = i + direction;
  if (i < 0 || j < 0 || j >= d.questions.length) return d;
  const qs = [...d.questions];
  [qs[i], qs[j]] = [qs[j], qs[i]];
  return { ...d, questions: qs };
}

export const totalPoints = (d: ExamDraft): number => d.questions.reduce((sum, q) => sum + (Number(q.points) || 0), 0);

/** Shape sent to the Rust `NewExam` type (client-only `uid`s stripped). */
export function toPayload(d: ExamDraft): Record<string, unknown> {
  return {
    title: d.title, description: d.description, instructions: d.instructions,
    durationMinutes: Math.round(Number(d.durationMinutes) || 0),
    passingScore: Number(d.passingScore) || 0,
    randomizeQuestions: d.randomizeQuestions, randomizeChoices: d.randomizeChoices,
    allowReview: d.allowReview, autoSubmit: d.autoSubmit, showResults: d.showResults,
    questions: d.questions.map((q) => ({
      questionText: q.questionText, questionType: q.questionType, points: Number(q.points) || 0, required: q.required,
      explanation: q.explanation.trim() || null,
      choices: q.choices.map((c) => ({ choiceText: c.choiceText, isCorrect: c.isCorrect })),
    })),
  };
}

export function fromExam(full: ExamFull): ExamDraft {
  return {
    title: full.title, description: full.description, instructions: full.instructions,
    durationMinutes: full.durationMinutes, passingScore: full.passingScore,
    randomizeQuestions: full.randomizeQuestions, randomizeChoices: full.randomizeChoices,
    allowReview: full.allowReview, autoSubmit: full.autoSubmit, showResults: full.showResults,
    questions: full.questions.map((q) => ({
      uid: q.id, questionText: q.questionText, questionType: q.questionType, points: q.points, required: q.required,
      explanation: q.explanation ?? "",
      choices: q.choices.map((c) => ({ uid: c.id, choiceText: c.choiceText, isCorrect: c.isCorrect })),
    })),
  };
}

/** Groups backend validation issues by question index (`questions.<i>.<field>`). */
export function issuesByQuestion<T extends { path: string }>(issues: T[]): Map<number, T[]> {
  const map = new Map<number, T[]>();
  for (const issue of issues) {
    const m = /^questions\.(\d+)\./.exec(issue.path);
    if (m) map.set(Number(m[1]), [...(map.get(Number(m[1])) ?? []), issue]);
  }
  return map;
}
