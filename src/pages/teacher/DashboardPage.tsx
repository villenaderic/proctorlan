import { CheckCircle2, FileText, Radio, Users } from "lucide-react";
import { useEffect, useState, type ComponentType } from "react";
import { Card } from "@/components/ui/card";
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

  useEffect(() => {
    authApi.dashboardStats().then(setStats).catch((e) => {
      const msg = toMessage(e);
      if (isSessionExpired(msg)) refreshAuth();
      else setError(msg);
    });
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
      <Card className="p-6">
        <h2 className="font-medium">Recent exams</h2>
        <p className="mt-2 text-sm text-slate-600 dark:text-slate-300">No exams yet. The exam builder arrives in Phase 4.</p>
      </Card>
    </div>
  );
}
