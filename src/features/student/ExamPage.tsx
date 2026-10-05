import { AlertTriangle, ChevronLeft, ChevronRight, Cloud, CloudOff, Pause, Send } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { useStudent } from "@/stores/student";
import { QUESTION_TYPE_LABELS } from "@/types/exam";
import { cn } from "@/utils/cn";
import { formatClock } from "@/utils/format";
import { displayRemaining, monotonicNow } from "@/utils/sessionClock";
import { answeredCount, isAnswered, timerAnnouncement, timerTone, unansweredIndexes } from "./paper";
import { QuestionInput } from "./QuestionInput";

const TONE = { normal: "text-slate-900 dark:text-white", warning: "text-amber-600 dark:text-amber-400", danger: "text-red-600 dark:text-red-400" } as const;

/** The live exam: one question at a time, autosaved, with a server-clock countdown. */
export function ExamPage() {
  const s = useStudent();
  const { paper, answers, current, status, info } = s;
  const [now, setNow] = useState(() => monotonicNow());
  const [confirm, setConfirm] = useState(false);
  const spoken = useRef<string | null>(null);
  const [announce, setAnnounce] = useState("");

  useEffect(() => {
    const t = setInterval(() => setNow(monotonicNow()), 1000);
    return () => clearInterval(t);
  }, []);

  const remaining = displayRemaining({ status, remainingSeconds: s.remainingSeconds }, s.receivedAt, now);
  useEffect(() => {
    const msg = status === "RUNNING" ? timerAnnouncement(remaining) : null;
    if (msg && msg !== spoken.current) { spoken.current = msg; setAnnounce(msg); }
  }, [remaining, status]);

  if (!paper || !info) return null;
  const q = paper.questions[current];
  const total = paper.questions.length;
  const paused = status === "PAUSED";
  const locked = paused || s.timeUp || s.submitting;
  const done = answeredCount(paper, answers);
  const missing = unansweredIndexes(paper, answers);
  const save = s.saveState();
  const isLast = current === total - 1;

  return (
    <main className="flex min-h-full flex-col">
      <header className="flex flex-wrap items-center gap-3 border-b border-slate-200 bg-white px-4 py-3 dark:border-slate-700 dark:bg-navy-900">
        <div className="min-w-0 flex-1">
          <p className="truncate font-medium">{info.exam.title}</p>
          <p className="truncate text-xs text-slate-500">{info.studentName} · {info.studentId}</p>
        </div>
        <Badge tone={save === "saved" ? "green" : "amber"} aria-live="polite">
          {save === "saved" ? <Cloud className="h-3 w-3" aria-hidden /> : <CloudOff className="h-3 w-3" aria-hidden />}
          {save === "saved" ? "All answers saved" : save === "saving" ? "Saving…" : `Offline — ${s.pendingCount()} ${s.pendingCount() === 1 ? "answer" : "answers"} kept on this computer, will sync`}
        </Badge>
        <div className="text-right">
          <p className="text-[10px] uppercase text-slate-500">Time remaining</p>
          <p className={cn("font-mono text-2xl font-semibold leading-none", TONE[timerTone(remaining)])} aria-hidden>{remaining === null ? "--:--:--" : formatClock(remaining)}</p>
          <span className="sr-only">Time remaining {remaining === null ? "unknown" : formatClock(remaining)}</span>
        </div>
      </header>
      <p className="sr-only" role="status" aria-live="assertive">{announce}</p>

      {paused && (
        <p role="alert" className="flex items-center justify-center gap-2 bg-amber-50 px-4 py-2 text-sm text-amber-800 dark:bg-amber-950 dark:text-amber-300">
          <Pause className="h-4 w-4" aria-hidden /> The teacher paused the exam. Your clock is stopped; you can continue when they resume.
        </p>
      )}
      {s.timeUp && (
        <p role="alert" className="flex items-center justify-center gap-2 bg-red-50 px-4 py-2 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">
          <AlertTriangle className="h-4 w-4" aria-hidden /> Time is up. Answers are locked. Press Submit if your exam has not been submitted automatically.
        </p>
      )}
      {s.examError && <p role="alert" className="bg-red-50 px-4 py-2 text-center text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{s.examError}</p>}

      <div className="mx-auto grid w-full max-w-5xl flex-1 gap-4 p-4 md:grid-cols-[1fr_14rem]">
        <Card className="flex flex-col gap-5 p-6">
          <div className="flex items-center justify-between text-xs text-slate-500">
            <span>Question {current + 1} of {total}</span>
            <span>{QUESTION_TYPE_LABELS[q.type]} · {q.points} {q.points === 1 ? "point" : "points"}</span>
          </div>
          <h1 className="whitespace-pre-wrap text-lg font-medium">{q.text}</h1>
          <QuestionInput key={q.id} question={q} value={answers[q.id]} disabled={locked} onChange={(v) => s.setAnswer(q.id, v)} />

          <div className="mt-auto flex items-center justify-between gap-2 pt-2">
            {paper.allowReview
              ? <Button variant="outline" onClick={() => s.goTo(current - 1)} disabled={current === 0}><ChevronLeft className="h-4 w-4" aria-hidden /> Previous</Button>
              : <span />}
            {isLast
              ? <Button onClick={() => setConfirm(true)} disabled={paused || s.submitting}><Send className="h-4 w-4" aria-hidden /> {s.submitting ? "Submitting…" : "Submit exam"}</Button>
              : <Button onClick={() => s.goTo(current + 1)}>Next <ChevronRight className="h-4 w-4" aria-hidden /></Button>}
          </div>
        </Card>

        <aside className="space-y-3" aria-label="Progress">
          <Card className="space-y-3 p-4">
            <p className="text-sm font-medium">{done} of {total} answered</p>
            <div className="h-2 overflow-hidden rounded-full bg-slate-200 dark:bg-slate-700" role="progressbar" aria-valuemin={0} aria-valuemax={total} aria-valuenow={done}>
              <div className="h-full bg-brand-600" style={{ width: `${total ? (done / total) * 100 : 0}%` }} />
            </div>
            {paper.allowReview ? (
              <nav aria-label="Question navigator" className="grid grid-cols-5 gap-1.5">
                {paper.questions.map((pq, i) => (
                  <button key={pq.id} onClick={() => s.goTo(i)} aria-label={`Question ${i + 1}${isAnswered(answers[pq.id]) ? ", answered" : ", not answered"}`} aria-current={i === current ? "step" : undefined}
                    className={cn("h-8 rounded-md border text-xs font-medium",
                      i === current ? "border-brand-600 ring-2 ring-brand-600/40" : "border-slate-300 dark:border-slate-600",
                      isAnswered(answers[pq.id]) ? "bg-brand-600 text-white" : "bg-white dark:bg-navy-950")}>
                    {i + 1}
                  </button>
                ))}
              </nav>
            ) : <p className="text-xs text-slate-500">Review is off for this exam: you can only move forward.</p>}
          </Card>
          <Button className="w-full" variant="outline" onClick={() => setConfirm(true)} disabled={paused || s.submitting}><Send className="h-4 w-4" aria-hidden /> Submit exam</Button>
        </aside>
      </div>

      <ConfirmDialog open={confirm} title="Submit your exam?" confirmLabel="Submit"
        onCancel={() => setConfirm(false)} onConfirm={() => { setConfirm(false); void s.submit(); }}>
        {missing.length > 0
          ? <p className="font-medium text-amber-700 dark:text-amber-400">You have {missing.length} unanswered {missing.length === 1 ? "question" : "questions"}: {missing.slice(0, 12).map((i) => i + 1).join(", ")}{missing.length > 12 ? "…" : ""}.</p>
          : <p>You answered every question.</p>}
        <p className="mt-2">After you submit you cannot change your answers.</p>
      </ConfirmDialog>
    </main>
  );
}
