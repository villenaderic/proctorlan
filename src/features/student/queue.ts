import type { AnswerValue } from "@/types/paper";

/** An answer the server has not acknowledged yet. */
export interface PendingAnswer { answer: AnswerValue; seq: number }
export type PendingMap = Record<string, PendingAnswer>;

const PREFIX = "proctorlan.student.queue.";

const valid = (v: unknown): v is PendingAnswer => {
  if (!v || typeof v !== "object") return false;
  const { answer, seq } = v as PendingAnswer;
  const answerOk = typeof answer === "string" || (Array.isArray(answer) && answer.every((x) => typeof x === "string"));
  return answerOk && typeof seq === "number" && Number.isFinite(seq) && seq >= 0;
};

/** Loads the unsent answers for one attempt. Corrupt or foreign data is ignored, never trusted. */
export function loadQueue(attemptId: string): PendingMap {
  try {
    const raw = localStorage.getItem(PREFIX + attemptId);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).filter(([, v]) => valid(v))) as PendingMap;
  } catch { return {}; }
}

/** Writes the queue (an empty queue removes the entry). Storage failures never break the exam. */
export function saveQueue(attemptId: string, pending: PendingMap): void {
  try {
    if (Object.keys(pending).length === 0) localStorage.removeItem(PREFIX + attemptId);
    else localStorage.setItem(PREFIX + attemptId, JSON.stringify(pending));
  } catch { /* storage full/unavailable: answers still sync while the app stays open */ }
}

export function clearQueue(attemptId: string): void {
  try { localStorage.removeItem(PREFIX + attemptId); } catch { /* ignore */ }
}

/** Newer sequence wins, per question. Used when a restarted app meets answers already in memory. */
export function mergeQueues(a: PendingMap, b: PendingMap): PendingMap {
  const out: PendingMap = { ...a };
  for (const [q, p] of Object.entries(b)) if (!out[q] || p.seq > out[q].seq) out[q] = p;
  return out;
}
