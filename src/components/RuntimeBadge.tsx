import { useEffect, useState } from "react";
import { getAppInfo, getDbStatus } from "@/services/tauri";
import type { AppInfo, DbStatus } from "@/types/app";

/** Footer proving the React <-> Rust <-> SQLite chain works (real `app_info` and `db_status` commands). */
export function RuntimeBadge() {
  const [info, setInfo] = useState<AppInfo | null | undefined>(undefined);
  const [db, setDb] = useState<DbStatus | null>(null);
  const [dbError, setDbError] = useState(false);

  useEffect(() => {
    getAppInfo().then(setInfo).catch(() => setInfo(null));
    getDbStatus().then(setDb).catch(() => setDbError(true));
  }, []);

  if (info === undefined) return null;
  if (!info) return <p className="text-xs text-slate-500 dark:text-slate-400">Browser preview (not running inside the desktop shell)</p>;

  return (
    <p className="text-xs text-slate-500 dark:text-slate-400">
      v{info.version} · {info.os}/{info.arch} ·{" "}
      {dbError ? "Database error" : db ? `Database ready (schema v${db.migrationsApplied})` : "Opening database…"}
    </p>
  );
}
