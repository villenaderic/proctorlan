import { Pause, Play, Square, UserMinus, Users, Wifi, WifiOff } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/dialog";
import { isSessionExpired, toMessage } from "@/services/auth";
import { sessionsApi } from "@/services/sessions";
import { refreshAuth } from "@/stores/auth";
import type { RosterRow, SessionAction, SessionSnapshot } from "@/types/session";
import { formatClock } from "@/utils/format";
import { STATUS_LABEL, allowedActions, displayRemaining } from "@/utils/sessionClock";

const POLL_MS = 2000;

export function SessionLivePage() {
  const { id = "" } = useParams();
  const [snap, setSnap] = useState<SessionSnapshot | null>(null);
  const [roster, setRoster] = useState<RosterRow[]>([]);
  const [toRemove, setToRemove] = useState<RosterRow | null>(null);
  const [fetchedAt, setFetchedAt] = useState(() => Date.now());
  const [now, setNow] = useState(() => Date.now());
  const [error, setError] = useState<string | null>(null);
  const [confirmEnd, setConfirmEnd] = useState(false);
  const [busy, setBusy] = useState(false);
  const alive = useRef(true);

  const fail = useCallback((e: unknown) => {
    const text = toMessage(e);
    if (isSessionExpired(text)) refreshAuth();
    setError(text);
  }, []);

  const apply = useCallback((s: SessionSnapshot) => {
    if (!alive.current) return;
    setSnap(s);
    setFetchedAt(Date.now());
    setError(null);
  }, []);

  useEffect(() => {
    alive.current = true;
    const load = () => {
      sessionsApi.snapshot(id).then(apply).catch(fail);
      sessionsApi.roster(id).then((r) => alive.current && setRoster(r)).catch(fail);
    };
    load();
    const poll = setInterval(load, POLL_MS);
    const tick = setInterval(() => setNow(Date.now()), 1000);
    return () => { alive.current = false; clearInterval(poll); clearInterval(tick); };
  }, [id, apply, fail]);

  async function act(action: SessionAction) {
    setBusy(true);
    try { apply(await sessionsApi.act(id, action)); } catch (e) { fail(e); }
    setBusy(false);
  }

  if (!snap) {
    return error ? <p role="alert" className="text-red-700 dark:text-red-300">{error}</p> : <p>Loading session…</p>;
  }

  const can = allowedActions(snap.status);
  const remaining = displayRemaining(snap, fetchedAt, now);
  const joinAddress = `${snap.hostIp}:${snap.hostPort}`;
  const ended = snap.status === "ENDED";

  return (
    <div className="space-y-4">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <Link to="/teacher/sessions" className="text-sm text-brand-600 underline">← All sessions</Link>
          <h1 className="text-2xl font-semibold">{snap.examTitle}</h1>
        </div>
        <Badge tone={snap.status === "RUNNING" ? "green" : snap.status === "PAUSED" ? "amber" : ended ? "gray" : "blue"}>{STATUS_LABEL[snap.status]}</Badge>
      </header>

      {error && <p role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</p>}

      {!ended && (
        <Card className="grid gap-6 p-6 sm:grid-cols-2">
          <div>
            <p className="text-xs uppercase text-slate-500">Session code</p>
            <p className="font-mono text-5xl font-bold tracking-[0.3em]" aria-label={`Session code ${snap.sessionCode.split("").join(" ")}`}>{snap.sessionCode}</p>
          </div>
          <div>
            <p className="text-xs uppercase text-slate-500">Students enter this address</p>
            <p className="font-mono text-2xl font-semibold">{joinAddress}</p>
            <p className="mt-1 text-xs text-slate-500">Students must be on the same network as this computer.</p>
          </div>
        </Card>
      )}

      <div className="grid gap-4 sm:grid-cols-4">
        <Stat label="Online now" value={snap.online} icon />
        <Stat label="Joined" value={snap.joined} />
        <Stat label="Submitted" value={snap.submitted} />
        <Card className="p-4">
          <p className="text-xs uppercase text-slate-500">Time remaining</p>
          <p className="font-mono text-3xl font-semibold" aria-live="off">
            {remaining === null ? formatClock(0) : formatClock(remaining)}
          </p>
          <p className="text-xs text-slate-500">{remaining === null ? "Starts when you press Start." : snap.status === "PAUSED" ? "Paused" : "Server clock"}</p>
        </Card>
      </div>

      <div className="flex flex-wrap gap-2">
        {can.start && <Button disabled={busy} onClick={() => act("start")}><Play className="h-4 w-4" aria-hidden /> Start exam</Button>}
        {can.pause && <Button variant="outline" disabled={busy} onClick={() => act("pause")}><Pause className="h-4 w-4" aria-hidden /> Pause</Button>}
        {can.resume && <Button disabled={busy} onClick={() => act("resume")}><Play className="h-4 w-4" aria-hidden /> Resume</Button>}
        {can.end && <Button variant="outline" disabled={busy} onClick={() => setConfirmEnd(true)}><Square className="h-4 w-4" aria-hidden /> End session</Button>}
      </div>

      <Card className="overflow-hidden">
        <h2 className="border-b border-slate-200 px-4 py-3 font-medium dark:border-slate-700">Students ({roster.length})</h2>
        {roster.length === 0 ? (
          <p className="p-4 text-sm text-slate-600 dark:text-slate-300">No one has joined yet. Share the code and address above.</p>
        ) : (
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-left text-xs uppercase text-slate-500 dark:bg-navy-800 dark:text-slate-300">
              <tr><th className="px-4 py-2">Name</th><th className="px-4 py-2">Student ID</th><th className="px-4 py-2">Connection</th><th className="px-4 py-2">Progress</th><th className="px-4 py-2" /></tr>
            </thead>
            <tbody>
              {roster.map((r) => (
                <tr key={r.attemptId} className="border-t border-slate-200 dark:border-slate-700">
                  <td className="px-4 py-2 font-medium">{r.name}</td>
                  <td className="px-4 py-2 font-mono">{r.studentNumber}</td>
                  <td className="px-4 py-2">
                    <Badge tone={r.online ? "green" : "amber"}>{r.online ? <Wifi className="h-3 w-3" aria-hidden /> : <WifiOff className="h-3 w-3" aria-hidden />}{r.online ? "Online" : "Disconnected"}</Badge>
                  </td>
                  <td className="px-4 py-2">{r.status === "JOINED" ? "Joined" : r.status === "IN_PROGRESS" ? "In progress" : "Submitted"}</td>
                  <td className="px-4 py-2 text-right">
                    {r.status === "JOINED" && !ended && (
                      <Button size="sm" variant="outline" onClick={() => setToRemove(r)} aria-label={`Remove ${r.name}`}><UserMinus className="h-3.5 w-3.5" aria-hidden /> Remove</Button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      <p className="text-sm text-slate-600 dark:text-slate-300">
        {snap.questionCount} questions · passing score {snap.passingScore}%. Student join, answers and live proctoring arrive in the next phases.
      </p>

      <ConfirmDialog open={!!toRemove} danger title={`Remove ${toRemove?.name ?? "student"}?`} confirmLabel="Remove"
        onCancel={() => setToRemove(null)}
        onConfirm={() => {
          const r = toRemove;
          setToRemove(null);
          if (r) sessionsApi.removeStudent(r.attemptId).then(() => sessionsApi.roster(id).then(setRoster)).catch(fail);
        }}>
        They are disconnected from this session and can join again with their student ID. Only students who have not started the exam can be removed.
      </ConfirmDialog>

      <ConfirmDialog open={confirmEnd} danger title="End this session?" confirmLabel="End session"
        onCancel={() => setConfirmEnd(false)} onConfirm={() => { setConfirmEnd(false); act("end"); }}>
        Students will be told the exam is over and the code stops working. This cannot be undone.
      </ConfirmDialog>
    </div>
  );
}

function Stat({ label, value, icon }: { label: string; value: number; icon?: boolean }) {
  return (
    <Card className="p-4">
      <p className="flex items-center gap-1 text-xs uppercase text-slate-500">{icon && <Users className="h-3.5 w-3.5" aria-hidden />}{label}</p>
      <p className="text-3xl font-semibold">{value}</p>
    </Card>
  );
}
