/** Remembers the student's place so a restart or crash can rejoin the SAME attempt. */
export interface SavedJoin {
  host: string;
  port: number;
  sessionCode: string;
  studentName: string;
  studentId: string;
  token: string | null;
  examTitle: string | null;
}

const KEY = "proctorlan.student.join";

export function loadSaved(): SavedJoin | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const v = JSON.parse(raw);
    return typeof v?.host === "string" && typeof v?.sessionCode === "string" && typeof v?.studentId === "string" ? (v as SavedJoin) : null;
  } catch { return null; }
}

export function saveJoin(v: SavedJoin): void {
  try { localStorage.setItem(KEY, JSON.stringify(v)); } catch { /* storage unavailable: rejoin just won't be offered */ }
}

export function clearSaved(): void {
  try { localStorage.removeItem(KEY); } catch { /* ignore */ }
}
