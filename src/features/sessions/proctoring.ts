import type { ProctorEventType, RosterRow, SessionEvent } from "@/types/session";

/** "45 s", "3 min 5 s", "1 h 2 min". */
export function formatSpan(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return s % 60 === 0 ? `${m} min` : `${m} min ${s % 60} s`;
  return `${Math.floor(m / 60)} h ${m % 60} min`;
}

export interface Flag { key: string; text: string; tone: "amber" | "red" }

/** The short badges shown next to a student. They describe counts, not guilt. */
export function flagsFor(r: RosterRow): Flag[] {
  const out: Flag[] = [];
  if (r.focusLostCount > 0) {
    out.push({ key: "focus", text: `Left window ×${r.focusLostCount}${r.focusLostMs > 0 ? ` (${formatSpan(r.focusLostMs)})` : ""}`, tone: r.focusLostCount >= 5 ? "red" : "amber" });
  }
  if (r.disconnectCount > 0) out.push({ key: "disc", text: `Disconnected ×${r.disconnectCount}`, tone: r.disconnectCount >= 3 ? "red" : "amber" });
  return out;
}

/** Students currently out of the window or offline mid-exam float up; otherwise most flags first, then name. */
export function sortForAttention(rows: RosterRow[]): RosterRow[] {
  const score = (r: RosterRow) => (r.status === "IN_PROGRESS" && !r.online ? 1000 : 0) + r.focusLostCount + r.disconnectCount * 2;
  return [...rows].sort((a, b) => score(b) - score(a) || a.name.localeCompare(b.name));
}

export const EVENT_LABEL: Record<ProctorEventType, string> = {
  FOCUS_LOST: "Left exam window",
  FOCUS_RESTORED: "Returned to window",
  DISCONNECTED: "Disconnected",
  RECONNECTED: "Reconnected",
  SUBMISSION: "Submitted",
  TIMEOUT: "Time ran out",
};

export const EVENT_TONE: Record<ProctorEventType, "amber" | "green" | "blue" | "gray"> = {
  FOCUS_LOST: "amber", FOCUS_RESTORED: "green", DISCONNECTED: "amber", RECONNECTED: "green", SUBMISSION: "blue", TIMEOUT: "gray",
};

/** One student's events in the order they happened. The feed arrives newest first. */
export function timelineFor(events: SessionEvent[], attemptId: string): SessionEvent[] {
  return events.filter((e) => e.attemptId === attemptId).reverse();
}
