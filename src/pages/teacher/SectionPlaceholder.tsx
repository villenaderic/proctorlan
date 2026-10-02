/** Honest stand-in inside the teacher shell for sections whose phase is not built yet. */
export function SectionPlaceholder({ title, phase }: { title: string; phase: string }) {
  return (
    <div className="space-y-2">
      <h1 className="text-2xl font-semibold">{title}</h1>
      <p className="text-slate-600 dark:text-slate-300">Not built yet — scheduled for {phase}.</p>
    </div>
  );
}
