import { AlertTriangle, CheckCircle2, Lock, Plus } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { CheckField, Select, Textarea } from "@/components/ui/form";
import { Field } from "@/components/Field";
import { isSessionExpired, toMessage } from "@/services/auth";
import { examsApi } from "@/services/exams";
import { refreshAuth } from "@/stores/auth";
import { QUESTION_TYPE_LABELS, type ExamDraft, type QuestionType, type ValidationIssue } from "@/types/exam";
import { cn } from "@/utils/cn";
import {
  addQuestion, deleteQuestion, duplicateQuestion, fromExam, issuesByQuestion, moveQuestion, newDraft, toPayload, totalPoints, updateQuestion,
} from "./draft";
import { ExamPreview } from "./ExamPreview";
import { QuestionEditor } from "./QuestionEditor";

const STEPS = ["Basic info", "Questions", "Settings", "Preview"] as const;
const BASIC_PATHS = ["title", "description", "instructions", "durationMinutes", "passingScore"];

export function ExamBuilderPage() {
  const { id } = useParams();
  const nav = useNavigate();
  const [draft, setDraft] = useState<ExamDraft>(newDraft);
  const [loaded, setLoaded] = useState(!id);
  const [step, setStep] = useState(0);
  const [openUid, setOpenUid] = useState<string | null>(null);
  const [newType, setNewType] = useState<QuestionType>("multiple_choice");
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [confirmCancel, setConfirmCancel] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
  const [sessionCount, setSessionCount] = useState(0);
  const baseline = useRef<string>(JSON.stringify(newDraft()));

  const fail = (e: unknown) => {
    const text = toMessage(e);
    if (isSessionExpired(text)) refreshAuth();
    setError(text);
  };

  useEffect(() => {
    if (!id) return;
    examsApi.get(id).then((full) => {
      const d = fromExam(full);
      baseline.current = JSON.stringify(d);
      setDraft(d);
      setSessionCount(full.sessionCount);
      setLoaded(true);
    }).catch(fail);
  }, [id]);

  // Live validation from the backend (single source of truth), debounced.
  useEffect(() => {
    if (!loaded) return;
    const t = setTimeout(() => { examsApi.validate(toPayload(draft)).then(setIssues).catch(() => undefined); }, 350);
    return () => clearTimeout(t);
  }, [draft, loaded]);

  const dirty = JSON.stringify(draft) !== baseline.current;
  const locked = sessionCount > 0;
  const perQuestion = useMemo(() => issuesByQuestion(issues), [issues]);
  const basicIssues = issues.filter((i) => BASIC_PATHS.includes(i.path));
  const questionIssues = issues.filter((i) => i.path === "questions" || i.path.startsWith("questions."));
  const issueFor = (path: string) => issues.find((i) => i.path === path)?.message ?? null;
  const set = <K extends keyof ExamDraft>(key: K, value: ExamDraft[K]) => setDraft((d) => ({ ...d, [key]: value }));
  const stepIssueCount = [basicIssues.length, questionIssues.length, 0, 0];

  async function save() {
    setSaving(true);
    setError(null);
    try {
      const payload = toPayload(draft);
      const saved = id ? await examsApi.update(id, payload) : await examsApi.create(payload);
      nav("/teacher/exams", { state: { notice: `Saved "${saved.title}".` } });
    } catch (e) { fail(e); setSaving(false); }
  }

  if (!loaded) return error ? <p role="alert" className="text-red-600">{error}</p> : <p className="text-slate-500" role="status">Loading…</p>;

  const cancel = () => (dirty ? setConfirmCancel(true) : nav("/teacher/exams"));
  const pendingQuestionNumber = pendingDelete ? draft.questions.findIndex((q) => q.uid === pendingDelete) + 1 : 0;

  return (
    <div className="mx-auto max-w-4xl space-y-5">
      <header>
        <h1 className="text-2xl font-semibold">{id ? "Edit exam" : "Create exam"}</h1>
        {dirty && <p className="text-xs text-amber-600 dark:text-amber-400">Unsaved changes</p>}
      </header>

      {locked && (
        <p role="status" className="flex items-start gap-2 rounded-md bg-amber-50 p-3 text-sm text-amber-900 dark:bg-amber-950 dark:text-amber-200">
          <Lock className="mt-0.5 h-4 w-4 shrink-0" aria-hidden /> This exam has been used in a session, so it can no longer be edited (results must stay accurate). Go back and use Duplicate to make a changed copy.
        </p>
      )}

      <nav aria-label="Builder steps"><ol className="flex flex-wrap gap-2">
        {STEPS.map((label, i) => (
          <li key={label}>
            <button type="button" onClick={() => setStep(i)} aria-current={i === step ? "step" : undefined}
              className={cn("flex items-center gap-2 rounded-full border px-4 py-1.5 text-sm", i === step ? "border-brand-600 bg-brand-600 text-white" : "border-slate-300 hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-navy-800")}>
              <span className="font-semibold">{i + 1}</span> {label}
              {stepIssueCount[i] > 0 && <span className="rounded-full bg-amber-400 px-1.5 text-xs font-semibold text-black" aria-label={`${stepIssueCount[i]} issues`}>{stepIssueCount[i]}</span>}
            </button>
          </li>
        ))}
      </ol></nav>

      <fieldset disabled={locked} className="min-w-0 space-y-4">
        <legend className="sr-only">{STEPS[step]}</legend>

        {step === 0 && (
          <Card className="grid gap-4 p-5 sm:grid-cols-2">
            <div className="sm:col-span-2"><Field label="Title *" value={draft.title} maxLength={200} onChange={(e) => set("title", e.target.value)} error={issueFor("title")} placeholder="e.g. Introduction to Programming" autoFocus /></div>
            <div className="sm:col-span-2">
              <label htmlFor="desc" className="text-sm font-medium">Description</label>
              <Textarea id="desc" value={draft.description} onChange={(e) => set("description", e.target.value)} placeholder="What is this exam about?" />
            </div>
            <div className="sm:col-span-2">
              <label htmlFor="instr" className="text-sm font-medium">Instructions for students</label>
              <Textarea id="instr" value={draft.instructions} onChange={(e) => set("instructions", e.target.value)} placeholder="Read each question carefully. Select the best answer." />
            </div>
            <Field label="Duration (minutes)" type="number" min={1} value={draft.durationMinutes} onChange={(e) => set("durationMinutes", e.target.valueAsNumber)} error={issueFor("durationMinutes")} />
            <Field label="Passing score (%)" type="number" min={0} max={100} value={draft.passingScore} onChange={(e) => set("passingScore", e.target.valueAsNumber)} error={issueFor("passingScore")} />
            <p className="text-sm text-slate-600 sm:col-span-2 dark:text-slate-300">Total points: <strong>{totalPoints(draft)}</strong> <span className="text-slate-500">(sum of all question points)</span></p>
          </Card>
        )}

        {step === 1 && (
          <div className="space-y-3">
            {questionIssues.some((i) => i.path === "questions") && <p role="alert" className="text-sm text-amber-700 dark:text-amber-300">{questionIssues.find((i) => i.path === "questions")?.message}</p>}
            {draft.questions.map((q, i) => (
              <QuestionEditor key={q.uid} index={i} total={draft.questions.length} question={q} issues={perQuestion.get(i) ?? []} open={openUid === q.uid}
                onToggle={() => setOpenUid(openUid === q.uid ? null : q.uid)}
                onChange={(fn) => setDraft((d) => updateQuestion(d, q.uid, fn))}
                onMove={(dir) => setDraft((d) => moveQuestion(d, q.uid, dir))}
                onDuplicate={() => setDraft((d) => duplicateQuestion(d, q.uid))}
                onDelete={() => setPendingDelete(q.uid)} />
            ))}
            <div className="flex flex-wrap items-center gap-2">
              <Select aria-label="Type for the new question" className="w-48" value={newType} onChange={(e) => setNewType(e.target.value as QuestionType)}>
                {Object.entries(QUESTION_TYPE_LABELS).map(([v, l]) => <option key={v} value={v}>{l}</option>)}
              </Select>
              <Button variant="outline" onClick={() => { const d = addQuestion(draft, newType); setDraft(d); setOpenUid(d.questions[d.questions.length - 1].uid); }}><Plus className="h-4 w-4" aria-hidden /> Add question</Button>
              <span className="text-sm text-slate-500">{draft.questions.length} questions · {totalPoints(draft)} points</span>
            </div>
          </div>
        )}

        {step === 2 && (
          <Card className="space-y-5 p-5">
            <CheckField label="Randomize question order" hint="Each student gets the questions in a different order." checked={draft.randomizeQuestions} onChange={(e) => set("randomizeQuestions", e.target.checked)} />
            <CheckField label="Randomize choice order" hint="Choices of multiple choice and multiple select questions are shuffled per student." checked={draft.randomizeChoices} onChange={(e) => set("randomizeChoices", e.target.checked)} />
            <CheckField label="Allow students to review answers" hint="When off, students cannot go back to earlier questions." checked={draft.allowReview} onChange={(e) => set("allowReview", e.target.checked)} />
            <CheckField label="Auto-submit when time runs out" hint="Answers are saved and submitted automatically at the deadline." checked={draft.autoSubmit} onChange={(e) => set("autoSubmit", e.target.checked)} />
            <CheckField label="Show results to students after submission" hint="When off, only you can see scores." checked={draft.showResults} onChange={(e) => set("showResults", e.target.checked)} />
          </Card>
        )}

        {step === 3 && (
          <div className="space-y-4">
            <Card className="p-4">
              {issues.length === 0 ? (
                <p className="flex items-center gap-2 text-sm font-medium text-green-700 dark:text-green-400"><CheckCircle2 className="h-4 w-4" aria-hidden /> Ready to run. After saving, activate it from the exam list.</p>
              ) : (
                <div className="space-y-2">
                  <p className="flex items-center gap-2 text-sm font-medium text-amber-700 dark:text-amber-300"><AlertTriangle className="h-4 w-4" aria-hidden /> Not ready to run yet — you can still save it as a draft.</p>
                  <ul className="list-disc space-y-1 pl-6 text-sm">{issues.slice(0, 8).map((i) => <li key={i.path + i.message}>{i.message}</li>)}{issues.length > 8 && <li>…and {issues.length - 8} more</li>}</ul>
                </div>
              )}
            </Card>
            <ExamPreview draft={draft} />
          </div>
        )}
      </fieldset>

      {error && <p role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>}

      <footer className="flex flex-wrap items-center justify-between gap-2 border-t border-slate-200 pt-4 dark:border-slate-700">
        <Button variant="outline" onClick={cancel}>Cancel</Button>
        <div className="flex items-center gap-2">
          {step > 0 && <Button variant="outline" onClick={() => setStep(step - 1)}>Back</Button>}
          {step < STEPS.length - 1 && <Button variant="outline" onClick={() => setStep(step + 1)}>Next</Button>}
          <Button disabled={saving || locked || basicIssues.length > 0 || questionIssues.some((i) => /points|choices.*at most|text.*at most/.test(i.message))} onClick={save}>{saving ? "Saving…" : id ? "Save changes" : "Save exam"}</Button>
          {draft.questions.length > 0 && issues.length === 0 && <Badge tone="green">✓ Ready</Badge>}
        </div>
      </footer>

      <ConfirmDialog open={confirmCancel} title="Discard changes?" confirmLabel="Discard" danger onCancel={() => setConfirmCancel(false)} onConfirm={() => nav("/teacher/exams")}>
        You have unsaved changes. If you leave now they will be lost.
      </ConfirmDialog>
      <ConfirmDialog open={!!pendingDelete} title={`Delete question ${pendingQuestionNumber}?`} confirmLabel="Delete" danger onCancel={() => setPendingDelete(null)}
        onConfirm={() => { if (pendingDelete) setDraft((d) => deleteQuestion(d, pendingDelete)); setPendingDelete(null); }}>
        This removes the question from the exam draft. It is only permanent once you save.
      </ConfirmDialog>
    </div>
  );
}
