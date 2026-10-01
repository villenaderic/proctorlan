export type AppMode = "teacher" | "student";

/** Shape returned by the Rust `app_info` command. */
export interface AppInfo {
  name: string;
  version: string;
  os: string;
  arch: string;
  dataDir: string;
}
