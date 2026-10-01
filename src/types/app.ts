export type AppMode = "teacher" | "student";

/** Shape returned by the Rust `app_info` command. */
export interface AppInfo {
  name: string;
  version: string;
  os: string;
  arch: string;
  dataDir: string;
}

export interface DbStats {
  exams: number;
  activeSessions: number;
  students: number;
  completedAttempts: number;
}

/** Shape returned by the Rust `db_status` command. */
export interface DbStatus {
  ready: boolean;
  migrationsApplied: number;
  stats: DbStats;
}
