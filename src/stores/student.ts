import { create } from "zustand";
import { StudentClient, type ClientFailure, type ConnectionState, type Envelope, type JoinParams, type JoinedInfo, type SocketFactory } from "@/features/student/client";
import { createSequencer, mergeAnswers } from "@/features/student/paper";
import { clearQueue, loadQueue, mergeQueues, saveQueue, type PendingMap } from "@/features/student/queue";
import { clearSaved, saveJoin } from "@/features/student/saved";
import type { AnswerValue, Paper, ResultSummary } from "@/types/paper";
import type { SessionStatus } from "@/types/session";
import { monotonicNow } from "@/utils/sessionClock";

export type StudentPhase = "form" | "joining" | "in_session" | "ended";
export type SaveState = "saved" | "saving" | "offline";

/** How often unsent answers are retried while connected (covers pauses and transient refusals). */
const RETRY_MS = 3000;

interface StudentState {
  phase: StudentPhase;
  connection: ConnectionState;
  info: JoinedInfo | null;
  status: SessionStatus;
  remainingSeconds: number | null;
  /** monotonic timestamp (performance.now) of when remainingSeconds was received. */
  receivedAt: number;
  error: string | null;

  paper: Paper | null;
  answers: Record<string, AnswerValue>;
  /** Answers not yet acknowledged by the server. Persisted on disk and resent after any reconnect or restart. */
  pending: PendingMap;
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
  pendingCount(): number;
}

let client: StudentClient | null = null;
let attemptId: string | null = null;
let retryTimer: ReturnType<typeof setInterval> | undefined;
let socketFactory: SocketFactory | undefined;
const nextSeq = createSequencer();

const blankExam = {
  paper: null, answers: {}, pending: {}, current: 0, timeUp: false, submitting: false,
  submitted: false, autoSubmitted: false, result: null, examError: null,
} as const;

const applyTimer = (p: Record<string, any>) => ({
  status: (p.status as SessionStatus) ?? "WAITING",
  remainingSeconds: typeof p.remainingSeconds === "number" ? p.remainingSeconds : null,
  receivedAt: monotonicNow(),
});

