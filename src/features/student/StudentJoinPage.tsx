import { Loader2, LogIn, WifiOff } from "lucide-react";
import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { Field } from "@/components/Field";
import { AuthShell } from "@/components/AuthShell";
import { Button } from "@/components/ui/button";
import { NETWORK_DEFAULTS } from "@/config";
import { useStudent } from "@/stores/student";
import { isAddressError, normalizeCode, parseHostAddress } from "./address";
import { loadSaved } from "./saved";
import { StudentRoomPage } from "./StudentRoomPage";

export function StudentPage() {
  const phase = useStudent((s) => s.phase);
  if (phase === "in_session" || phase === "ended") return <StudentRoomPage />;
  return <StudentJoinPage />;
}

function StudentJoinPage() {
  const nav = useNavigate();
  const { phase, connection, error, join, leave } = useStudent();
  const saved = loadSaved();
  const [address, setAddress] = useState(saved ? `${saved.host}:${saved.port}` : "");
  const [code, setCode] = useState(saved?.sessionCode ?? "");
  const [name, setName] = useState(saved?.studentName ?? "");
  const [studentId, setStudentId] = useState(saved?.studentId ?? "");
  const [touched, setTouched] = useState(false);

  const parsed = parseHostAddress(address);
  const addressError = isAddressError(parsed) ? parsed.error : null;
  const codeClean = normalizeCode(code);
  const codeError = codeClean.length !== NETWORK_DEFAULTS.sessionCodeLength ? `The code has ${NETWORK_DEFAULTS.sessionCodeLength} characters.` : null;
  const nameError = name.trim() ? null : "Enter your full name.";
  const idError = studentId.trim() ? null : "Enter your student ID.";
  const valid = !addressError && !codeError && !nameError && !idError;
  const joining = phase === "joining";

  function submit(ev: FormEvent, token: string | null) {
    ev.preventDefault();
    setTouched(true);
    if (!valid || isAddressError(parsed)) return;
    join({ host: parsed.host, port: parsed.port, sessionCode: codeClean, studentName: name.trim(), studentId: studentId.trim(), token });
  }

  const canRejoin = saved?.token && saved.sessionCode === codeClean && saved.studentId === studentId.trim();

  return (
    <AuthShell title="Join an exam" subtitle="Enter the details your teacher shows on screen.">
      <form onSubmit={(e) => submit(e, canRejoin ? saved!.token : null)} className="space-y-4" noValidate>
        {saved?.token && !joining && (
          <p role="status" className="rounded-md bg-brand-50 p-3 text-sm text-brand-700 dark:bg-navy-800 dark:text-blue-300">
            You were in “{saved.examTitle ?? "an exam session"}”. Press <strong>Rejoin</strong> to continue where you left off.
          </p>
        )}
        <Field label="Teacher's address" placeholder="192.168.1.20:38123" value={address} onChange={(e) => setAddress(e.target.value)} disabled={joining}
          autoComplete="off" spellCheck={false} error={touched ? addressError : null} hint="Shown on the teacher's screen as IP:port." />
        <Field label="Session code" placeholder="ABC23" value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} disabled={joining}
          autoComplete="off" spellCheck={false} maxLength={12} className="font-mono tracking-widest uppercase" error={touched ? codeError : null} />
        <Field label="Full name" value={name} onChange={(e) => setName(e.target.value)} disabled={joining} autoComplete="off" maxLength={64} error={touched ? nameError : null} />
        <Field label="Student ID" value={studentId} onChange={(e) => setStudentId(e.target.value)} disabled={joining} autoComplete="off" maxLength={32} error={touched ? idError : null} />

        {error && <p role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>}

        {joining && connection === "reconnecting" && (
          <p role="status" className="flex items-start gap-2 rounded-md bg-amber-50 p-3 text-sm text-amber-800 dark:bg-amber-950 dark:text-amber-300">
            <WifiOff className="mt-0.5 h-4 w-4 shrink-0" aria-hidden />
            Can't reach the teacher's computer yet. Check the address, that you are on the same Wi-Fi or network, and that the session is open. Still trying…
          </p>
        )}

        <div className="flex gap-2">
          <Button type="submit" className="flex-1" disabled={joining}>
            {joining ? <><Loader2 className="h-4 w-4 animate-spin" aria-hidden /> Connecting…</> : <><LogIn className="h-4 w-4" aria-hidden /> {canRejoin ? "Rejoin" : "Join exam"}</>}
          </Button>
          {joining ? <Button type="button" variant="outline" onClick={leave}>Cancel</Button> : <Button type="button" variant="outline" onClick={() => nav("/")}>Back</Button>}
        </div>
      </form>
    </AuthShell>
  );
}
