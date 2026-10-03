/**
 * Student-side connection to the teacher's LAN server.
 *
 * - hello (session code) → join (name + student ID [+ token]) over one WebSocket.
 * - Heartbeats while connected; any frame from the server counts as "alive".
 * - Automatic reconnect with backoff, re-joining with the saved token so the student resumes
 *   the SAME attempt. Fatal answers (wrong code, session closed, removed…) stop retrying.
 * Phase 7+ extends the message handling (questions, answers); the plumbing stays.
 */

export interface SocketLike {
  send(data: string): void;
  close(): void;
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: ((ev: unknown) => void) | null;
  onerror: ((ev: unknown) => void) | null;
}
export type SocketFactory = (url: string) => SocketLike;

export interface Envelope { id: string; type: string; sessionId?: string | null; timestamp?: string; payload: Record<string, any> }

export interface JoinParams {
  host: string;
  port: number;
  sessionCode: string;
  studentName: string;
  studentId: string;
  token?: string | null;
}

export interface JoinedInfo {
  sessionId: string;
  attemptId: string;
  token: string;
  studentName: string;
  studentId: string;
  resumed: boolean;
  status: string;
  endsAt: string | null;
  remainingSeconds: number | null;
  serverTime: string;
  exam: { title: string; description: string; instructions: string; durationMinutes: number; questionCount: number };
}

export type ConnectionState = "connecting" | "online" | "reconnecting" | "stopped";
export interface ClientFailure { code: string; message: string }

export interface ClientEvents {
  onState(state: ConnectionState): void;
  /** First successful join, and every successful re-join after a reconnect. */
  onJoined(info: JoinedInfo): void;
  /** Server push: session_started, session_paused, session_resumed, session_ended, timer_sync. */
  onMessage(env: Envelope): void;
  /** Unrecoverable (retrying cannot help). The client has stopped itself. */
  onFatal(failure: ClientFailure): void;
}

export interface ClientOptions {
  factory?: SocketFactory;
  requestTimeoutMs?: number;
  silenceTimeoutMs?: number;
  backoffMs?: number[];
}

/** Errors for which reconnecting would only repeat the failure. */
const FATAL_CODES = new Set(["session_not_found", "session_closed", "already_joined", "invalid_identity", "too_many_attempts", "session_full", "removed", "hello_required"]);

const defaultFactory: SocketFactory = (url) => new WebSocket(url) as unknown as SocketLike;

let counter = 0;
const nextId = () => `c${Date.now().toString(36)}${(counter++).toString(36)}`;

export class StudentClient {
  private socket: SocketLike | null = null;
  private stopped = false;
  private token: string | null;
  private attempt = 0;
  private heartbeat: ReturnType<typeof setInterval> | undefined;
  private retry: ReturnType<typeof setTimeout> | undefined;
  private silence: ReturnType<typeof setTimeout> | undefined;
  private pending = new Map<string, { resolve: (e: Envelope) => void; reject: (e: ClientFailure) => void; timer: ReturnType<typeof setTimeout> }>();
  private readonly factory: SocketFactory;
  private readonly requestTimeoutMs: number;
  private readonly silenceTimeoutMs: number;
  private readonly backoff: number[];
  private heartbeatMs = 5000;

  constructor(private readonly params: JoinParams, private readonly events: ClientEvents, opts: ClientOptions = {}) {
    this.token = params.token ?? null;
    this.factory = opts.factory ?? defaultFactory;
    this.requestTimeoutMs = opts.requestTimeoutMs ?? 8000;
    this.silenceTimeoutMs = opts.silenceTimeoutMs ?? 20000;
    this.backoff = opts.backoffMs ?? [1000, 2000, 4000, 8000];
  }

  start() {
    this.stopped = false;
    this.events.onState("connecting");
    this.open();
  }

  stop() {
    this.stopped = true;
    this.teardown();
    this.events.onState("stopped");
  }

