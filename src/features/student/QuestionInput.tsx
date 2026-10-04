import { useEffect, useId, useRef, useState } from "react";
import type { AnswerValue, PaperQuestion } from "@/types/paper";
import { cn } from "@/utils/cn";

interface Props {
  question: PaperQuestion;
  value: AnswerValue | undefined;
  disabled: boolean;
  onChange: (v: AnswerValue) => void;
}

const optionClass = (selected: boolean, disabled: boolean) =>
  cn("flex cursor-pointer items-start gap-3 rounded-lg border p-3 text-sm transition-colors",
    selected ? "border-brand-600 bg-brand-50 dark:bg-navy-800" : "border-slate-300 hover:bg-slate-50 dark:border-slate-600 dark:hover:bg-navy-800",
    disabled && "cursor-not-allowed opacity-60");

/** The answer control for one question. Native radios/checkboxes keep keyboard and screen-reader behaviour. */
export function QuestionInput({ question, value, disabled, onChange }: Props) {
  const name = useId();
  if (question.type === "identification") return <TextAnswer question={question} value={typeof value === "string" ? value : ""} disabled={disabled} onChange={onChange} />;

  const multi = question.type === "multiple_select";
  const selected = new Set(Array.isArray(value) ? value : typeof value === "string" && value ? [value] : []);

  return (
    <fieldset className="space-y-2" disabled={disabled}>
      <legend className="sr-only">{multi ? "Select all that apply" : "Select one answer"}</legend>
      {multi && <p className="text-xs text-slate-500">Select all that apply.</p>}
      {question.choices.map((c) => {
        const on = selected.has(c.id);
        return (
          <label key={c.id} className={optionClass(on, disabled)}>
            <input
              type={multi ? "checkbox" : "radio"} name={name} checked={on} className="mt-0.5 h-4 w-4 accent-brand-600"
              onChange={() => {
                if (!multi) return onChange(c.id);
                const next = new Set(selected);
                if (on) next.delete(c.id); else next.add(c.id);
                onChange([...next]);
              }}
            />
            <span>{c.text}</span>
          </label>
        );
      })}
    </fieldset>
  );
}

/**
 * Typed answers save shortly after the student pauses typing, immediately on blur, and when the
 * question is left. The parent keys this component by question id, so one instance = one question.
 */
function TextAnswer({ value, disabled, onChange }: { question: PaperQuestion; value: string; disabled: boolean; onChange: (v: AnswerValue) => void }) {
  const [text, setText] = useState(value);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const lastSent = useRef(value);
  const latestText = useRef(value);
  const latestOnChange = useRef(onChange);
  latestOnChange.current = onChange;
  const id = useId();

  const send = () => {
    clearTimeout(timer.current);
    if (latestText.current !== lastSent.current) {
      lastSent.current = latestText.current;
      latestOnChange.current(latestText.current);
    }
  };
  // Leaving the question must not lose text typed in the last half second.
  useEffect(() => () => send(), []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="space-y-1.5">
      <label htmlFor={id} className="text-sm font-medium">Your answer</label>
      <input
        id={id} type="text" value={text} disabled={disabled} maxLength={500} autoComplete="off" spellCheck={false}
        className="h-11 w-full rounded-md border border-slate-300 bg-white px-3 text-sm disabled:opacity-60 dark:border-slate-600 dark:bg-navy-950"
        onChange={(e) => {
          setText(e.target.value);
          latestText.current = e.target.value;
          clearTimeout(timer.current);
          timer.current = setTimeout(send, 500);
        }}
        onBlur={send}
      />
    </div>
  );
}
