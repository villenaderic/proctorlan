import type { SessionSnapshot } from "@/types/session";

/**
 * Display-only countdown. The server decides the deadline; this just smooths the seconds
 * between snapshots. Only a RUNNING session counts down; paused/waiting values are frozen.
 */
export function displayRemaining(snap: Pick<SessionSnapshot, "status" | "remainingSeconds">, fetchedAtMs: number, nowMs: number): number | null {
  if (snap.remainingSeconds == null) return null;
  if (snap.status !== "RUNNING") return Math.max(0, snap.remainingSeconds);
  return Math.max(0, snap.remainingSeconds - Math.floor((nowMs - fetchedAtMs) / 1000));
}

/** Which teacher actions make sense in each state. */
export function allowedActions(status: SessionStatus): Record<"start" | "pause" | "resume" | "end", boolean> {
  return {
    start: status === "WAITING",
    pause: status === "RUNNING",
    resume: status === "PAUSED",
    end: status === "WAITING" || status === "RUNNING" || status === "PAUSED",
  };
}

type SessionStatus = SessionSnapshot["status"];

export const STATUS_LABEL: Record<SessionStatus, string> = {
  CREATED: "Created",
  WAITING: "Waiting for students",
  RUNNING: "In progress",
  PAUSED: "Paused",
  ENDED: "Ended",
};
