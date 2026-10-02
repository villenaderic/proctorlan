import { Copy, Eye, Pencil, Plus, Power, PowerOff, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { isSessionExpired, toMessage } from "@/services/auth";
import { examsApi } from "@/services/exams";
import { refreshAuth } from "@/stores/auth";
import type { ExamSummary } from "@/types/exam";
import { formatDate } from "@/utils/format";

type Banner = { kind: "ok" | "error"; text: string } | null;

export function ExamsPage() {
  const nav = useNavigate();
  const location = useLocation();
  const initialNotice = (location.state as { notice?: string } | null)?.notice;
  const [exams, setExams] = useState<ExamSummary[] | null>(null);
  const [banner, setBanner] = useState<Banner>(initialNotice ? { kind: "ok", text: initialNotice } : null);
  const [toDelete, setToDelete] = useState<ExamSummary | null>(null);
  const [query, setQuery] = useState("");

  const fail = useCallback((e: unknown) => {
    const text = toMessage(e);
    if (isSessionExpired(text)) refreshAuth();
    setBanner({ kind: "error", text });
  }, []);

  const load = useCallback(() => examsApi.list().then(setExams).catch(fail), [fail]);
  useEffect(() => { load(); }, [load]);

  async function run(action: () => Promise<unknown>, okText: string) {
    try { await action(); setBanner({ kind: "ok", text: okText }); await load(); } catch (e) { fail(e); }
  }

  const shown = (exams ?? []).filter((e) => e.title.toLowerCase().includes(query.trim().toLowerCase()));

  return (
    <div className="space-y-4">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold">Exams</h1>
          <p className="text-sm text-slate-600 dark:text-slate-300">Create, edit and prepare exams for your sessions.</p>
        </div>
        <Button onClick={() => nav("/teacher/exams/new")}><Plus className="h-4 w-4" aria-hidden /> Create exam</Button>
      </header>

      {banner && (
        <p role={banner.kind === "error" ? "alert" : "status"}
          className={`rounded-md p-3 text-sm ${banner.kind === "ok" ? "bg-green-50 text-green-800 dark:bg-green-950 dark:text-green-300" : "bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-300"}`}>
          {banner.text}
        </p>
      )}

      {exams === null ? <p className="text-slate-500" role="status">Loading…</p> : exams.length === 0 ? (
        <Card className="space-y-3 p-10 text-center">
          <p className="font-medium">No exams yet</p>
          <p className="text-sm text-slate-600 dark:text-slate-300">Create your first exam to get started.</p>
          <div><Button onClick={() => nav("/teacher/exams/new")}><Plus className="h-4 w-4" aria-hidden /> Create exam</Button></div>
        </Card>
      ) : (
        <Card className="overflow-x-auto">
          <div className="border-b border-slate-200 p-3 dark:border-slate-700">
            <input type="search" aria-label="Search exams" placeholder="Search exams…" value={query} onChange={(e) => setQuery(e.target.value)}
              className="h-9 w-full max-w-xs rounded-md border border-slate-300 bg-white px-3 text-sm dark:border-slate-600 dark:bg-navy-950" />
          </div>
          <table className="w-full text-left text-sm">
            <thead className="text-xs uppercase text-slate-500 dark:text-slate-400">
              <tr><th className="p-3">Title</th><th className="p-3">Questions</th><th className="p-3">Points</th><th className="p-3">Duration</th><th className="p-3">Status</th><th className="p-3">Updated</th><th className="p-3 text-right">Actions</th></tr>
            </thead>
            <tbody>
              {shown.map((e) => {
                const locked = e.sessionCount > 0;
                return (
                  <tr key={e.id} className="border-t border-slate-200 dark:border-slate-700">
                    <td className="p-3 font-medium">{e.title}{locked && <span className="ml-2 text-xs font-normal text-slate-500">has sessions</span>}</td>
                    <td className="p-3">{e.questionCount}</td>
                    <td className="p-3">{e.totalPoints}</td>
                    <td className="p-3">{e.durationMinutes} min</td>
                    <td className="p-3"><Badge tone={e.status === "active" ? "green" : "gray"}>{e.status === "active" ? "● Active" : "○ Inactive"}</Badge></td>
                    <td className="p-3 whitespace-nowrap">{formatDate(e.updatedAt)}</td>
                    <td className="p-3">
                      <div className="flex justify-end gap-1">
                        <Link to={`/teacher/exams/${e.id}/preview`} aria-label={`Preview ${e.title}`} className="inline-flex h-8 w-8 items-center justify-center rounded-md hover:bg-slate-200/70 dark:hover:bg-navy-800"><Eye className="h-4 w-4" /></Link>
                        <Button size="icon" variant="ghost" aria-label={locked ? `${e.title} is locked: it has sessions` : `Edit ${e.title}`} title={locked ? "Has sessions: duplicate it to make changes" : "Edit"} onClick={() => nav(`/teacher/exams/${e.id}/edit`)}><Pencil className="h-4 w-4" /></Button>
                        <Button size="icon" variant="ghost" aria-label={`Duplicate ${e.title}`} title="Duplicate" onClick={() => run(() => examsApi.duplicate(e.id), `Duplicated "${e.title}".`)}><Copy className="h-4 w-4" /></Button>
                        <Button size="icon" variant="ghost" aria-label={e.status === "active" ? `Deactivate ${e.title}` : `Activate ${e.title}`} title={e.status === "active" ? "Deactivate" : "Activate"}
                          onClick={() => run(() => examsApi.setActive(e.id, e.status !== "active"), e.status === "active" ? `"${e.title}" is now inactive.` : `"${e.title}" is now active.`)}>
                          {e.status === "active" ? <PowerOff className="h-4 w-4" /> : <Power className="h-4 w-4" />}
                        </Button>
                        <Button size="icon" variant="ghost" aria-label={`Delete ${e.title}`} title={locked ? "Has sessions: cannot delete" : "Delete"} disabled={locked} onClick={() => setToDelete(e)}><Trash2 className="h-4 w-4" /></Button>
                      </div>
                    </td>
                  </tr>
                );
              })}
              {shown.length === 0 && <tr><td colSpan={7} className="p-6 text-center text-slate-500">No exams match “{query}”.</td></tr>}
            </tbody>
          </table>
        </Card>
      )}

      <ConfirmDialog open={!!toDelete} danger title="Delete exam?" confirmLabel="Delete" onCancel={() => setToDelete(null)}
        onConfirm={() => { const e = toDelete; setToDelete(null); if (e) run(() => examsApi.remove(e.id), `Deleted "${e.title}".`); }}>
        “{toDelete?.title}” and all of its questions will be permanently deleted. This cannot be undone.
      </ConfirmDialog>
    </div>
  );
}
