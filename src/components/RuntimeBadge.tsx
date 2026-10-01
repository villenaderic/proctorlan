import { useEffect, useState } from "react";
import { getAppInfo } from "@/services/tauri";
import type { AppInfo } from "@/types/app";

/** Small footer proving the React <-> Rust bridge works (real `app_info` command). */
export function RuntimeBadge() {
  const [info, setInfo] = useState<AppInfo | null | undefined>(undefined);
  useEffect(() => { getAppInfo().then(setInfo).catch(() => setInfo(null)); }, []);
  if (info === undefined) return null;
  return (
    <p className="text-xs text-slate-500 dark:text-slate-400">
      {info ? `v${info.version} · ${info.os}/${info.arch}` : "Browser preview (not running inside the desktop shell)"}
    </p>
  );
}
