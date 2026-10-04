import type { AnswerValue, Paper } from "@/types/paper";

/** An answer counts only if something is actually selected or typed. */
export function isAnswered(v: AnswerValue | undefined): boolean {
  if (v === undefined) return false;
  return Array.isArray(v) ? v.length > 0 : v.trim().length > 0;
}

export function answeredCount(paper: Paper, answers: Record<string, AnswerValue>): number {
  return paper.questions.filter((q) => isAnswered(answers[q.id])).length;
}

export function unansweredIndexes(paper: Paper, answers: Record<string, AnswerValue>): number[] {
  return paper.questions.flatMap((q, i) => (isAnswered(answers[q.id]) ? [] : [i]));
}

/**
 * Monotonic per-answer sequence numbers. Time-based so they keep increasing across app restarts
 * (the server keeps the highest one it has seen and ignores anything older).
 */
export function createSequencer(now: () => number = Date.now) {
  let last = 0;
  return () => {
    last = Math.max(last + 1, now());
    return last;
  };
}

/** Merge server-saved answers with answers still waiting to be acknowledged: pending wins. */
export function mergeAnswers(server: Record<string, AnswerValue>, pending: Record<string, { answer: AnswerValue }>): Record<string, AnswerValue> {
  const out = { ...server };
  for (const [q, p] of Object.entries(pending)) out[q] = p.answer;
  return out;
}

export type TimerTone = "normal" | "warning" | "danger";

export function timerTone(remainingSeconds: number | null): TimerTone {
  if (remainingSeconds === null) return "normal";
  if (remainingSeconds <= 60) return "danger";
  if (remainingSeconds <= 300) return "warning";
  return "normal";
}

/** Spoken announcements at 5 minutes and 1 minute (and time up), once each. */
export function timerAnnouncement(remainingSeconds: number | null): string | null {
  if (remainingSeconds === null) return null;
  if (remainingSeconds === 0) return "Time is up.";
  if (remainingSeconds <= 60) return "One minute remaining.";
  if (remainingSeconds <= 300) return "Five minutes remaining.";
  return null;
}
