import { Play, Radio } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/form";
import { isSessionExpired, toMessage } from "@/services/auth";
import { examsApi } from "@/services/exams";
import { sessionsApi } from "@/services/sessions";
import { refreshAuth } from "@/stores/auth";
import type { ExamSummary } from "@/types/exam";
import type { SessionRow, SessionStatus } from "@/types/session";
import { formatDate } from "@/utils/format";
import { STATUS_LABEL } from "@/utils/sessionClock";

const tone = (s: SessionStatus) => (s === "RUNNING" ? "green" : s === "PAUSED" ? "amber" : s === "ENDED" ? "gray" : "blue");

export function SessionsPage() {
  const nav = useNavigate();
  const [sessions, setSessions] = useState<SessionRow[] | null>(null);
  const [exams, setExams] = useState<ExamSummary[]>([]);
  const [examId, setExamId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const fail = useCallback((e: unknown) => {
    const text = toMessage(e);
    if (isSessionExpired(text)) refreshAuth();
    setError(text);
  }, []);

  useEffect(() => {
    sessionsApi.list().then(setSessions).catch(fail);
    examsApi.list().then((all) => {
      const active = all.filter((e) => e.status === "active");
      setExams(active);
      setExamId((cur) => cur || active[0]?.id || "");
    }).catch(fail);
  }, [fail]);

  async function create() {
    setBusy(true);
    setError(null);
    try {
      const s = await sessionsApi.create(examId);
      nav(`/teacher/sessions/${s.id}`);
    } catch (e) { fail(e); }
    setBusy(false);
  }

  return (
    <div className="space-y-4">
      <header>
        <h1 className="text-2xl font-semibold">Sessions</h1>
        <p className="text-sm text-slate-600 dark:text-slate-300">Open a session so students on your network can join with its code.</p>
      </header>

      {error && <p role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>}

      <Card className="p-4">
        <h2 className="mb-3 font-medium">Start a new session</h2>
        {exams.length === 0 ? (
          <p className="text-sm text-slate-600 dark:text-slate-300">
            No active exams yet. Activate a complete exam in <Link className="underline" to="/teacher/exams">Exams</Link> first.
          </p>
        ) : (
          <div className="flex flex-wrap items-center gap-3">
            <Select aria-label="Exam" className="max-w-sm" value={examId} onChange={(e) => setExamId(e.target.value)}>
              {exams.map((e) => <option key={e.id} value={e.id}>{e.title}</option>)}
            </Select>
            <Button onClick={create} disabled={!examId || busy}><Radio className="h-4 w-4" aria-hidden /> {busy ? "Opening…" : "Open session"}</Button>
          </div>
        )}
      </Card>

      <Card className="overflow-hidden">
        {sessions === null ? <p className="p-4 text-sm">Loading…</p> : sessions.length === 0 ? (
          <p className="p-4 text-sm text-slate-600 dark:text-slate-300">No sessions yet.</p>
        ) : (
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-left text-xs uppercase text-slate-500 dark:bg-navy-800 dark:text-slate-300">
              <tr><th className="px-4 py-2">Exam</th><th className="px-4 py-2">Code</th><th className="px-4 py-2">Status</th><th className="px-4 py-2">Created</th><th className="px-4 py-2" /></tr>
            </thead>
            <tbody>
              {sessions.map((s) => (
                <tr key={s.id} className="border-t border-slate-200 dark:border-slate-700">
                  <td className="px-4 py-2 font-medium">{s.examTitle}</td>
                  <td className="px-4 py-2 font-mono tracking-widest">{s.status === "ENDED" ? "—" : s.sessionCode}</td>
                  <td className="px-4 py-2"><Badge tone={tone(s.status)}>{STATUS_LABEL[s.status]}</Badge></td>
                  <td className="px-4 py-2">{formatDate(s.createdAt)}</td>
                  <td className="px-4 py-2 text-right"><Button size="sm" variant="outline" onClick={() => nav(`/teacher/sessions/${s.id}`)}><Play className="h-3.5 w-3.5" aria-hidden /> Open</Button></td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>
    </div>
  );
}
