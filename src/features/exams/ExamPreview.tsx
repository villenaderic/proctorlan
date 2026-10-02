import { Flag } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { DraftQuestion, ExamDraft } from "@/types/exam";
import { cn } from "@/utils/cn";
import { formatClock } from "@/utils/format";
import { totalPoints } from "./draft";

type Answer = string[] | string;
const isAnswered = (a: Answer | undefined) => (Array.isArray(a) ? a.length > 0 : !!a?.trim());

/**
 * Teacher-side simulation of the student experience. Everything lives in component state:
 * it never creates an attempt, never touches the database and never contacts the network.
 */
export function ExamPreview({ draft }: { draft: ExamDraft }) {
  const qs = draft.questions;
  const [index, setIndex] = useState(0);
  const [answers, setAnswers] = useState<Record<string, Answer>>({});
  const [marked, setMarked] = useState<Set<string>>(new Set());
  const [seconds, setSeconds] = useState(draft.durationMinutes * 60);
  const [running, setRunning] = useState(true);
  const [reviewOpen, setReviewOpen] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [done, setDone] = useState<null | "submitted" | "timeout">(null);

  useEffect(() => { setSeconds(draft.durationMinutes * 60); }, [draft.durationMinutes]);
  useEffect(() => {
    if (!running || done) return;
    const t = setInterval(() => setSeconds((s) => Math.max(0, s - 1)), 1000);
    return () => clearInterval(t);
  }, [running, done]);
  useEffect(() => { if (seconds === 0 && !done) setDone("timeout"); }, [seconds, done]);

  const q: DraftQuestion | undefined = qs[Math.min(index, qs.length - 1)];
  const answeredCount = useMemo(() => qs.filter((x) => isAnswered(answers[x.uid])).length, [qs, answers]);

  if (qs.length === 0 || !q) return <Card className="p-6 text-sm text-slate-600 dark:text-slate-300">Add at least one question to preview the exam.</Card>;

  const setAnswer = (a: Answer) => setAnswers((prev) => ({ ...prev, [q.uid]: a }));
  const toggleMark = () => setMarked((m) => { const n = new Set(m); if (n.has(q.uid)) n.delete(q.uid); else n.add(q.uid); return n; });
  const goTo = (i: number) => { if (draft.allowReview || i >= index) setIndex(i); };
  const status = (x: DraftQuestion, i: number) => (i === index ? "current" : marked.has(x.uid) ? "marked" : isAnswered(answers[x.uid]) ? "answered" : "unanswered");

  const restart = () => { setIndex(0); setAnswers({}); setMarked(new Set()); setSeconds(draft.durationMinutes * 60); setRunning(true); setDone(null); setReviewOpen(false); };

  if (done)
    return (
      <Card className="space-y-3 p-8 text-center">
        <h3 className="text-lg font-semibold">{done === "timeout" ? "Time is up" : "Exam submitted"}</h3>
        <p className="text-sm text-slate-600 dark:text-slate-300">
          {done === "timeout" && draft.autoSubmit ? "With auto-submit on, students’ answers would be submitted automatically at this point. " : ""}
          You answered {answeredCount} of {qs.length} questions in this preview.
        </p>
        <p className="text-xs text-slate-500">Preview only — nothing was saved or graded.</p>
        <div><Button variant="outline" onClick={restart}>Restart preview</Button></div>
      </Card>
    );

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg bg-navy-900 px-4 py-2 text-white">
        <div>
          <p className="text-sm font-medium">{draft.title || "Untitled exam"}</p>
          <p className="text-xs text-slate-300">Duration {draft.durationMinutes} min · Passing {draft.passingScore}% · {totalPoints(draft)} points</p>
        </div>
        <div className="flex items-center gap-3">
          <span className="font-mono text-lg" role="timer" aria-label="Time remaining">{formatClock(seconds)}</span>
          <Button size="sm" variant="outline" className="border-slate-500 bg-transparent text-white hover:bg-navy-800" onClick={() => setRunning((r) => !r)}>{running ? "Pause" : "Resume"}</Button>
          <Button size="sm" variant="outline" className="border-slate-500 bg-transparent text-white hover:bg-navy-800" onClick={restart}>Reset</Button>
        </div>
      </div>
      <p className="text-xs text-slate-500">Timer simulation. {draft.randomizeQuestions && "Question order is randomized per student. "}{draft.randomizeChoices && "Choice order is randomized per student. "}{!draft.allowReview && "Review is off: students cannot go back to earlier questions."}</p>

      <div className="grid gap-4 lg:grid-cols-[1fr_16rem]">
        <Card className="space-y-4 p-5">
          <div className="flex items-center justify-between text-sm text-slate-600 dark:text-slate-300">
            <span>Question {index + 1} of {qs.length}</span>
            <span>{q.points} {q.points === 1 ? "point" : "points"}{q.required ? "" : " · optional"}</span>
          </div>
          <p className="whitespace-pre-wrap text-base font-medium">{q.questionText || <em className="text-slate-400">No question text yet</em>}</p>

          {q.questionType === "identification" ? (
            <Input aria-label="Your answer" placeholder="Type your answer" value={typeof answers[q.uid] === "string" ? (answers[q.uid] as string) : ""} onChange={(e) => setAnswer(e.target.value)} />
          ) : (
            <fieldset className="space-y-2">
              <legend className="sr-only">Answer choices</legend>
              {q.choices.map((c) => {
                const multi = q.questionType === "multiple_select";
                const sel = Array.isArray(answers[q.uid]) ? (answers[q.uid] as string[]) : [];
                const checked = sel.includes(c.uid);
                return (
                  <label key={c.uid} className={cn("flex cursor-pointer items-center gap-3 rounded-md border p-3 text-sm", checked ? "border-brand-600 bg-brand-50 dark:bg-navy-800" : "border-slate-200 dark:border-slate-700")}>
                    <input type={multi ? "checkbox" : "radio"} name={`q-${q.uid}`} className="h-4 w-4 accent-brand-600" checked={checked}
                      onChange={() => setAnswer(multi ? (checked ? sel.filter((x) => x !== c.uid) : [...sel, c.uid]) : [c.uid])} />
                    {c.choiceText || <em className="text-slate-400">Empty choice</em>}
                  </label>
                );
              })}
            </fieldset>
          )}
          {q.questionType === "multiple_select" && <p className="text-xs text-slate-500">Select all that apply.</p>}

          <div className="flex flex-wrap items-center gap-2 border-t border-slate-200 pt-4 dark:border-slate-700">
            {draft.allowReview && <Button variant="outline" disabled={index === 0} onClick={() => setIndex(index - 1)}>Previous</Button>}
            <Button variant="outline" disabled={index === qs.length - 1} onClick={() => setIndex(index + 1)}>Next</Button>
            {draft.allowReview && <Button variant="outline" onClick={toggleMark} aria-pressed={marked.has(q.uid)}><Flag className="h-4 w-4" aria-hidden /> {marked.has(q.uid) ? "Unmark" : "Mark for review"}</Button>}
            {draft.allowReview && <Button variant="outline" onClick={() => setReviewOpen((o) => !o)} aria-expanded={reviewOpen}>Review</Button>}
            <Button className="ml-auto" onClick={() => setConfirmOpen(true)}>Submit</Button>
          </div>
          {reviewOpen && draft.allowReview && (
            <ul className="space-y-1 rounded-md bg-slate-50 p-3 text-sm dark:bg-navy-950">
              {qs.map((x, i) => (
                <li key={x.uid}><button className="flex w-full items-center gap-2 rounded px-2 py-1 text-left hover:bg-slate-200/60 dark:hover:bg-navy-800" onClick={() => { setIndex(i); setReviewOpen(false); }}>
                  <span className="w-8 font-medium">{i + 1}.</span>
                  <span className="flex-1 truncate">{x.questionText || "(no text)"}</span>
                  {marked.has(x.uid) && <Badge tone="amber">Marked</Badge>}
                  <Badge tone={isAnswered(answers[x.uid]) ? "green" : "gray"}>{isAnswered(answers[x.uid]) ? "Answered" : "Unanswered"}</Badge>
                </button></li>
              ))}
            </ul>
          )}
        </Card>

        <Card className="space-y-3 p-4">
          <h3 className="text-sm font-medium">Question navigator</h3>
          <div className="grid grid-cols-5 gap-1.5">
            {qs.map((x, i) => {
              const s = status(x, i);
              return (
                <button key={x.uid} onClick={() => goTo(i)} aria-label={`Question ${i + 1}, ${s}`} aria-current={i === index ? "true" : undefined} disabled={!draft.allowReview && i < index}
                  className={cn("relative h-8 rounded text-xs font-medium disabled:opacity-40",
                    s === "current" && "bg-brand-600 text-white ring-2 ring-brand-600 ring-offset-1",
                    s === "answered" && "bg-green-600 text-white",
                    s === "marked" && "border-2 border-amber-500 bg-amber-100 text-amber-900 dark:bg-amber-950 dark:text-amber-200",
                    s === "unanswered" && "border border-slate-300 bg-white dark:border-slate-600 dark:bg-navy-950")}>
                  {i + 1}{marked.has(x.uid) && s !== "marked" && <Flag className="absolute -right-1 -top-1 h-3 w-3 text-amber-500" aria-hidden />}
                </button>
              );
            })}
          </div>
          <ul className="space-y-1 text-xs text-slate-600 dark:text-slate-300">
            <li><span className="mr-2 inline-block h-3 w-3 rounded bg-brand-600 align-middle" />Current</li>
            <li><span className="mr-2 inline-block h-3 w-3 rounded bg-green-600 align-middle" />Answered</li>
            <li><span className="mr-2 inline-block h-3 w-3 rounded border border-slate-400 align-middle" />Unanswered</li>
            <li><span className="mr-2 inline-block h-3 w-3 rounded border-2 border-amber-500 bg-amber-100 align-middle" />Marked for review</li>
          </ul>
        </Card>
      </div>

      <ConfirmDialog open={confirmOpen} title="Submit exam?" confirmLabel="Submit exam" onCancel={() => setConfirmOpen(false)} onConfirm={() => { setConfirmOpen(false); setDone("submitted"); }}>
        You have answered {answeredCount} of {qs.length} questions. Are you sure you want to submit?
        <span className="mt-2 block text-xs text-slate-500">(Preview only — nothing will be saved.)</span>
      </ConfirmDialog>
    </div>
  );
}
