import { create } from "zustand";
import { StudentClient, type ClientFailure, type ConnectionState, type Envelope, type JoinParams, type JoinedInfo } from "@/features/student/client";
import { createSequencer, mergeAnswers } from "@/features/student/paper";
import { clearSaved, saveJoin } from "@/features/student/saved";
import type { AnswerValue, Paper, ResultSummary } from "@/types/paper";
import type { SessionStatus } from "@/types/session";

export type StudentPhase = "form" | "joining" | "in_session" | "ended";
export type SaveState = "saved" | "saving" | "offline";

interface Pending { answer: AnswerValue; seq: number }

interface StudentState {
  phase: StudentPhase;
  connection: ConnectionState;
  info: JoinedInfo | null;
  status: SessionStatus;
  remainingSeconds: number | null;
  /** Date.now() when remainingSeconds was received. */
  receivedAt: number;
  error: string | null;

  paper: Paper | null;
  answers: Record<string, AnswerValue>;
  /** Answers not yet acknowledged by the server. Resent after any reconnect. */
  pending: Record<string, Pending>;
  current: number;
  timeUp: boolean;
  submitting: boolean;
  submitted: boolean;
  autoSubmitted: boolean;
  result: ResultSummary | null;
  examError: string | null;

  join(params: JoinParams): void;
  /** Leaves the screen but keeps the saved token, so the student can rejoin the same attempt. */
  leave(): void;
  openExam(): Promise<void>;
  setAnswer(questionId: string, answer: AnswerValue): void;
  goTo(index: number): void;
  submit(): Promise<boolean>;
  saveState(): SaveState;
}

let client: StudentClient | null = null;
const nextSeq = createSequencer();

const blankExam = {
  paper: null, answers: {}, pending: {}, current: 0, timeUp: false, submitting: false,
  submitted: false, autoSubmitted: false, result: null, examError: null,
} as const;

const applyTimer = (p: Record<string, any>) => ({
  status: (p.status as SessionStatus) ?? "WAITING",
  remainingSeconds: typeof p.remainingSeconds === "number" ? p.remainingSeconds : null,
  receivedAt: Date.now(),
});

