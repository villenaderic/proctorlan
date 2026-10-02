import { BarChart3, FileText, HardDriveDownload, LayoutDashboard, LogOut, Radio, Settings, Users } from "lucide-react";
import { NavLink, Outlet } from "react-router-dom";
import { APP } from "@/config";
import { Logo } from "@/components/Logo";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/stores/auth";
import { cn } from "@/utils/cn";

const NAV = [
  { to: "/teacher", label: "Dashboard", icon: LayoutDashboard, end: true },
  { to: "/teacher/exams", label: "Exams", icon: FileText },
  { to: "/teacher/sessions", label: "Sessions", icon: Radio },
  { to: "/teacher/students", label: "Students", icon: Users },
  { to: "/teacher/results", label: "Results", icon: BarChart3 },
  { to: "/teacher/backups", label: "Backups", icon: HardDriveDownload },
  { to: "/teacher/settings", label: "Settings", icon: Settings },
];

export function TeacherLayout() {
  const { user, logout } = useAuth();
  return (
    <div className="flex h-full">
      <aside className="flex w-56 shrink-0 flex-col bg-navy-900 text-slate-200">
        <div className="flex items-center gap-2 px-4 py-4">
          <Logo size={32} />
          <span className="font-semibold text-white">{APP.name}</span>
        </div>
        <nav aria-label="Main" className="flex-1 space-y-1 px-2">
          {NAV.map(({ to, label, icon: Icon, end }) => (
            <NavLink key={to} to={to} end={end}
              className={({ isActive }) => cn("flex items-center gap-3 rounded-md px-3 py-2 text-sm", isActive ? "bg-brand-600 text-white" : "hover:bg-navy-800")}>
              <Icon className="h-4 w-4" aria-hidden /> {label}
            </NavLink>
          ))}
        </nav>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center justify-end gap-3 border-b border-slate-200 bg-white px-6 py-2 dark:border-slate-700 dark:bg-navy-900">
          <ThemeToggle />
          <span className="text-sm">{user?.displayName}</span>
          <Button variant="outline" size="sm" onClick={() => logout()}><LogOut className="h-4 w-4" aria-hidden /> Log out</Button>
        </header>
        <main className="flex-1 overflow-auto p-6"><Outlet /></main>
      </div>
    </div>
  );
}
