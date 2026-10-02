import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { TeacherLayout } from "@/layouts/TeacherLayout";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/stores/auth";
import { LoginPage } from "./LoginPage";
import { SetupPage } from "./SetupPage";

/** Decides what the teacher sees: first-run setup, login, or the signed-in shell. */
export function TeacherGate() {
  const { phase, refresh } = useAuth();
  const nav = useNavigate();
  useEffect(() => { refresh(); }, [refresh]);

  if (phase === "loading") return <p className="p-8 text-center text-slate-500" role="status">Loading…</p>;
  if (phase === "unavailable")
    return (
      <main className="flex min-h-full flex-col items-center justify-center gap-3 p-8 text-center">
        <h1 className="text-xl font-semibold">Teacher mode needs the desktop app</h1>
        <p className="max-w-md text-slate-600 dark:text-slate-300">Accounts and exams are stored by the desktop application. Start it with <code>npm run tauri:dev</code> instead of opening the page in a browser.</p>
        <Button variant="outline" onClick={() => nav("/")}>Back</Button>
      </main>
    );
  if (phase === "setup") return <SetupPage />;
  if (phase === "login") return <LoginPage />;
  return <TeacherLayout />;
}
