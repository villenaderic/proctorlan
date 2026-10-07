import type { BackupContents, BackupKind } from "@/types/backup";

export function fileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export const KIND_LABEL: Record<BackupKind, string> = {
  manual: "Manual", auto: "Automatic", prerestore: "Before a restore", corrupt: "Unreadable (saved)",
};

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** "3 exams, 2 sessions, 40 students, 38 attempts" */
export function describeContents(c: BackupContents): string {
  return [plural(c.exams, "exam"), plural(c.sessions, "session"), plural(c.students, "student"), plural(c.attempts, "attempt")].join(", ");
}

/** Strips the quotes Windows adds when copying a file as a path. */
export const cleanPath = (p: string): string => p.trim().replace(/^"(.*)"$/, "$1").trim();

/** Windows (C:\ or \\server), or POSIX (/). Relative paths cannot be restored from. */
export const looksAbsolute = (p: string): boolean => /^([a-zA-Z]:[\\/]|\\\\|\/)/.test(cleanPath(p));
