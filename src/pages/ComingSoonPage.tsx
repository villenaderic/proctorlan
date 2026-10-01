import { ArrowLeft } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";

/** Honest stand-in for areas whose phase has not been built yet. Removed as phases land. */
export function ComingSoonPage({ title, phase }: { title: string; phase: string }) {
  const nav = useNavigate();
  return (
    <main className="flex min-h-full flex-col items-center justify-center gap-4 p-8 text-center">
      <h1 className="text-2xl font-semibold">{title}</h1>
      <p className="text-slate-600 dark:text-slate-300">Not built yet — scheduled for {phase}.</p>
      <Button variant="outline" onClick={() => nav("/")}><ArrowLeft className="h-4 w-4" /> Back</Button>
    </main>
  );
}
