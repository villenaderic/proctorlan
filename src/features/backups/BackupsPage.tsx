import { DatabaseBackup, RotateCcw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { isSessionExpired, toMessage } from "@/services/auth";
import { backupApi } from "@/services/backup";
import { refreshAuth } from "@/stores/auth";
import type { BackupContents, BackupInfo, BackupOverview } from "@/types/backup";
import { formatDate } from "@/utils/format";
import { KIND_LABEL, cleanPath, describeContents, fileSize, looksAbsolute } from "./format";

type Banner = { kind: "ok" | "error"; text: string } | null;
type Target = { label: string; name?: string; path?: string };

export function BackupsPage() {
  const [ov, setOv] = useState<BackupOverview | null>(null);
  const [banner, setBanner] = useState<Banner>(null);
  const [busy, setBusy] = useState(false);
  const [dest, setDest] = useState("");
  const [filePath, setFilePath] = useState("");
  const [toRestore, setToRestore] = useState<Target | null>(null);
  const [toDelete, setToDelete] = useState<BackupInfo | null>(null);
  const [staged, setStaged] = useState<BackupContents | null>(null);

  const fail = useCallback((e: unknown) => {
    const text = toMessage(e);
    if (isSessionExpired(text)) refreshAuth();
    setBanner({ kind: "error", text });
  }, []);
  const load = useCallback(() => backupApi.overview().then(setOv).catch(fail), [fail]);
  useEffect(() => { load(); }, [load]);

  async function run<T>(action: () => Promise<T>, ok: (r: T) => string) {
    setBusy(true); setBanner(null);
    try { const r = await action(); setBanner({ kind: "ok", text: ok(r) }); await load(); return r; } catch (e) { fail(e); } finally { setBusy(false); }
  }

  async function stage() {
    const t = toRestore;
    setToRestore(null);
    if (!t) return;
    const c = await run(() => (t.name ? backupApi.stageNamed(t.name) : backupApi.stagePath(cleanPath(t.path ?? ""))), () => "Restore is ready. Restart ProctorLAN to finish.");
    if (c) setStaged(c);
  }

  if (!ov) return banner ? <p role="alert" className="text-red-700 dark:text-red-300">{banner.text}</p> : <p>Loading…</p>;
  const pending = ov.pendingRestore ?? staged;

  return (
    <div className="space-y-4">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold">Backups</h1>
          <p className="text-sm text-slate-600 dark:text-slate-300">Protect your exams, students and results against a lost or damaged computer.</p>
        </div>
        <Button disabled={busy} onClick={() => run(() => backupApi.create(dest.trim() || null), (b) => `Backup saved: ${b.path}`)}>
          <DatabaseBackup className="h-4 w-4" aria-hidden /> {busy ? "Working…" : "Create backup now"}
        </Button>
      </header>

      {banner && (
        <p role={banner.kind === "error" ? "alert" : "status"} className={`break-all rounded-md p-3 text-sm ${banner.kind === "ok" ? "bg-green-50 text-green-800 dark:bg-green-950 dark:text-green-300" : "bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-300"}`}>{banner.text}</p>
      )}

      {pending && (
        <Card className="space-y-3 border-amber-400 p-4 dark:border-amber-500">
          <p className="font-medium">A restore is waiting for a restart</p>
          <p className="text-sm text-slate-600 dark:text-slate-300">It contains {describeContents(pending)}. When ProctorLAN restarts it replaces your current data with this backup. Your current data is saved first as a “Before a restore” backup.</p>
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => backupApi.restart().catch(fail)}><RotateCcw className="h-4 w-4" aria-hidden /> Restart now</Button>
            <Button variant="outline" onClick={() => run(() => backupApi.cancelRestore(), () => "Restore cancelled.").then(() => setStaged(null))}>Cancel restore</Button>
          </div>
          <p className="text-xs text-slate-500">If the restart button does nothing, close ProctorLAN and open it again.</p>
        </Card>
      )}

      <Card className="space-y-3 p-4">
        <h2 className="font-medium">Settings</h2>
        <label className="flex items-start gap-2 text-sm">
          <input type="checkbox" className="mt-1" checked={ov.autoEnabled} onChange={(e) => run(() => backupApi.setAuto(e.target.checked), () => (e.target.checked ? "Automatic backups are on." : "Automatic backups are off."))} />
          <span><span className="font-medium">Back up automatically</span><br /><span className="text-slate-600 dark:text-slate-300">When ProctorLAN starts and the last backup is over a day old, it saves one and keeps the 10 newest automatic backups.</span></span>
        </label>
        <div>
          <label htmlFor="dest" className="text-sm font-medium">Save manual backups to another folder (optional)</label>
          <input id="dest" value={dest} onChange={(e) => setDest(e.target.value)} placeholder="e.g. E:\ProctorLAN backups  (a USB drive)"
            className="mt-1 h-10 w-full rounded-md border border-slate-300 bg-white px-3 text-sm dark:border-slate-600 dark:bg-navy-950" />
          <p className="mt-1 text-xs text-slate-500">Left empty, backups go to: <span className="break-all font-mono">{ov.directory}</span></p>
        </div>
        <p className="text-xs text-slate-500">Backups contain everything, including student records and the teacher password hash. Keep them as private as the exams themselves.</p>
      </Card>

      <Card className="overflow-x-auto">
        <h2 className="border-b border-slate-200 px-4 py-3 font-medium dark:border-slate-700">Saved backups ({ov.backups.length})</h2>
        {ov.backups.length === 0 ? <p className="p-4 text-sm text-slate-600 dark:text-slate-300">No backups yet. Press “Create backup now”.</p> : (
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-left text-xs uppercase text-slate-500 dark:bg-navy-800 dark:text-slate-300">
              <tr><th className="px-4 py-2">Date</th><th className="px-4 py-2">Type</th><th className="px-4 py-2">Size</th><th className="px-4 py-2 text-right">Actions</th></tr>
            </thead>
            <tbody>
              {ov.backups.map((b) => (
                <tr key={b.fileName} className="border-t border-slate-200 dark:border-slate-700">
                  <td className="px-4 py-2">{formatDate(b.createdAt)}</td>
                  <td className="px-4 py-2"><Badge tone={b.kind === "manual" ? "blue" : b.kind === "auto" ? "gray" : "amber"}>{KIND_LABEL[b.kind]}</Badge></td>
                  <td className="px-4 py-2">{fileSize(b.sizeBytes)}</td>
                  <td className="px-4 py-2">
                    <div className="flex justify-end gap-1">
                      <Button size="sm" variant="outline" disabled={busy} onClick={() => setToRestore({ label: `${KIND_LABEL[b.kind]} backup from ${formatDate(b.createdAt)}`, name: b.fileName })}><RotateCcw className="h-3.5 w-3.5" aria-hidden /> Restore</Button>
                      <Button size="icon" variant="ghost" aria-label={`Delete backup from ${formatDate(b.createdAt)}`} disabled={busy} onClick={() => setToDelete(b)}><Trash2 className="h-4 w-4" /></Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      <Card className="space-y-2 p-4">
        <h2 className="font-medium">Restore from a file</h2>
        <p className="text-sm text-slate-600 dark:text-slate-300">Use this for a backup on a USB drive or another computer. Paste the full path to the backup file (a <span className="font-mono">.db</span> file).</p>
        <div className="flex flex-wrap gap-2">
          <input aria-label="Path to backup file" value={filePath} onChange={(e) => setFilePath(e.target.value)} placeholder="e.g. E:\ProctorLAN backups\proctorlan-manual-20261006-101500.db"
            className="h-10 min-w-0 flex-1 rounded-md border border-slate-300 bg-white px-3 text-sm dark:border-slate-600 dark:bg-navy-950" />
          <Button variant="outline" disabled={busy || !looksAbsolute(filePath)} onClick={() => setToRestore({ label: cleanPath(filePath), path: filePath })}>Restore from file…</Button>
        </div>
        {filePath.trim() !== "" && !looksAbsolute(filePath) && <p className="text-xs text-amber-700 dark:text-amber-400">Enter the full path, starting with a drive letter such as E:\ or a /.</p>}
      </Card>

      <ConfirmDialog open={!!toRestore} danger title="Restore this backup?" confirmLabel="Prepare restore" onCancel={() => setToRestore(null)} onConfirm={stage}>
        <p>You are about to replace everything in ProctorLAN with: <span className="font-medium">{toRestore?.label}</span>.</p>
        <p className="mt-2">Nothing changes until you restart the app. Your current data is saved first as a “Before a restore” backup, so you can come back to it. You cannot restore while a session is running.</p>
      </ConfirmDialog>
      <ConfirmDialog open={!!toDelete} danger title="Delete this backup?" confirmLabel="Delete" onCancel={() => setToDelete(null)}
        onConfirm={() => { const b = toDelete; setToDelete(null); if (b) run(() => backupApi.remove(b.fileName), () => "Backup deleted."); }}>
        This permanently removes the backup file from this computer.
      </ConfirmDialog>
    </div>
  );
}
