import { create } from "zustand";
import { StudentClient, type ClientFailure, type ConnectionState, type Envelope, type JoinParams, type JoinedInfo } from "@/features/student/client";
import { clearSaved, saveJoin } from "@/features/student/saved";
import type { SessionStatus } from "@/types/session";

export type StudentPhase = "form" | "joining" | "in_session" | "ended";

interface StudentState {
  phase: StudentPhase;
  connection: ConnectionState;
  info: JoinedInfo | null;
  status: SessionStatus;
  remainingSeconds: number | null;
  /** performance.now()-style local stamp of when remainingSeconds was received. */
  receivedAt: number;
  error: string | null;
  join(params: JoinParams): void;
  /** Leaves the screen but keeps the saved token, so the student can rejoin the same attempt. */
  leave(): void;
}

let client: StudentClient | null = null;

const applyTimer = (p: Record<string, any>) => ({
  status: (p.status as SessionStatus) ?? "WAITING",
  remainingSeconds: typeof p.remainingSeconds === "number" ? p.remainingSeconds : null,
  receivedAt: Date.now(),
});

export const useStudent = create<StudentState>((set) => ({
  phase: "form",
  connection: "stopped",
  info: null,
  status: "WAITING",
  remainingSeconds: null,
  receivedAt: 0,
  error: null,

  join(params) {
    client?.stop();
    set({ phase: "joining", error: null, connection: "connecting", info: null });
    client = new StudentClient(params, {
      onState: (connection) => set({ connection }),
      onJoined: (info: JoinedInfo) => {
        saveJoin({ host: params.host, port: params.port, sessionCode: params.sessionCode, studentName: info.studentName, studentId: info.studentId, token: info.token, examTitle: info.exam.title });
        const ended = info.status === "ENDED";
        set({ phase: ended ? "ended" : "in_session", info, ...applyTimer(info as unknown as Record<string, any>) });
      },
      onMessage: (env: Envelope) => {
        if (env.type === "session_ended") { clearSaved(); set({ phase: "ended", ...applyTimer(env.payload) }); return; }
        if (["session_started", "session_paused", "session_resumed", "timer_sync"].includes(env.type)) set(applyTimer(env.payload));
      },
      onFatal: (f: ClientFailure) => {
        // These answers mean the saved place is useless; forget it so the form does not offer it again.
        if (["session_not_found", "session_closed", "removed", "already_joined"].includes(f.code)) clearSaved();
        client = null;
        set({ phase: "form", error: f.message, connection: "stopped", info: null });
      },
    });
    client.start();
  },

  leave() {
    client?.stop();
    client = null;
    set({ phase: "form", connection: "stopped", info: null, error: null });
  },
}));

/** Test hook: forget module state between tests. */
export const __resetStudentForTests = () => { client?.stop(); client = null; useStudent.setState({ phase: "form", connection: "stopped", info: null, error: null }); };
