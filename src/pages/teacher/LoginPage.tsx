import { useState, type FormEvent } from "react";
import { AuthShell } from "@/components/AuthShell";
import { Field, PasswordField } from "@/components/Field";
import { Button } from "@/components/ui/button";
import { toMessage } from "@/services/auth";
import { useAuth } from "@/stores/auth";

export function LoginPage() {
  const login = useAuth((s) => s.login);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(ev: FormEvent) {
    ev.preventDefault();
    if (!username.trim() || !password) return setError("Enter your username and password.");
    setBusy(true);
    setError(null);
    try {
      await login(username.trim(), password);
    } catch (e) {
      setError(toMessage(e));
      setPassword("");
      setBusy(false);
    }
  }

  return (
    <AuthShell title="Teacher Login" subtitle="Sign in to your account.">
      <form onSubmit={submit} className="space-y-4" noValidate>
        <Field label="Username" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" autoFocus />
        <PasswordField label="Password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" />
        {error && <p role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>}
        <Button type="submit" className="w-full" disabled={busy}>{busy ? "Signing in…" : "Login"}</Button>
      </form>
    </AuthShell>
  );
}