  /** Sends a request and resolves with the reply that references it. */
  request(type: string, payload: Record<string, unknown> = {}): Promise<Envelope> {
    return new Promise((resolve, reject) => {
      const sock = this.socket;
      if (!sock) return reject({ code: "not_connected", message: "Not connected." });
      const id = nextId();
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject({ code: "timeout", message: "The teacher's computer did not answer in time." });
      }, this.requestTimeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      try { sock.send(JSON.stringify({ id, type, payload })); } catch {
        clearTimeout(timer); this.pending.delete(id);
        reject({ code: "not_connected", message: "Not connected." });
      }
    });
  }

  // ------------------------------------------------------------------ internals

  private open() {
    let sock: SocketLike;
    try { sock = this.factory(`ws://${this.params.host}:${this.params.port}/ws`); } catch {
      this.lost(); return;
    }
    this.socket = sock;
    sock.onopen = () => { void this.handshake(sock); };
    sock.onmessage = (ev) => { if (this.socket === sock) this.receive(ev.data); };
    sock.onerror = () => { /* onclose follows */ };
    sock.onclose = () => { if (this.socket === sock) this.lost(); };
  }

  private async handshake(sock: SocketLike) {
    try {
      const welcome = await this.request("hello", { sessionCode: this.params.sessionCode });
      if (welcome.type === "error") return this.fail(welcome.payload as ClientFailure);
      this.heartbeatMs = Number(welcome.payload.heartbeatIntervalMs) || 5000;
      const joined = await this.request("join", {
        studentName: this.params.studentName, studentId: this.params.studentId, token: this.token,
      });
      if (this.socket !== sock || this.stopped) return;
      if (joined.type === "error") return this.fail(joined.payload as ClientFailure);
      const p = joined.payload;
      if (p.token) this.token = p.token as string;
      this.attempt = 0;
      this.events.onState("online");
      this.startHeartbeat();
      this.events.onJoined({ ...(p as any), token: this.token ?? "" } as JoinedInfo);
    } catch (e) {
      if (this.socket === sock && !this.stopped) {
        const f = e as ClientFailure;
        // A timeout during the handshake is a bad connection, not a verdict: try again.
        if (f.code === "timeout" || f.code === "not_connected") this.lost(); else this.fail(f);
      }
    }
  }

  private receive(data: unknown) {
    if (typeof data !== "string") return;
    let env: Envelope;
    try { env = JSON.parse(data); } catch { return; }
    this.armSilence();
    const reply = (env.payload?.inReplyTo as string | undefined) ?? "";
    const waiter = this.pending.get(reply);
    if (waiter) {
      clearTimeout(waiter.timer);
      this.pending.delete(reply);
      waiter.resolve(env);
      return;
    }
    if (env.type === "removed") return this.fail({ code: "removed", message: "Your teacher removed you from this session. Ask them if you should join again." });
    if (env.type === "error" && FATAL_CODES.has(env.payload?.code)) return this.fail(env.payload as ClientFailure);
    if (env.type === "heartbeat_ack") return;
    this.events.onMessage(env);
  }

  private startHeartbeat() {
    clearInterval(this.heartbeat);
    this.armSilence();
    this.heartbeat = setInterval(() => {
      try { this.socket?.send(JSON.stringify({ id: nextId(), type: "heartbeat", payload: {} })); } catch { /* close handler reconnects */ }
    }, this.heartbeatMs);
  }

  /** If the server goes quiet (cable pulled, Wi-Fi dropped without a close frame) force a reconnect. */
  private armSilence() {
    clearTimeout(this.silence);
    this.silence = setTimeout(() => {
      if (this.stopped || !this.socket) return;
      const s = this.socket;
      this.socket = null;
      try { s.close(); } catch { /* ignore */ }
      this.reject("not_connected");
      this.lost();
    }, this.silenceTimeoutMs);
  }

  private lost() {
    if (this.stopped) return;
    this.cleanupSocket();
    this.events.onState("reconnecting");
    const delay = this.backoff[Math.min(this.attempt, this.backoff.length - 1)];
    this.attempt += 1;
    this.retry = setTimeout(() => { if (!this.stopped) this.open(); }, delay);
  }

  private fail(f: ClientFailure) {
    this.stopped = true;
    this.teardown();
    this.events.onState("stopped");
    this.events.onFatal({ code: f.code, message: f.message });
  }

  private reject(code: string) {
    for (const [, w] of this.pending) { clearTimeout(w.timer); w.reject({ code, message: "Connection lost." }); }
    this.pending.clear();
  }

  private cleanupSocket() {
    clearInterval(this.heartbeat);
    clearTimeout(this.silence);
    this.reject("not_connected");
    const s = this.socket;
    this.socket = null;
    if (s) { s.onopen = s.onmessage = s.onclose = s.onerror = null; try { s.close(); } catch { /* ignore */ } }
  }

  private teardown() {
    clearTimeout(this.retry);
    this.cleanupSocket();
  }
}
