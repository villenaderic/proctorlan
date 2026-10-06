import type { SocketFactory, SocketLike } from "../../src/features/student/client";

/** In-memory stand-in for the teacher's server, speaking the real wire protocol. */
export class FakeServer {
  sockets: FakeSock[] = [];
  received: any[] = [];
  /** answers_sync verdict per item; default: stored. */
  verdict: (item: { questionId: string; clientSeq: number }) => string = () => "stored";
  /** When false the server accepts connections but never answers (a black hole). */
  responding = true;
  status = "RUNNING";
  attemptId = "att-1";
  resumed = false;
  /** Answers the server has stored (what a rejoining student's paper contains). */
  saved: Record<string, unknown> = {};

  factory: SocketFactory = () => {
    const s = new FakeSock(this);
    this.sockets.push(s);
    queueMicrotask(() => s.onopen?.({}));
    return s;
  };

  get live() { return this.sockets.filter((s) => !s.closed); }
  events() { return this.received.filter((m) => m.type === "proctor_event").map((m) => m.payload); }
  syncs() { return this.received.filter((m) => m.type === "answers_sync"); }

  handle(sock: FakeSock, msg: any) {
    this.received.push(msg);
    if (!this.responding) return;
    const reply = (type: string, payload: object) =>
      queueMicrotask(() => sock.onmessage?.({ data: JSON.stringify({ id: "s", type, payload: { inReplyTo: msg.id, ...payload } }) }));
    switch (msg.type) {
      case "hello": return reply("welcome", { heartbeatIntervalMs: 5000, status: this.status });
      case "join": {
        const resumed = this.resumed;
        this.resumed = true;
        return reply("joined", {
          sessionId: "s1", attemptId: this.attemptId, token: resumed ? null : "tok", resumed, studentName: "Ana", studentId: "A1",
          status: this.status, endsAt: null, remainingSeconds: 1800, serverTime: "t",
          exam: { title: "Exam", description: "", instructions: "", durationMinutes: 30, questionCount: 2 },
        });
      }
      case "start_exam": return reply("exam_paper", {
        status: this.status, remainingSeconds: 1800,
        paper: { allowReview: true, totalPoints: 2, answers: this.saved, questions: [
          { id: "q1", text: "One", type: "multiple_choice", points: 1, required: true, choices: [{ id: "c1", text: "A" }, { id: "c2", text: "B" }] },
          { id: "q2", text: "Two", type: "identification", points: 1, required: true, choices: [] },
        ] },
      });
      case "answers_sync": return reply("answers_synced", {
        results: msg.payload.answers.map((a: any) => {
          const status = this.verdict(a);
          if (status === "stored") this.saved[a.questionId] = a.answer;
          return { questionId: a.questionId, clientSeq: a.clientSeq, status };
        }),
      });
      case "submit": return reply("submitted", { attemptId: this.attemptId, auto: false, result: null });
      case "proctor_event": return reply("proctor_ack", { recorded: true });
      case "heartbeat": return;
    }
  }

  push(sock: FakeSock, type: string, payload: object = {}) {
    sock.onmessage?.({ data: JSON.stringify({ id: "p", type, payload }) });
  }
}

export class FakeSock implements SocketLike {
  onopen: any = null; onmessage: any = null; onclose: any = null; onerror: any = null;
  closed = false;
  constructor(private server: FakeServer) {}
  send(d: string) { if (!this.closed) this.server.handle(this, JSON.parse(d)); }
  close() { this.closed = true; }
  /** Connection lost (cable pulled): the page sees an onclose. */
  drop() { this.closed = true; this.onclose?.({}); }
}

export class MemoryStorage {
  data = new Map<string, string>();
  getItem(k: string) { return this.data.get(k) ?? null; }
  setItem(k: string, v: string) { this.data.set(k, v); }
  removeItem(k: string) { this.data.delete(k); }
  clear() { this.data.clear(); }
}
