import { CheckCircle2, FileText, Radio, Users } from "lucide-react";
import { useEffect, useState, type ComponentType } from "react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { examsApi } from "@/services/exams";
import type { ExamSummary } from "@/types/exam";
import { formatDate } from "@/utils/format";
import { authApi, isSessionExpired, toMessage } from "@/services/auth";
import { refreshAuth, useAuth } from "@/stores/auth";
import type { Stats } from "@/types/app";

function StatCard({ label, value, icon: Icon }: { label: string; value: number | null; icon: ComponentType<{ className?: string }> }) {
  return (
    <Card className="flex items-center gap-4 p-5">
      <div className="rounded-lg bg-brand-50 p-3 text-brand-600 dark:bg-navy-800"><Icon className="h-5 w-5" /></div>
      <div>
        <p className="text-sm text-slate-600 dark:text-slate-300">{label}</p>
        <p className="text-2xl font-semibold">{value ?? "–"}</p>
      </div>
    </Card>
  );
}

export function DashboardPage() {
  const user = useAuth((s) => s.user);
  const [stats, setStats] = useState<Stats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [recent, setRecent] = useState<ExamSummary[] | null>(null);

  useEffect(() => {
    authApi.dashboardStats().then(setStats).catch((e) => {
      const msg = toMessage(e);
      if (isSessionExpired(msg)) refreshAuth();
      else setError(msg);
    });
    examsApi.list().then((all) => setRecent(all.slice(0, 5))).catch(() => setRecent([]));
  }, []);

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Dashboard</h1>
        <p className="text-sm text-slate-600 dark:text-slate-300">Welcome back, {user?.displayName}</p>
      </header>
      {error && <p role="alert" className="text-sm text-red-600">{error}</p>}
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard label="Total Exams" value={stats?.exams ?? null} icon={FileText} />
        <StatCard label="Active Sessions" value={stats?.activeSessions ?? null} icon={Radio} />
        <StatCard label="Students" value={stats?.students ?? null} icon={Users} />
        <StatCard label="Completed Attempts" value={stats?.completedAttempts ?? null} icon={CheckCircle2} />
      </div>
      <Card className="overflow-x-auto p-6">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="font-medium">Recent exams</h2>
          <Link to="/teacher/exams" className="text-sm text-brand-600 hover:underline">View all</Link>
        </div>
        {recent === null ? <p className="text-sm text-slate-500" role="status">Loading…</p> : recent.length === 0 ? (
          <p className="text-sm text-slate-600 dark:text-slate-300">No exams yet. <Link to="/teacher/exams/new" className="text-brand-600 hover:underline">Create your first exam</Link>.</p>
        ) : (
          <table className="w-full text-left text-sm">
            <thead className="text-xs uppercase text-slate-500 dark:text-slate-400"><tr><th className="py-2">Title</th><th>Questions</th><th>Duration</th><th>Status</th><th>Updated</th></tr></thead>
            <tbody>{recent.map((e) => (
              <tr key={e.id} className="border-t border-slate-200 dark:border-slate-700">
                <td className="py-2 font-medium">{e.title}</td><td>{e.questionCount}</td><td>{e.durationMinutes} min</td>
                <td><Badge tone={e.status === "active" ? "green" : "gray"}>{e.status === "active" ? "● Active" : "○ Inactive"}</Badge></td>
                <td>{formatDate(e.updatedAt)}</td>
              </tr>))}</tbody>
          </table>
        )}
      </Card>
    </div>
  );
}
