import { Eye, EyeOff } from "lucide-react";
import { useId, useState, type InputHTMLAttributes, type ReactNode } from "react";
import { Input } from "@/components/ui/input";

interface FieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "id"> {
  label: string;
  hint?: ReactNode;
  error?: string | null;
}

/** Labelled input with accessible error/hint wiring. */
export function Field({ label, hint, error, ...input }: FieldProps) {
  const id = useId();
  const describedBy = error ? `${id}-err` : hint ? `${id}-hint` : undefined;
  return (
    <div className="space-y-1.5">
      <label htmlFor={id} className="text-sm font-medium">{label}</label>
      <Input id={id} aria-invalid={!!error} aria-describedby={describedBy} {...input} />
      {error ? <p id={`${id}-err`} role="alert" className="text-xs text-red-600 dark:text-red-400">{error}</p>
        : hint ? <p id={`${id}-hint`} className="text-xs text-slate-500 dark:text-slate-400">{hint}</p> : null}
    </div>
  );
}

export function PasswordField({ label, hint, error, ...input }: FieldProps) {
  const id = useId();
  const [shown, setShown] = useState(false);
  const describedBy = error ? `${id}-err` : hint ? `${id}-hint` : undefined;
  return (
    <div className="space-y-1.5">
      <label htmlFor={id} className="text-sm font-medium">{label}</label>
      <div className="relative">
        <Input id={id} type={shown ? "text" : "password"} className="pr-10" aria-invalid={!!error} aria-describedby={describedBy} {...input} />
        <button type="button" onClick={() => setShown((s) => !s)} aria-label={shown ? "Hide password" : "Show password"}
          className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1 text-slate-500 hover:text-slate-900 dark:hover:text-white">
          {shown ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
        </button>
      </div>
      {error ? <p id={`${id}-err`} role="alert" className="text-xs text-red-600 dark:text-red-400">{error}</p>
        : hint ? <p id={`${id}-hint`} className="text-xs text-slate-500 dark:text-slate-400">{hint}</p> : null}
    </div>
  );
}
