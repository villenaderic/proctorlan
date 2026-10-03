import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { StudentClient, type ClientEvents, type JoinedInfo, type SocketLike } from "../src/features/student/client";

class FakeSocket implements SocketLike {
  onopen: any = null; onmessage: any = null; onclose: any = null; onerror: any = null;
  sent: any[] = [];
  closed = false;
  send(d: string) { this.sent.push(JSON.parse(d)); }
  close() { this.closed = true; }
  open() { this.onopen?.({}); }
  push(env: object) { this.onmessage?.({ data: JSON.stringify(env) }); }
  drop() { this.onclose?.({}); }
  last(type: string) { return [...this.sent].reverse().find((m) => m.type === type); }
}

function setup(opts: { token?: string | null } = {}) {
  const sockets: FakeSocket[] = [];
  const log = { states: [] as string[], joined: [] as JoinedInfo[], messages: [] as any[], fatal: [] as any[] };
  const events: ClientEvents = {
    onState: (s) => log.states.push(s), onJoined: (j) => log.joined.push(j),
    onMessage: (m) => log.messages.push(m), onFatal: (f) => log.fatal.push(f),
  };
  const client = new StudentClient(
    { host: "10.0.0.5", port: 38123, sessionCode: "ABCDE", studentName: "Ana", studentId: "A1", token: opts.token },
    events, { factory: () => { const s = new FakeSocket(); sockets.push(s); return s; }, backoffMs: [1000, 2000, 4000] },
  );
  return { client, sockets, log };
}

const welcome = (to: string) => ({ type: "welcome", payload: { inReplyTo: to, heartbeatIntervalMs: 5000, status: "WAITING" } });
const joinedMsg = (to: string, token: string | null, resumed = false) => ({
  type: "joined",
  payload: { inReplyTo: to, sessionId: "s1", attemptId: "a1", token, resumed, studentName: "Ana", studentId: "A1", status: "WAITING",
    endsAt: null, remainingSeconds: null, serverTime: "t", exam: { title: "T", description: "", instructions: "", durationMinutes: 30, questionCount: 1 } },
});