export const useStudent = create<StudentState>((set, get) => {
  /** Sends one pending answer. Keeps it queued for retry unless the server gave a final verdict. */
  async function sendAnswer(questionId: string): Promise<void> {
    const p = get().pending[questionId];
    if (!p || !client) return;
    const drop = () => {
      const cur = get().pending[questionId];
      if (cur && cur.seq === p.seq) {
        const { [questionId]: _gone, ...rest } = get().pending;
        set({ pending: rest });
      }
    };
    try {
      const r = await client.request("answer", { questionId, answer: p.answer, clientSeq: p.seq });
      if (r.type === "answer_ack") return drop();
      switch (r.payload.code) {
        case "exam_not_running": return; // paused: keep it, resend after resume
        case "time_up": set({ timeUp: true, examError: "Time is up. Your last change was not saved." }); return drop();
        case "already_submitted": set({ submitted: true }); return drop();
        default: set({ examError: String(r.payload.message ?? "An answer could not be saved.") }); return drop();
      }
    } catch { /* offline or timed out: stays pending, resent when we are back */ }
  }

  async function flushPending(): Promise<boolean> {
    await Promise.all(Object.keys(get().pending).map(sendAnswer));
    return Object.keys(get().pending).length === 0;
  }

  return {
    phase: "form", connection: "stopped", info: null, status: "WAITING", remainingSeconds: null, receivedAt: 0, error: null,
    ...blankExam,

    join(params) {
      client?.stop();
      set({ phase: "joining", error: null, connection: "connecting", info: null, ...blankExam });
      client = new StudentClient(params, {
        onState: (connection) => set({ connection }),
        onJoined: (info: JoinedInfo) => {
          saveJoin({ host: params.host, port: params.port, sessionCode: params.sessionCode, studentName: info.studentName, studentId: info.studentId, token: info.token, examTitle: info.exam.title });
          set({ phase: info.status === "ENDED" ? "ended" : "in_session", info, ...applyTimer(info as unknown as Record<string, any>) });
          // Back online: resend anything unacknowledged and (re)load the paper if the exam is running.
          void flushPending();
          if (info.status === "RUNNING" && !get().paper) void get().openExam();
        },
        onMessage: (env: Envelope) => {
          const p = env.payload;
          switch (env.type) {
            case "session_ended":
              clearSaved();
              set({ phase: "ended", ...applyTimer(p) });
              return;
            case "session_started":
            case "session_resumed":
              set(applyTimer(p));
              if (!get().paper) void get().openExam();
              else void flushPending();
              return;
            case "session_paused":
            case "timer_sync":
              set(applyTimer(p));
              return;
            case "time_up":
              set({ timeUp: true, remainingSeconds: 0, receivedAt: Date.now() });
              return;
            case "submitted":
              clearSaved();
              set({ submitted: true, submitting: false, autoSubmitted: !!p.auto, result: (p.result as ResultSummary | null) ?? null, pending: {} });
              return;
          }
        },
        onFatal: (f: ClientFailure) => {
          // These answers mean the saved place is useless; forget it so the form does not offer it again.
          if (["session_not_found", "session_closed", "removed", "already_joined"].includes(f.code)) clearSaved();
          client = null;
          set({ phase: "form", error: f.message, connection: "stopped", info: null, ...blankExam });
        },
      });
      client.start();
    },

    leave() {
      client?.stop();
      client = null;
      set({ phase: "form", connection: "stopped", info: null, error: null, ...blankExam });
    },

    async openExam() {
      if (!client || get().paper) return;
      try {
        const r = await client.request("start_exam");
        if (r.type === "exam_paper") {
          const paper = r.payload.paper as Paper;
          set({ paper, answers: mergeAnswers(paper.answers, get().pending), examError: null, ...applyTimer(r.payload) });
          void flushPending();
        } else {
          switch (r.payload.code) {
            case "already_submitted": set({ submitted: true }); break;
            case "time_up": set({ timeUp: true }); break;
            case "exam_not_running": break;
            default: set({ examError: String(r.payload.message ?? "The exam could not be opened.") });
          }
        }
      } catch {
        set({ examError: "Could not load the exam. Reconnecting…" });
      }
    },

    setAnswer(questionId, answer) {
      const s = get();
      if (!s.paper || s.submitted || s.timeUp || s.status !== "RUNNING") return;
      const seq = nextSeq();
      set({ answers: { ...s.answers, [questionId]: answer }, pending: { ...s.pending, [questionId]: { answer, seq } }, examError: null });
      void sendAnswer(questionId);
    },

    goTo(index) {
      const s = get();
      if (!s.paper) return;
      const max = s.paper.questions.length - 1;
      const target = Math.max(0, Math.min(max, index));
      // With review off, students can only move forward.
      if (!s.paper.allowReview && target < s.current) return;
      set({ current: target });
    },

    async submit() {
      if (!client || get().submitting || get().submitted) return get().submitted;
      set({ submitting: true, examError: null });
      const saved = await flushPending();
      if (!saved) {
        set({ submitting: false, examError: "Your latest answers have not reached the teacher's computer yet. Check your connection and try again." });
        return false;
      }
      try {
        const r = await client.request("submit");
        if (r.type === "submitted") {
          clearSaved();
          set({ submitted: true, submitting: false, autoSubmitted: false, result: (r.payload.result as ResultSummary | null) ?? null });
          return true;
        }
        if (r.payload.code === "already_submitted") { set({ submitted: true, submitting: false }); return true; }
        set({ submitting: false, examError: String(r.payload.message ?? "Submit failed.") });
      } catch {
        set({ submitting: false, examError: "Could not reach the teacher's computer. Your answers are safe; try Submit again." });
      }
      return false;
    },

    saveState() {
      const s = get();
      if (Object.keys(s.pending).length === 0) return "saved";
      return s.connection === "online" ? "saving" : "offline";
    },
  };
});

/** Test hook: forget module state between tests. */
export const __resetStudentForTests = () => {
  client?.stop();
  client = null;
  useStudent.setState({ phase: "form", connection: "stopped", info: null, error: null, ...blankExam });
};
