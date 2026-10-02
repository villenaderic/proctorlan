import { useState, type FormEvent } from "react";
import { AuthShell } from "@/components/AuthShell";
import { Field, PasswordField } from "@/components/Field";
import { Button } from "@/components/ui/button";
import { toMessage } from "@/services/auth";
import { useAuth } from "@/stores/auth";
import { passwordError, usernameError } from "@/utils/validation";

export function SetupPage() {
  const setupAdmin = useAuth((s) => s.setupAdmin);
  const [displayName, setDisplayName] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [touched, setTouched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);

  const errors = {
    displayName: displayName.trim() ? null : "Enter your name.",
    username: usernameError(username),
    password: passwordError(password, username),
    confirm: confirm === password ? null : "Passwords do not match.",
  };
  const valid = Object.values(errors).every((e) => e === null);
  const show = (e: string | null) => (touched ? e : null);

  async function submit(ev: FormEvent) {
    ev.preventDefault();
    setTouched(true);
    if (!valid) return;
    setBusy(true);
    setServerError(null);
    try {
      await setupAdmin(username.trim(), password, displayName.trim());
    } catch (e) {
      setServerError(toMessage(e));
      setBusy(false);
    }
  }

  return (
    <AuthShell title="Create Administrator Account" subtitle="This account stays on this computer. Choose a password you will remember — there is no online recovery.">
      <form onSubmit={submit} className="space-y-4" noValidate>
        <Field label="Your name" value={displayName} onChange={(e) => setDisplayName(e.target.value)} autoComplete="name" error={show(errors.displayName)} autoFocus />
        <Field label="Username" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" error={show(errors.username)} />
        <PasswordField label="Password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="new-password" error={show(errors.password)} hint="At least 8 characters." />
        <PasswordField label="Confirm password" value={confirm} onChange={(e) => setConfirm(e.target.value)} autoComplete="new-password" error={show(errors.confirm)} />
        {serverError && <p role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{serverError}</p>}
        <Button type="submit" className="w-full" disabled={busy}>{busy ? "Creating…" : "Create account"}</Button>
      </form>
    </AuthShell>
  );
}
