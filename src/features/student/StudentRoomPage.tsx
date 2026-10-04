import { CheckCircle2, Clock, FileText, LogOut, Pause, Wifi, WifiOff } from "lucide-react";
import { useEffect, useState } from "react";
import { Logo } from "@/components/Logo";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { useStudent } from "@/stores/student";
import { formatClock } from "@/utils/format";
import { displayRemaining } from "@/utils/sessionClock";
import { ExamPage } from "./ExamPage";

/** Waiting room, plus the submitted / session-ended screens. The live exam is ExamPage. */
export function StudentRoomPage() {
  const { phase, connection, info, status, remainingSeconds, receivedAt, leave, paper, submitted, autoSubmitted, result, examError } = useStudent();
  const [now, setNow] = useState(() => Date.now());
  const [confirmLeave, setConfirmLeave] = useState(false);
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  if (!info) return null;
  // The exam itself: shown whenever questions are loaded and the student has not submitted.
  if (paper && !submitted && phase === "in_session") return <ExamPage />;

  const ended = phase === "ended";
  const online = connection === "online";
  const remaining = displayRemaining({ status, remainingSeconds }, receivedAt, now);

  return (
    <main className="flex min-h-full flex-col">
      <header className="flex items-center gap-3 border-b border-slate-200 bg-white px-6 py-3 dark:border-slate-700 dark:bg-navy-900">
        <Logo size={28} />
        <div className="min-w-0 flex-1">
          <p className="truncate font-medium">{info.exam.title}</p>
          <p className="truncate text-xs text-slate-500">{info.studentName} · {info.studentId}</p>
        </div>
        <Badge tone={online ? "green" : "amber"}>
          {online ? <Wifi className="h-3 w-3" aria-hidden /> : <WifiOff className="h-3 w-3" aria-hidden />}
          {online ? "Connected" : "Reconnecting…"}
        </Badge>
        <ThemeToggle />
        {!ended && <Button variant="outline" size="sm" onClick={() => setConfirmLeave(true)}><LogOut className="h-4 w-4" aria-hidden /> Leave</Button>}
      </header>

      {!online && !ended && (
        <p role="alert" className="bg-amber-50 px-6 py-2 text-sm text-amber-800 dark:bg-amber-950 dark:text-amber-300">
          Connection to your teacher's computer was lost. Reconnecting automatically; your place is saved.
        </p>
      )}

      <div className="mx-auto flex w-full max-w-2xl flex-1 flex-col justify-center gap-4 p-6">
        {submitted || ended ? (
          <Card className="space-y-3 p-8 text-center">
            <CheckCircle2 className="mx-auto h-10 w-10 text-green-600" aria-hidden />
            <h1 className="text-xl font-semibold">{submitted ? (autoSubmitted ? "Your exam was submitted automatically" : "Your exam has been submitted") : "This session has ended"}</h1>
            {result ? (
              <div role="status" className="space-y-1">
                <p className="text-4xl font-semibold">{result.percentage}%</p>
                <p className="text-sm">{result.score} of {result.totalPoints} points · <strong>{result.passed ? "Passed" : "Not passed"}</strong></p>
              </div>
            ) : (
              <p className="text-sm text-slate-600 dark:text-slate-300">{submitted ? "Your answers were received. Your teacher will share results if they choose to." : "Thank you. Your teacher will share results if they choose to."}</p>
            )}
            {examError && <p role="alert" className="text-sm text-red-700 dark:text-red-300">{examError}</p>}
            <Button className="mx-auto" onClick={leave}>Done</Button>
          </Card>
        ) : (
          <>
            <Card className="space-y-2 p-6 text-center">
              {status === "WAITING" && (<><h1 className="text-xl font-semibold">You're in. Waiting for the teacher to start…</h1>
                <p className="text-sm text-slate-600 dark:text-slate-300">Keep this window open. The exam begins for everyone at the same time.</p></>)}
              {status === "RUNNING" && (<><h1 className="text-xl font-semibold">The exam is starting…</h1>
                <p className="text-sm text-slate-600 dark:text-slate-300">Loading your questions.</p>
                {examError && <p role="alert" className="text-sm text-red-700 dark:text-red-300">{examError}</p>}</>)}
              {status === "PAUSED" && (<><h1 className="flex items-center justify-center gap-2 text-xl font-semibold"><Pause className="h-5 w-5" aria-hidden /> Exam paused</h1>
                <p className="text-sm text-slate-600 dark:text-slate-300">Your teacher paused the exam. The clock is stopped and will continue when they resume.</p></>)}
              {remaining !== null && (
                <p className="font-mono text-4xl font-semibold" aria-label="Time remaining">{formatClock(remaining)}</p>
              )}
            </Card>
            <Card className="space-y-3 p-6">
              <h2 className="flex items-center gap-2 font-medium"><FileText className="h-4 w-4" aria-hidden /> About this exam</h2>
              <dl className="grid grid-cols-2 gap-3 text-sm">
                <div><dt className="text-xs uppercase text-slate-500">Questions</dt><dd className="font-semibold">{info.exam.questionCount}</dd></div>
                <div><dt className="flex items-center gap-1 text-xs uppercase text-slate-500"><Clock className="h-3 w-3" aria-hidden /> Time allowed</dt><dd className="font-semibold">{info.exam.durationMinutes} minutes</dd></div>
              </dl>
              {info.exam.description && <p className="text-sm">{info.exam.description}</p>}
              {info.exam.instructions && (
                <div>
                  <h3 className="text-xs uppercase text-slate-500">Instructions</h3>
                  <p className="whitespace-pre-wrap text-sm">{info.exam.instructions}</p>
                </div>
              )}
            </Card>
          </>
        )}
      </div>

      <ConfirmDialog open={confirmLeave} title="Leave this session?" confirmLabel="Leave"
        onCancel={() => setConfirmLeave(false)} onConfirm={() => { setConfirmLeave(false); leave(); }}>
        You can rejoin later from the Join screen with the same student ID; your place is saved on this computer.
      </ConfirmDialog>
    </main>
  );
}
