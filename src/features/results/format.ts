import type { AttemptStatus } from "@/types/session";
import type { QuestionStat, ResultRow } from "@/types/results";

export const pct = (v: number | null | undefined): string => (v == null ? "—" : `${Math.round(v * 10) / 10}%`);

/** "12 min 5 s", "45 s", "1 h 3 min"; "—" when unknown. */
export function duration(seconds: number | null | undefined): string {
  if (seconds == null) return "—";
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return s % 60 === 0 ? `${m} min` : `${m} min ${s % 60} s`;
  return `${Math.floor(m / 60)} h ${m % 60} min`;
}

export const STATUS_TEXT: Record<AttemptStatus, string> = {
  JOINED: "Joined, did not start", IN_PROGRESS: "In progress", SUBMITTED: "Submitted", AUTO_SUBMITTED: "Auto-submitted",
};

export const resultText = (passed: boolean | null): string => (passed === true ? "Passed" : passed === false ? "Not passed" : "—");

export type Difficulty = "easy" | "medium" | "hard" | "none";
/** Plain-language read of how many students got a question right. Needs at least one submitted attempt. */
export function difficulty(q: Pick<QuestionStat, "attempts" | "percentCorrect">): Difficulty {
  if (q.attempts === 0) return "none";
  if (q.percentCorrect >= 80) return "easy";
  if (q.percentCorrect >= 40) return "medium";
  return "hard";
}
export const DIFFICULTY_TEXT: Record<Difficulty, string> = { easy: "Most got it", medium: "Mixed", hard: "Few got it", none: "No data" };

export type SortKey = "name" | "percentage" | "time";
export function sortRows(rows: ResultRow[], key: SortKey, dir: 1 | -1 = 1): ResultRow[] {
  const val = (r: ResultRow): number | string | null =>
    key === "name" ? r.name.toLowerCase() : key === "percentage" ? r.percentage : r.timeTakenSeconds;
  return [...rows].sort((a, b) => {
    const [x, y] = [val(a), val(b)];
    if (x === null && y === null) return 0;
    if (x === null) return 1; // unfinished always last, regardless of direction
    if (y === null) return -1;
    return (x < y ? -1 : x > y ? 1 : 0) * dir;
  });
}
