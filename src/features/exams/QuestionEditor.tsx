import { ChevronDown, ChevronUp, Copy, Plus, Trash2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { CheckField, Select, Textarea } from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { QUESTION_TYPE_LABELS, type DraftQuestion, type QuestionType, type ValidationIssue } from "@/types/exam";
import { addChoice, changeType, MAX_CHOICES, removeChoice, setCorrect, updateChoice } from "./draft";

interface Props {
  index: number;
  total: number;
  question: DraftQuestion;
  issues: ValidationIssue[];
  open: boolean;
  onToggle: () => void;
  onChange: (fn: (q: DraftQuestion) => DraftQuestion) => void;
  onMove: (dir: -1 | 1) => void;
  onDuplicate: () => void;
  onDelete: () => void;
}

export function QuestionEditor({ index, total, question: q, issues, open, onToggle, onChange, onMove, onDuplicate, onDelete }: Props) {
  const n = index + 1;
  const multi = q.questionType === "multiple_select";
  const fixedChoices = q.questionType === "true_false";
  const minChoices = q.questionType === "identification" ? 1 : 2;
  const issueFor = (field: string) => issues.filter((i) => i.path.endsWith(`.${field}`)).map((i) => i.message);

  return (
    <Card className={issues.length ? "border-amber-400 dark:border-amber-600" : ""}>
      <div className="flex flex-wrap items-center gap-2 p-3">
        <button type="button" onClick={onToggle} aria-expanded={open} aria-label={`Question ${n}: ${open ? "collapse" : "expand"}`} className="flex min-w-0 flex-1 items-center gap-3 text-left">
          {open ? <ChevronUp className="h-4 w-4 shrink-0" aria-hidden /> : <ChevronDown className="h-4 w-4 shrink-0" aria-hidden />}
          <span className="font-semibold">{n}.</span>
          <span className="truncate text-sm">{q.questionText || <em className="text-slate-400">Untitled question</em>}</span>
        </button>
        <Badge tone="blue">{QUESTION_TYPE_LABELS[q.questionType]}</Badge>
        <Badge>{q.points} pt</Badge>
        {issues.length > 0 && <Badge tone="amber">⚠ {issues.length} to fix</Badge>}
        <div className="flex gap-0.5">
          <Button size="icon" variant="ghost" aria-label={`Move question ${n} up`} disabled={index === 0} onClick={() => onMove(-1)}><ChevronUp className="h-4 w-4" /></Button>
          <Button size="icon" variant="ghost" aria-label={`Move question ${n} down`} disabled={index === total - 1} onClick={() => onMove(1)}><ChevronDown className="h-4 w-4" /></Button>
          <Button size="icon" variant="ghost" aria-label={`Duplicate question ${n}`} onClick={onDuplicate}><Copy className="h-4 w-4" /></Button>
          <Button size="icon" variant="ghost" aria-label={`Delete question ${n}`} onClick={onDelete}><Trash2 className="h-4 w-4" /></Button>
        </div>
      </div>

      {open && (
        <div className="space-y-4 border-t border-slate-200 p-4 dark:border-slate-700">
          <div className="grid gap-4 sm:grid-cols-[1fr_8rem_12rem]">
            <div className="sm:col-span-3">
              <label htmlFor={`qt-${q.uid}`} className="text-sm font-medium">Question text</label>
              <Textarea id={`qt-${q.uid}`} value={q.questionText} onChange={(e) => onChange((x) => ({ ...x, questionText: e.target.value }))} />
              {issueFor("text").map((m) => <p key={m} role="alert" className="text-xs text-red-600 dark:text-red-400">{m}</p>)}
            </div>
            <div>
              <label htmlFor={`ty-${q.uid}`} className="text-sm font-medium">Type</label>
              <Select id={`ty-${q.uid}`} value={q.questionType} onChange={(e) => onChange((x) => changeType(x, e.target.value as QuestionType))}>
                {Object.entries(QUESTION_TYPE_LABELS).map(([v, l]) => <option key={v} value={v}>{l}</option>)}
              </Select>
            </div>
            <div>
              <label htmlFor={`pt-${q.uid}`} className="text-sm font-medium">Points</label>
              <Input id={`pt-${q.uid}`} type="number" min={0.5} step={0.5} value={q.points} onChange={(e) => onChange((x) => ({ ...x, points: e.target.valueAsNumber }))} />
              {issueFor("points").map((m) => <p key={m} role="alert" className="text-xs text-red-600 dark:text-red-400">{m}</p>)}
            </div>
            <div className="flex items-end pb-2"><CheckField label="Required" checked={q.required} onChange={(e) => onChange((x) => ({ ...x, required: e.target.checked }))} /></div>
          </div>

          <div className="space-y-2">
            <p className="text-sm font-medium">{q.questionType === "identification" ? "Accepted answers" : multi ? "Choices (tick every correct one)" : "Choices (select the correct one)"}</p>
            {q.choices.map((c, ci) => (
              <div key={c.uid} className="flex items-center gap-2">
                {q.questionType !== "identification" && (
                  <input type={multi ? "checkbox" : "radio"} name={`correct-${q.uid}`} checked={c.isCorrect} className="h-4 w-4 shrink-0 accent-brand-600"
                    aria-label={`Mark choice ${ci + 1} as correct`} onChange={(e) => onChange((x) => setCorrect(x, c.uid, e.target.checked))} />
                )}
                {fixedChoices ? <span className="flex-1 rounded-md border border-slate-200 px-3 py-2 text-sm dark:border-slate-700">{c.choiceText}</span> : (
                  <Input aria-label={q.questionType === "identification" ? `Accepted answer ${ci + 1}` : `Choice ${ci + 1}`} value={c.choiceText}
                    placeholder={q.questionType === "identification" ? "e.g. Paris" : `Choice ${String.fromCharCode(65 + ci)}`} onChange={(e) => onChange((x) => updateChoice(x, c.uid, e.target.value))} />
                )}
                {!fixedChoices && <Button size="icon" variant="ghost" aria-label={`Remove choice ${ci + 1}`} disabled={q.choices.length <= minChoices} onClick={() => onChange((x) => removeChoice(x, c.uid))}><X className="h-4 w-4" /></Button>}
              </div>
            ))}
            {!fixedChoices && <Button size="sm" variant="outline" disabled={q.choices.length >= MAX_CHOICES} onClick={() => onChange(addChoice)}><Plus className="h-4 w-4" aria-hidden /> {q.questionType === "identification" ? "Add accepted answer" : "Add choice"}</Button>}
            {q.questionType === "identification" && <p className="text-xs text-slate-500">Matching ignores capital letters and extra spaces.</p>}
            {[...issueFor("choices"), ...issueFor("correct")].map((m) => <p key={m} role="alert" className="text-xs text-red-600 dark:text-red-400">{m}</p>)}
          </div>

          <div>
            <label htmlFor={`ex-${q.uid}`} className="text-sm font-medium">Explanation <span className="font-normal text-slate-500">(optional, shown with results if you allow it)</span></label>
            <Textarea id={`ex-${q.uid}`} className="min-h-14" value={q.explanation} onChange={(e) => onChange((x) => ({ ...x, explanation: e.target.value }))} />
          </div>
        </div>
      )}
    </Card>
  );
}
