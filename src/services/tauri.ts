import { invoke } from "@tauri-apps/api/core";
import type { AppInfo, DbStatus } from "@/types/app";

/** True when running inside the Tauri shell (false in a plain browser, e.g. `npm run dev`). */
export const isTauri = (): boolean => "__TAURI_INTERNALS__" in window;

export async function getAppInfo(): Promise<AppInfo | null> {
  if (!isTauri()) return null;
  return invoke<AppInfo>("app_info");
}

export async function getDbStatus(): Promise<DbStatus | null> {
  if (!isTauri()) return null;
  return invoke<DbStatus>("db_status");
}
