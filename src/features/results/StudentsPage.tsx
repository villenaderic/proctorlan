import { Fragment, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { toMessage } from "@/services/auth";
import { resultsApi } from "@/services/results";
import type { StudentAttemptRow, StudentSummary } from "@/types/results";
import { formatDate } from "@/utils/format";
import { STATUS_TEXT, pct } from "./format";

export function StudentsPage() {
  const [students, setStudents] = useState<StudentSummary[] | null>(null);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [history, setHistory] = useState<StudentAttemptRow[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => { resultsApi.students().then(setStudents).catch((e) => setError(toMessage(e))); }, []);
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (students ?? []).filter((s) => !q || s.name.toLowerCase().includes(q) || s.studentNumber.toLowerCase().includes(q));
  }, [students, query]);

  async function toggle(id: string) {
    if (open === id) { setOpen(null); return; }
    setOpen(id); setHistory([]);
    try { setHistory(await resultsApi.studentHistory(id)); } catch (e) { setError(toMessage(e)); }
  }

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold">Students</h1>
      <p className="text-sm text-slate-600 dark:text-slate-300">Everyone who has joined one of your sessions. Students are identified by their student ID.</p>
      <input aria-label="Search students" placeholder="Search by name or student ID" value={query} onChange={(e) => setQuery(e.target.value)}
        className="w-full max-w-sm rounded-md border border-slate-300 bg-white px-3 py-2 text-sm dark:border-slate-600 dark:bg-navy-900" />
      {error && <p role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>}
      {students === null && !error && <p>Loading…</p>}
      {students && shown.length === 0 && <Card className="p-6 text-sm">{students.length === 0 ? "No students yet. They appear after they join a session." : "No student matches that search."}</Card>}
      {shown.length > 0 && (
        <Card className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-left text-xs uppercase text-slate-500 dark:bg-navy-800 dark:text-slate-300">
              <tr><th className="px-4 py-2">Name</th><th className="px-4 py-2">Student ID</th><th className="px-4 py-2">Sessions</th><th className="px-4 py-2">Average</th><th className="px-4 py-2">Last submitted</th><th className="px-4 py-2" /></tr>
            </thead>
            <tbody>
              {shown.map((s) => (
                <Fragment key={s.id}>
                  <tr className="border-t border-slate-200 dark:border-slate-700">
                    <td className="px-4 py-2 font-medium">{s.name}</td>
                    <td className="px-4 py-2 font-mono">{s.studentNumber}</td>
                    <td className="px-4 py-2">{s.attempts}</td>
                    <td className="px-4 py-2">{pct(s.averagePercentage)}</td>
                    <td className="px-4 py-2">{s.lastAttemptAt ? formatDate(s.lastAttemptAt) : "—"}</td>
                    <td className="px-4 py-2 text-right"><Button size="sm" variant="outline" aria-expanded={open === s.id} onClick={() => toggle(s.id)}>{open === s.id ? "Hide history" : "History"}</Button></td>
                  </tr>
                  {open === s.id && (
                    <tr className="bg-slate-50 dark:bg-navy-950">
                      <td colSpan={6} className="px-4 py-3">
                        {history.length === 0 ? <p className="text-sm">Loading…</p> : (
                          <ul className="space-y-1 text-sm">
                            {history.map((h) => (
                              <li key={h.attemptId} className="flex flex-wrap items-center gap-2">
                                <Link className="text-brand-600 underline dark:text-sky-400" to={`/teacher/results/attempt/${h.attemptId}`}>{h.examTitle}</Link>
                                <span className="text-slate-500">{formatDate(h.submittedAt ?? h.createdAt)}</span>
                                {h.percentage != null ? <><span>{h.score} / {h.totalPoints} ({pct(h.percentage)})</span><Badge tone={h.passed ? "green" : "red"}>{h.passed ? "Passed" : "Not passed"}</Badge></> : <span>{STATUS_TEXT[h.status]}</span>}
                              </li>
                            ))}
                          </ul>
                        )}
                      </td>
                    </tr>
                  )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </Card>
      )}
    </div>
  );
}
