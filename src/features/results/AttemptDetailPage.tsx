import { Check, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { toMessage } from "@/services/auth";
import { resultsApi } from "@/services/results";
import type { AttemptDetail } from "@/types/results";
import { QUESTION_TYPE_LABELS } from "@/types/exam";
import { formatDate } from "@/utils/format";
import { EVENT_LABEL, EVENT_TONE } from "@/features/sessions/proctoring";
import { STATUS_TEXT, pct } from "./format";

export function AttemptDetailPage() {
  const { attemptId = "" } = useParams();
  const [d, setD] = useState<AttemptDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { resultsApi.attempt(attemptId).then(setD).catch((e) => setError(toMessage(e))); }, [attemptId]);

  if (!d) return error ? <p role="alert" className="text-red-700 dark:text-red-300">{error}</p> : <p>Loading…</p>;
  const finished = d.status === "SUBMITTED" || d.status === "AUTO_SUBMITTED";

  return (
    <div className="space-y-4">
      <header>
        <Link to={`/teacher/results/${d.sessionId}`} className="text-sm text-brand-600 underline dark:text-sky-400">← {d.examTitle}</Link>
        <h1 className="text-2xl font-semibold">{d.name} <span className="font-mono text-base font-normal text-slate-500">{d.studentNumber}</span></h1>
        <p className="text-sm text-slate-600 dark:text-slate-300">{STATUS_TEXT[d.status]}{d.submittedAt ? ` · ${formatDate(d.submittedAt)}` : ""}</p>
      </header>

      {finished && (
        <Card className="flex flex-wrap items-center gap-6 p-4">
          <div><p className="text-xs uppercase text-slate-500">Score</p><p className="text-2xl font-semibold">{d.score} / {d.totalPoints}</p></div>
          <div><p className="text-xs uppercase text-slate-500">Percent</p><p className="text-2xl font-semibold">{pct(d.percentage)}</p></div>
          <div><p className="text-xs uppercase text-slate-500">Result</p><Badge tone={d.passed ? "green" : "red"}>{d.passed ? "Passed" : "Not passed"} (needs {d.passingScore}%)</Badge></div>
        </Card>
      )}

      <ol className="space-y-3">
        {d.answers.map((a) => (
          <li key={a.questionId}>
            <Card className="space-y-2 p-4">
              <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-slate-500">
                <span>Question {a.position} · {QUESTION_TYPE_LABELS[a.questionType]}</span>
                <Badge tone={!finished ? "gray" : a.isCorrect ? "green" : "red"}>
                  {finished && (a.isCorrect ? <Check className="h-3 w-3" aria-hidden /> : <X className="h-3 w-3" aria-hidden />)}
                  {finished ? (a.isCorrect ? "Correct" : a.answered ? "Incorrect" : "Not answered") : "Not graded"} · {a.pointsAwarded} / {a.points}
                </Badge>
              </div>
              <p className="whitespace-pre-wrap font-medium">{a.text}</p>
              <p className="text-sm"><span className="text-slate-500">Student answered: </span>{a.given.length ? a.given.join(", ") : <em>no answer</em>}</p>
              {!a.isCorrect && <p className="text-sm"><span className="text-slate-500">Correct answer: </span>{a.correct.join(", ")}</p>}
              {a.explanation && <p className="rounded-md bg-slate-50 p-2 text-sm text-slate-700 dark:bg-navy-800 dark:text-slate-200">{a.explanation}</p>}
            </Card>
          </li>
        ))}
      </ol>

      <Card className="overflow-hidden">
        <h2 className="border-b border-slate-200 px-4 py-3 font-medium dark:border-slate-700">Proctoring timeline</h2>
        {d.events.length === 0 ? <p className="p-4 text-sm">Nothing recorded.</p> : (
          <ol className="divide-y divide-slate-200 text-sm dark:divide-slate-700">
            {d.events.map((e) => (
              <li key={e.id} className="flex flex-wrap items-center gap-2 px-4 py-2">
                <span className="w-24 font-mono text-xs text-slate-500">{new Date(e.createdAt).toLocaleTimeString()}</span>
                <Badge tone={EVENT_TONE[e.eventType]}>{EVENT_LABEL[e.eventType]}</Badge>
                <span className="text-slate-600 dark:text-slate-300">{e.description}</span>
              </li>
            ))}
          </ol>
        )}
        <p className="border-t border-slate-200 px-4 py-2 text-xs text-slate-500 dark:border-slate-700">Signals, not proof. See the proctoring guide for what they cannot detect.</p>
      </Card>
    </div>
  );
}
