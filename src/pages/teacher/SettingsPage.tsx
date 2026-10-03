import { Moon, Sun } from "lucide-react";
import { useState, type FormEvent } from "react";
import { Field, PasswordField } from "@/components/Field";
import { NetworkCard } from "@/features/sessions/NetworkCard";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { authApi, isSessionExpired, toMessage } from "@/services/auth";
import { refreshAuth, useAuth } from "@/stores/auth";
import { useTheme } from "@/stores/theme";
import { passwordError } from "@/utils/validation";

type Notice = { kind: "ok" | "error"; text: string } | null;

function NoticeBox({ n }: { n: Notice }) {
  if (!n) return null;
  const cls = n.kind === "ok" ? "bg-green-50 text-green-800 dark:bg-green-950 dark:text-green-300" : "bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-300";
  return <p role={n.kind === "error" ? "alert" : "status"} className={`rounded-md p-3 text-sm ${cls}`}>{n.text}</p>;
}

function handleError(e: unknown): Notice {
  const text = toMessage(e);
  if (isSessionExpired(text)) refreshAuth();
  return { kind: "error", text };
}

function ProfileCard() {
  const { user, setUser } = useAuth();
  const [name, setName] = useState(user?.displayName ?? "");
  const [notice, setNotice] = useState<Notice>(null);

  async function save(ev: FormEvent) {
    ev.preventDefault();
    try {
      setUser(await authApi.updateDisplayName(name));
      setNotice({ kind: "ok", text: "Display name updated." });
    } catch (e) { setNotice(handleError(e)); }
  }

  return (
    <Card className="p-6">
      <h2 className="mb-4 font-medium">Profile</h2>
      <form onSubmit={save} className="max-w-sm space-y-4">
        <Field label="Username" value={user?.username ?? ""} disabled readOnly />
        <Field label="Display name" value={name} onChange={(e) => setName(e.target.value)} />
        <NoticeBox n={notice} />
        <Button type="submit" disabled={!name.trim() || name.trim() === user?.displayName}>Save</Button>
      </form>
    </Card>
  );
}

function PasswordCard() {
  const user = useAuth((s) => s.user);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [notice, setNotice] = useState<Notice>(null);
  const [busy, setBusy] = useState(false);

  const newError = next ? passwordError(next, user?.username ?? "") : null;
  const confirmError = confirm && confirm !== next ? "Passwords do not match." : null;
  const canSubmit = current && next && !newError && next === confirm && !busy;

  async function save(ev: FormEvent) {
    ev.preventDefault();
    setBusy(true);
    try {
      await authApi.changePassword(current, next);
      setCurrent(""); setNext(""); setConfirm("");
      setNotice({ kind: "ok", text: "Password changed." });
    } catch (e) { setNotice(handleError(e)); }
    setBusy(false);
  }

  return (
    <Card className="p-6">
      <h2 className="mb-4 font-medium">Change password</h2>
      <form onSubmit={save} className="max-w-sm space-y-4">
        <PasswordField label="Current password" value={current} onChange={(e) => setCurrent(e.target.value)} autoComplete="current-password" />
        <PasswordField label="New password" value={next} onChange={(e) => setNext(e.target.value)} autoComplete="new-password" error={newError} hint="At least 8 characters." />
        <PasswordField label="Confirm new password" value={confirm} onChange={(e) => setConfirm(e.target.value)} autoComplete="new-password" error={confirmError} />
        <NoticeBox n={notice} />
        <Button type="submit" disabled={!canSubmit}>{busy ? "Saving…" : "Change password"}</Button>
      </form>
    </Card>
  );
}

function AppearanceCard() {
  const { theme, toggle } = useTheme();
  return (
    <Card className="p-6">
      <h2 className="mb-4 font-medium">Appearance</h2>
      <div className="flex gap-2" role="group" aria-label="Theme">
        <Button variant={theme === "light" ? "primary" : "outline"} onClick={() => theme !== "light" && toggle()} aria-pressed={theme === "light"}><Sun className="h-4 w-4" /> Light</Button>
        <Button variant={theme === "dark" ? "primary" : "outline"} onClick={() => theme !== "dark" && toggle()} aria-pressed={theme === "dark"}><Moon className="h-4 w-4" /> Dark</Button>
      </div>
    </Card>
  );
}

export function SettingsPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold">Settings</h1>
      <ProfileCard />
      <PasswordCard />
      <NetworkCard />
      <AppearanceCard />
    </div>
  );
}
