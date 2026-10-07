import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Card } from "@/components/ui/card";
import { toMessage } from "@/services/auth";
import { resultsApi } from "@/services/results";
import type { ResultSessionRow } from "@/types/results";
import { formatDate } from "@/utils/format";
import { pct } from "./format";

export function ResultsPage() {
  const [rows, setRows] = useState<ResultSessionRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { resultsApi.sessions().then(setRows).catch((e) => setError(toMessage(e))); }, []);

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold">Results</h1>
      {error && <p role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>}
      {rows === null && !error && <p>Loading…</p>}
      {rows?.length === 0 && (
        <Card className="p-6 text-sm text-slate-600 dark:text-slate-300">
          No results yet. Results appear here once students have joined a session. Start one from <Link className="text-brand-600 underline dark:text-sky-400" to="/teacher/sessions">Sessions</Link>.
        </Card>
      )}
      {rows && rows.length > 0 && (
        <Card className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-left text-xs uppercase text-slate-500 dark:bg-navy-800 dark:text-slate-300">
              <tr><th className="px-4 py-2">Exam</th><th className="px-4 py-2">Date</th><th className="px-4 py-2">Code</th><th className="px-4 py-2">Submitted</th><th className="px-4 py-2">Average</th><th className="px-4 py-2">Passed</th></tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.sessionId} className="border-t border-slate-200 dark:border-slate-700">
                  <td className="px-4 py-2 font-medium"><Link className="text-brand-600 underline dark:text-sky-400" to={`/teacher/results/${r.sessionId}`}>{r.examTitle}</Link></td>
                  <td className="px-4 py-2">{formatDate(r.createdAt)}</td>
                  <td className="px-4 py-2 font-mono">{r.sessionCode}</td>
                  <td className="px-4 py-2">{r.submitted} of {r.joined}</td>
                  <td className="px-4 py-2">{pct(r.averagePercentage)}</td>
                  <td className="px-4 py-2">{r.submitted > 0 ? `${r.passed} of ${r.submitted}` : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Card>
      )}
    </div>
  );
}
