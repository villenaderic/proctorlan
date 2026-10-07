import { ArrowDown, ArrowUp, Check, Download, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { Link, useParams } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { toMessage } from "@/services/auth";
import { resultsApi } from "@/services/results";
import type { ExportInfo, SessionResults } from "@/types/results";
import { formatDate } from "@/utils/format";
import { DIFFICULTY_TEXT, STATUS_TEXT, difficulty, duration, pct, resultText, sortRows, type SortKey } from "./format";

const BAR = "#2563eb"; // brand-600; one series, one hue

export function SessionResultsPage() {
  const { id = "" } = useParams();
  const [res, setRes] = useState<SessionResults | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: "name", dir: 1 });
  const [exp, setExp] = useState<ExportInfo | null>(null);
  const [exporting, setExporting] = useState(false);

  useEffect(() => { resultsApi.session(id).then(setRes).catch((e) => setError(toMessage(e))); }, [id]);
  const rows = useMemo(() => (res ? sortRows(res.rows, sort.key, sort.dir) : []), [res, sort]);

  async function doExport() {
    setExporting(true); setError(null);
    try { setExp(await resultsApi.exportCsv(id)); } catch (e) { setError(toMessage(e)); }
    setExporting(false);
  }
  const toggle = (key: SortKey) => setSort((s) => (s.key === key ? { key, dir: s.dir === 1 ? -1 : 1 } : { key, dir: key === "name" ? 1 : -1 }));
  const arrow = (key: SortKey) => sort.key === key ? (sort.dir === 1 ? <ArrowUp className="inline h-3 w-3" aria-label="ascending" /> : <ArrowDown className="inline h-3 w-3" aria-label="descending" />) : null;

  if (!res) return error ? <p role="alert" className="text-red-700 dark:text-red-300">{error}</p> : <p>Loading results…</p>;
  const s = res.stats;
  const none = s.submitted === 0;

  return (
    <div className="space-y-4">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <Link to="/teacher/results" className="text-sm text-brand-600 underline dark:text-sky-400">← All results</Link>
          <h1 className="text-2xl font-semibold">{res.examTitle}</h1>
          <p className="text-sm text-slate-600 dark:text-slate-300">{formatDate(res.createdAt)} · code {res.sessionCode} · {res.questionCount} questions · {res.totalPoints} points · passing {res.passingScore}%</p>
        </div>
        <Button onClick={doExport} disabled={exporting || res.rows.length === 0}><Download className="h-4 w-4" aria-hidden /> {exporting ? "Exporting…" : "Export CSV"}</Button>
      </header>

      {error && <p role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>}
      {exp && (
        <p role="status" className="rounded-md bg-green-50 p-3 text-sm text-green-800 dark:bg-green-950 dark:text-green-300">
          Saved {exp.rows} {exp.rows === 1 ? "row" : "rows"} to <span className="break-all font-mono">{exp.path}</span>
        </p>
      )}

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
        <Tile label="Average" value={none ? "—" : pct(s.mean)} sub={none ? "" : `median ${pct(s.median)}`} />
        <Tile label="Pass rate" value={none ? "—" : pct(s.passRate)} sub={`${s.passed} passed · ${s.failed} not`} />
        <Tile label="Highest / lowest" value={none ? "—" : `${pct(s.highest)} / ${pct(s.lowest)}`} sub={none ? "" : `spread ±${s.stdDev}`} />
        <Tile label="Submitted" value={String(s.submitted)} sub={s.notSubmitted > 0 ? `${s.notSubmitted} did not finish` : "everyone finished"} />
        <Tile label="Average time" value={duration(s.averageTimeSeconds)} sub="start to submit" />
      </div>

      {!none && (
        <div className="grid gap-4 lg:grid-cols-2">
          <Card className="p-4">
            <h2 className="font-medium">Score distribution</h2>
            <p className="mb-2 text-xs text-slate-500">Number of students in each 10-point score band.</p>
            <div className="h-56" role="img" aria-label={`Score distribution: ${res.distribution.filter((b) => b.count > 0).map((b) => `${b.count} in ${b.label}`).join(", ")}`}>
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={res.distribution} margin={{ top: 8, right: 8, bottom: 0, left: -20 }}>
                  <CartesianGrid vertical={false} stroke="currentColor" strokeOpacity={0.12} />
                  <XAxis dataKey="label" tick={{ fontSize: 11, fill: "currentColor" }} tickLine={false} axisLine={false} interval={0} />
                  <YAxis allowDecimals={false} tick={{ fontSize: 11, fill: "currentColor" }} tickLine={false} axisLine={false} />
                  <Tooltip cursor={{ fill: "currentColor", fillOpacity: 0.06 }} formatter={(v) => [`${v} students`, "Count"]} labelFormatter={(l) => `Score ${l}%`} />
                  <Bar dataKey="count" fill={BAR} radius={[4, 4, 0, 0]} maxBarSize={36} isAnimationActive={false} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </Card>
          <Card className="p-4">
            <h2 className="font-medium">Percent correct by question</h2>
            <p className="mb-2 text-xs text-slate-500">Low bars are the questions most students missed.</p>
            <div className="h-56" role="img" aria-label={`Percent correct per question: ${res.questions.map((q) => `Q${q.position} ${q.percentCorrect}%`).join(", ")}`}>
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={res.questions.map((q) => ({ name: `Q${q.position}`, percent: q.percentCorrect }))} margin={{ top: 8, right: 8, bottom: 0, left: -20 }}>
                  <CartesianGrid vertical={false} stroke="currentColor" strokeOpacity={0.12} />
                  <XAxis dataKey="name" tick={{ fontSize: 11, fill: "currentColor" }} tickLine={false} axisLine={false} />
                  <YAxis domain={[0, 100]} unit="%" tick={{ fontSize: 11, fill: "currentColor" }} tickLine={false} axisLine={false} />
                  <Tooltip cursor={{ fill: "currentColor", fillOpacity: 0.06 }} formatter={(v) => [`${v}%`, "Correct"]} />
                  <Bar dataKey="percent" fill={BAR} radius={[4, 4, 0, 0]} maxBarSize={36} isAnimationActive={false} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </Card>
        </div>
      )}

      <Card className="overflow-x-auto">
        <h2 className="border-b border-slate-200 px-4 py-3 font-medium dark:border-slate-700">Students ({rows.length})</h2>
        {rows.length === 0 ? <p className="p-4 text-sm">Nobody joined this session.</p> : (
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-left text-xs uppercase text-slate-500 dark:bg-navy-800 dark:text-slate-300">
              <tr>
                <th className="px-4 py-2"><button className="uppercase" onClick={() => toggle("name")}>Name {arrow("name")}</button></th>
                <th className="px-4 py-2">Student ID</th>
                <th className="px-4 py-2">Status</th>
                <th className="px-4 py-2">Score</th>
                <th className="px-4 py-2"><button className="uppercase" onClick={() => toggle("percentage")}>Percent {arrow("percentage")}</button></th>
                <th className="px-4 py-2">Result</th>
                <th className="px-4 py-2"><button className="uppercase" onClick={() => toggle("time")}>Time {arrow("time")}</button></th>
                <th className="px-4 py-2">Signals</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.attemptId} className="border-t border-slate-200 dark:border-slate-700">
                  <td className="px-4 py-2 font-medium"><Link className="text-brand-600 underline dark:text-sky-400" to={`/teacher/results/attempt/${r.attemptId}`}>{r.name}</Link></td>
                  <td className="px-4 py-2 font-mono">{r.studentNumber}</td>
                  <td className="px-4 py-2">{STATUS_TEXT[r.status]}</td>
                  <td className="px-4 py-2">{r.score == null ? "—" : `${r.score} / ${r.totalPoints}`}</td>
                  <td className="px-4 py-2">{pct(r.percentage)}</td>
                  <td className="px-4 py-2">{r.passed == null ? "—" : <Badge tone={r.passed ? "green" : "red"}>{r.passed ? <Check className="h-3 w-3" aria-hidden /> : <X className="h-3 w-3" aria-hidden />}{resultText(r.passed)}</Badge>}</td>
                  <td className="px-4 py-2">{duration(r.timeTakenSeconds)}</td>
                  <td className="px-4 py-2 text-xs text-slate-600 dark:text-slate-300">
                    {r.focusLostCount === 0 && r.disconnectCount === 0 ? "—" : [r.focusLostCount > 0 ? `left window ×${r.focusLostCount}` : "", r.disconnectCount > 0 ? `disconnected ×${r.disconnectCount}` : ""].filter(Boolean).join(", ")}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      {!none && (
        <Card className="overflow-x-auto">
          <h2 className="border-b border-slate-200 px-4 py-3 font-medium dark:border-slate-700">Question analysis</h2>
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-left text-xs uppercase text-slate-500 dark:bg-navy-800 dark:text-slate-300">
              <tr><th className="px-4 py-2">#</th><th className="px-4 py-2">Question</th><th className="px-4 py-2">Correct</th><th className="px-4 py-2">Answered</th><th className="px-4 py-2">Reading</th><th className="px-4 py-2">Choices picked / common mistakes</th></tr>
            </thead>
            <tbody>
              {res.questions.map((q) => (
                <tr key={q.questionId} className="border-t border-slate-200 align-top dark:border-slate-700">
                  <td className="px-4 py-2">{q.position}</td>
                  <td className="max-w-xs px-4 py-2">{q.text}</td>
                  <td className="px-4 py-2">{q.correct} of {q.attempts} ({pct(q.percentCorrect)})</td>
                  <td className="px-4 py-2">{q.answered}</td>
                  <td className="px-4 py-2">{DIFFICULTY_TEXT[difficulty(q)]}</td>
                  <td className="px-4 py-2 text-xs">
                    {q.questionType === "identification"
                      ? (q.commonWrong.length ? q.commonWrong.map((w) => `“${w.text}” ×${w.count}`).join(", ") : "—")
                      : q.options.map((o) => `${o.text}${o.isCorrect ? " ✓" : ""}: ${o.picked}`).join(" · ")}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Card>
      )}
    </div>
  );
}

function Tile({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <Card className="p-4">
      <p className="text-xs uppercase text-slate-500">{label}</p>
      <p className="text-2xl font-semibold">{value}</p>
      {sub && <p className="text-xs text-slate-500">{sub}</p>}
    </Card>
  );
}
