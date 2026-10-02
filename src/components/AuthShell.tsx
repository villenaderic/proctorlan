import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { APP } from "@/config";
import { Logo } from "@/components/Logo";
import { ThemeToggle } from "@/components/ThemeToggle";

/** Split layout used by setup and login: brand panel + form card (matches the design mockup). */
export function AuthShell({ title, subtitle, children }: { title: string; subtitle: string; children: ReactNode }) {
  const nav = useNavigate();
  return (
    <main className="grid min-h-full md:grid-cols-[2fr_3fr]">
      <section className="hidden flex-col items-center justify-center gap-4 bg-navy-900 p-8 text-center text-white md:flex">
        <Logo size={72} />
        <h1 className="text-2xl font-semibold">{APP.name}</h1>
        <p className="max-w-xs text-sm text-slate-300">{APP.tagline}</p>
      </section>
      <section className="relative flex items-center justify-center p-6">
        <div className="absolute left-4 top-4">
          <button onClick={() => nav("/")} className="flex items-center gap-1 rounded text-sm text-slate-600 hover:underline dark:text-slate-300">
            <ArrowLeft className="h-4 w-4" aria-hidden /> Back
          </button>
        </div>
        <div className="absolute right-4 top-4"><ThemeToggle /></div>
        <div className="w-full max-w-sm space-y-6">
          <header>
            <h2 className="text-2xl font-semibold tracking-tight">{title}</h2>
            <p className="mt-1 text-sm text-slate-600 dark:text-slate-300">{subtitle}</p>
          </header>
          {children}
        </div>
      </section>
    </main>
  );
}
