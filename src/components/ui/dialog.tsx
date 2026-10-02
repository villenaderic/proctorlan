import { useEffect, useRef, type ReactNode } from "react";
import { Button } from "@/components/ui/button";

interface Props {
  open: boolean;
  title: string;
  children?: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Modal built on the native <dialog>: focus trapping, Esc to close and backdrop come for free. */
export function ConfirmDialog({ open, title, children, confirmLabel = "Confirm", cancelLabel = "Cancel", danger, onConfirm, onCancel }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);

  return (
    <dialog ref={ref} aria-labelledby="dlg-title" onCancel={(e) => { e.preventDefault(); onCancel(); }}
      className="m-auto w-full max-w-md rounded-xl border border-slate-200 bg-white p-6 text-slate-900 shadow-xl backdrop:bg-black/50 dark:border-slate-700 dark:bg-navy-900 dark:text-slate-100">
      {open && (
        <div className="space-y-4">
          <h2 id="dlg-title" className="text-lg font-semibold">{title}</h2>
          <div className="text-sm text-slate-600 dark:text-slate-300">{children}</div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={onCancel}>{cancelLabel}</Button>
            <Button className={danger ? "bg-red-600 hover:bg-red-700" : ""} onClick={onConfirm}>{confirmLabel}</Button>
          </div>
        </div>
      )}
    </dialog>
  );
}