export const useStudent = create<StudentState>((set, get) => {
  const setPending = (pending: PendingMap) => {
    set({ pending });
    if (attemptId) saveQueue(attemptId, pending);
  };

  /** Removes an entry only if it has not been superseded by a newer edit since it was sent. */
  const settle = (questionId: string, seq: number) => {
    const cur = get().pending[questionId];
    if (cur && cur.seq === seq) {
      const { [questionId]: _done, ...rest } = get().pending;
      setPending(rest);
    }
  };

  /** One batch upload of everything unsent. Entries stay queued unless the server gave a final verdict. */
  async function flushOnce(): Promise<void> {
    const entries = Object.entries(get().pending);
    if (entries.length === 0 || !client) return;
    const sent = new Map(entries.map(([q, p]) => [q, p.seq]));
    try {
      const r = await client.request("answers_sync", { answers: entries.map(([questionId, p]) => ({ questionId, answer: p.answer, clientSeq: p.seq })) });
      if (r.type !== "answers_synced") return; // e.g. rate_limited: keep everything, retry later
      for (const res of r.payload.results as Array<{ questionId: string; clientSeq: number; status: string; message?: string }>) {
        const seq = sent.get(res.questionId);
        if (seq === undefined) continue;
        switch (res.status) {
          case "stored":
          case "duplicate":
            settle(res.questionId, seq); break;
          case "exam_not_running":
          case "exam_not_started":
            break; // paused or not open yet: keep, the retry loop and resume will resend
          case "time_up":
            set({ timeUp: true, examError: "Time is up. Your last changes were not saved." });
            settle(res.questionId, seq); break;
          case "already_submitted":
            set({ submitted: true });
            settle(res.questionId, seq); break;
          default:
            set({ examError: res.message ?? "An answer could not be saved." });
            settle(res.questionId, seq);
        }
      }
    } catch { /* offline or timed out: stays queued */ }
  }

  let flushing: Promise<void> | null = null;
  let again = false;
  /** Serialises uploads so two flushes never race; edits made mid-flight trigger another pass. */
  function flushPending(): Promise<boolean> {
    if (flushing) {
      again = true;
      return flushing.then(() => Object.keys(get().pending).length === 0);
    }
    flushing = (async () => {
      do { again = false; await flushOnce(); } while (again);
    })().finally(() => { flushing = null; });
    return flushing.then(() => Object.keys(get().pending).length === 0);
  }

  const startRetryLoop = () => {
    clearInterval(retryTimer);
    retryTimer = setInterval(() => {
      const s = get();
      if (s.connection === "online" && !s.submitted && Object.keys(s.pending).length > 0) void flushPending();
    }, RETRY_MS);
  };
  const stopRetryLoop = () => clearInterval(retryTimer);

  return {
    phase: "form", connection: "stopped", info: null, status: "WAITING", remainingSeconds: null, receivedAt: 0, error: null,
    ...blankExam,

    join(params) {
      client?.stop();
      attemptId = null;
      set({ phase: "joining", error: null, connection: "connecting", info: null, ...blankExam });
      client = new StudentClient(params, {
        onState: (connection) => set({ connection }),
        onJoined: (info: JoinedInfo) => {
          attemptId = info.attemptId;
          saveJoin({ host: params.host, port: params.port, sessionCode: params.sessionCode, studentName: info.studentName, studentId: info.studentId, token: info.token, examTitle: info.exam.title });
          // Answers typed before an app restart or crash come back from disk (newest edit wins).
          set({ phase: info.status === "ENDED" ? "ended" : "in_session", info, pending: mergeQueues(loadQueue(info.attemptId), get().pending), ...applyTimer(info as unknown as Record<string, any>) });
          startRetryLoop();
          void flushPending();
          if (info.status === "RUNNING" && !get().paper) void get().openExam();
        },
        onMessage: (env: Envelope) => {
          const p = env.payload;
          switch (env.type) {
            case "session_ended":
              clearSaved();
              stopRetryLoop();
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
              set({ timeUp: true, remainingSeconds: 0, receivedAt: monotonicNow() });
              return;
            case "submitted":
              clearSaved();
              if (attemptId) clearQueue(attemptId);
              stopRetryLoop();
              set({ submitted: true, submitting: false, autoSubmitted: !!p.auto, result: (p.result as ResultSummary | null) ?? null, pending: {} });
              return;
          }
        },
        onFatal: (f: ClientFailure) => {
          // These answers mean the saved place is useless; forget it so the form does not offer it again.
          if (["session_not_found", "session_closed", "removed", "already_joined"].includes(f.code)) clearSaved();
          stopRetryLoop();
          client = null;
          set({ phase: "form", error: f.message, connection: "stopped", info: null, ...blankExam });
        },
      }, { factory: socketFactory });
      client.start();
    },

    leave() {
      client?.stop();
      client = null;
      stopRetryLoop();
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
      set({ answers: { ...s.answers, [questionId]: answer }, examError: null });
      setPending({ ...get().pending, [questionId]: { answer, seq } }); // on disk before it is even sent
      void flushPending();
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
          if (attemptId) clearQueue(attemptId);
          stopRetryLoop();
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

    pendingCount() { return Object.keys(get().pending).length; },
  };
});

/** Test hooks. `resetKeepingDisk` simulates closing and reopening the app: memory is lost, disk is not. */
export const __setSocketFactoryForTests = (f: SocketFactory | undefined) => { socketFactory = f; };
export const __resetStudentForTests = (opts: { keepDisk?: boolean } = {}) => {
  client?.stop();
  client = null;
  stopRetryLoop();
  if (!opts.keepDisk) { if (attemptId) clearQueue(attemptId); }
  attemptId = null;
  useStudent.setState({ phase: "form", connection: "stopped", info: null, error: null, ...blankExam });
};
function stopRetryLoop() { clearInterval(retryTimer); }