/** Drives one full handshake on the given socket. */
async function handshake(s: FakeSocket, token: string | null, resumed = false) {
  s.open();
  await vi.advanceTimersByTimeAsync(0);
  s.push(welcome(s.last("hello").id));
  await vi.advanceTimersByTimeAsync(0);
  s.push(joinedMsg(s.last("join").id, token, resumed));
  await vi.advanceTimersByTimeAsync(0);
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("StudentClient", () => {
  it("does hello then join and reports the attempt and token", async () => {
    const { client, sockets, log } = setup();
    client.start();
    await handshake(sockets[0], "tok123");
    expect(sockets[0].last("hello").payload).toEqual({ sessionCode: "ABCDE" });
    expect(sockets[0].last("join").payload).toMatchObject({ studentName: "Ana", studentId: "A1", token: null });
    expect(log.joined[0]).toMatchObject({ attemptId: "a1", token: "tok123", resumed: false });
    expect(log.states).toEqual(["connecting", "online"]);
  });

  it("sends heartbeats at the server's interval", async () => {
    const { client, sockets } = setup();
    client.start();
    await handshake(sockets[0], "t");
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sockets[0].sent.filter((m) => m.type === "heartbeat").length).toBe(2);
  });

  it("reconnects with backoff and resumes with the saved token", async () => {
    const { client, sockets, log } = setup();
    client.start();
    await handshake(sockets[0], "tok123");
    sockets[0].drop();
    expect(log.states.at(-1)).toBe("reconnecting");
    await vi.advanceTimersByTimeAsync(999);
    expect(sockets.length).toBe(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(sockets.length).toBe(2);
    await handshake(sockets[1], null, true); // resumed joins return no new token
    expect(sockets[1].last("join").payload.token).toBe("tok123");
    expect(log.joined[1]).toMatchObject({ resumed: true, token: "tok123" });
    expect(log.states.at(-1)).toBe("online");
  });

  it("backs off progressively while the teacher is unreachable", async () => {
    const { client, sockets } = setup();
    client.start();
    sockets[0].drop();
    await vi.advanceTimersByTimeAsync(1000); // retry 1
    sockets[1].drop();
    await vi.advanceTimersByTimeAsync(1999);
    expect(sockets.length).toBe(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(sockets.length).toBe(3);
    sockets[2].drop();
    await vi.advanceTimersByTimeAsync(4000);
    sockets[3].drop();
    await vi.advanceTimersByTimeAsync(4000); // capped at the last step
    expect(sockets.length).toBe(5);
  });

  it("stops and reports fatal answers instead of retrying", async () => {
    const { client, sockets, log } = setup();
    client.start();
    sockets[0].open();
    await vi.advanceTimersByTimeAsync(0);
    sockets[0].push({ type: "error", payload: { inReplyTo: sockets[0].last("hello").id, code: "session_not_found", message: "No open session has that code." } });
    await vi.advanceTimersByTimeAsync(0);
    expect(log.fatal).toEqual([{ code: "session_not_found", message: "No open session has that code." }]);
    expect(log.states.at(-1)).toBe("stopped");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(sockets.length).toBe(1);
  });

  it("treats a rejected join (already joined) as fatal", async () => {
    const { client, sockets, log } = setup();
    client.start();
    sockets[0].open();
    await vi.advanceTimersByTimeAsync(0);
    sockets[0].push(welcome(sockets[0].last("hello").id));
    await vi.advanceTimersByTimeAsync(0);
    sockets[0].push({ type: "error", payload: { inReplyTo: sockets[0].last("join").id, code: "already_joined", message: "taken" } });
    await vi.advanceTimersByTimeAsync(0);
    expect(log.fatal[0].code).toBe("already_joined");
  });

  it("stops when the teacher removes the student", async () => {
    const { client, sockets, log } = setup();
    client.start();
    await handshake(sockets[0], "t");
    sockets[0].push({ type: "removed", sessionId: "s1", payload: { attemptId: "a1" } });
    expect(log.fatal[0].code).toBe("removed");
    sockets[0].drop();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(sockets.length).toBe(1);
  });

  it("forwards server pushes to the app but swallows heartbeat acks", async () => {
    const { client, sockets, log } = setup();
    client.start();
    await handshake(sockets[0], "t");
    sockets[0].push({ type: "session_started", payload: { status: "RUNNING", remainingSeconds: 1800 } });
    sockets[0].push({ type: "heartbeat_ack", payload: {} });
    sockets[0].push({ type: "timer_sync", payload: {} });
    expect(log.messages.map((m) => m.type)).toEqual(["session_started", "timer_sync"]);
  });

  it("reconnects when the server goes silent without closing", async () => {
    const { client, sockets } = setup();
    client.start();
    await handshake(sockets[0], "t");
    await vi.advanceTimersByTimeAsync(20_001);
    expect(sockets[0].closed).toBe(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(sockets.length).toBe(2);
  });

  it("ignores garbage frames and stale sockets", async () => {
    const { client, sockets, log } = setup();
    client.start();
    await handshake(sockets[0], "t");
    sockets[0].onmessage?.({ data: "not json" });
    sockets[0].onmessage?.({ data: new ArrayBuffer(2) });
    sockets[0].drop();
    await vi.advanceTimersByTimeAsync(1000);
    sockets[0].push({ type: "session_ended", payload: {} }); // old socket must not leak events
    expect(log.messages.length).toBe(0);
  });

  it("a user stop never reconnects", async () => {
    const { client, sockets, log } = setup();
    client.start();
    await handshake(sockets[0], "t");
    client.stop();
    sockets[0].drop();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(sockets.length).toBe(1);
    expect(log.states.at(-1)).toBe("stopped");
  });
});
