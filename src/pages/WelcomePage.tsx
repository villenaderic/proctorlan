import { GraduationCap, UserRound, WifiOff } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { APP } from "@/config";
import { Logo } from "@/components/Logo";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ThemeToggle } from "@/components/ThemeToggle";
import { RuntimeBadge } from "@/components/RuntimeBadge";

export function WelcomePage() {
  const nav = useNavigate();
  return (
    <main className="relative flex min-h-full flex-col items-center justify-center gap-8 p-8">
      <div className="absolute right-4 top-4"><ThemeToggle /></div>
      <div className="flex flex-col items-center gap-3 text-center">
        <Logo />
        <h1 className="text-3xl font-semibold tracking-tight">{APP.name}</h1>
        <p className="flex items-center gap-2 text-slate-600 dark:text-slate-300">
          <WifiOff className="h-4 w-4" aria-hidden /> {APP.tagline}
        </p>
      </div>
      <div className="grid w-full max-w-2xl gap-4 sm:grid-cols-2">
        <Card className="flex flex-col items-center gap-3 p-6 text-center">
          <GraduationCap className="h-8 w-8 text-brand-600" aria-hidden />
          <h2 className="text-lg font-medium">I'm a Teacher</h2>
          <p className="text-sm text-slate-600 dark:text-slate-300">Create and manage exams, monitor students, view results.</p>
          <Button className="mt-auto w-full" onClick={() => nav("/teacher")}>Continue</Button>
        </Card>
        <Card className="flex flex-col items-center gap-3 p-6 text-center">
          <UserRound className="h-8 w-8 text-brand-600" aria-hidden />
          <h2 className="text-lg font-medium">I'm a Student</h2>
          <p className="text-sm text-slate-600 dark:text-slate-300">Join an exam using the session code from your teacher.</p>
          <Button variant="outline" className="mt-auto w-full" onClick={() => nav("/student")}>Join Exam</Button>
        </Card>
      </div>
      <RuntimeBadge />
    </main>
  );
}
