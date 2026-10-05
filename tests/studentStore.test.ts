import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { __resetStudentForTests, __setSocketFactoryForTests, useStudent } from "../src/stores/student";
import { loadQueue } from "../src/features/student/queue";
import { FakeServer, MemoryStorage } from "./helpers/fakeServer";

let server: FakeServer;
let storage: MemoryStorage;
const st = () => useStudent.getState();
const settle = () => vi.advanceTimersByTimeAsync(0);

async function joinAndOpen() {
  st().join({ host: "10.0.0.5", port: 38123, sessionCode: "ABCDE", studentName: "Ana", studentId: "A1", token: null });
  await vi.advanceTimersByTimeAsync(10);
  await settle();
}

beforeEach(() => {
  vi.useFakeTimers();
  storage = new MemoryStorage();
  vi.stubGlobal("localStorage", storage);
  server = new FakeServer();
  __setSocketFactoryForTests(server.factory);
  __resetStudentForTests();
});
afterEach(() => {
  __resetStudentForTests();
  __setSocketFactoryForTests(undefined);
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("answer sync (student store)", () => {
  it("joins, loads the paper and uploads an answer; the queue empties once the server acknowledges", async () => {
    await joinAndOpen();
    expect(st().paper?.questions).toHaveLength(2);
    st().setAnswer("q1", "c1");
    expect(loadQueue("att-1").q1.answer).toBe("c1"); // on disk before the server has even answered
    await vi.advanceTimersByTimeAsync(10);
    expect(server.syncs().at(-1).payload.answers[0]).toMatchObject({ questionId: "q1", answer: "c1" });
    expect(st().pendingCount()).toBe(0);
    expect(loadQueue("att-1")).toEqual({});
    expect(st().saveState()).toBe("saved");
  });

  it("keeps edits made while offline, on disk, and uploads them as one batch on reconnect", async () => {
    await joinAndOpen();
    server.sockets[0].drop(); // Wi-Fi gone
    await vi.advanceTimersByTimeAsync(0);
    expect(st().connection).toBe("reconnecting");
    st().setAnswer("q1", "c2");
    st().setAnswer("q2", "paris");
    await vi.advanceTimersByTimeAsync(10);
    expect(st().pendingCount()).toBe(2);
    expect(st().saveState()).toBe("offline");
    expect(Object.keys(loadQueue("att-1")).sort()).toEqual(["q1", "q2"]);

    await vi.advanceTimersByTimeAsync(1100); // backoff elapses, handshake completes
    await vi.advanceTimersByTimeAsync(50);
    expect(st().connection).toBe("online");
    const batch = server.syncs().at(-1);
    expect(batch.payload.answers.map((a: any) => a.questionId).sort()).toEqual(["q1", "q2"]);
    expect(st().pendingCount()).toBe(0);
    expect(st().answers).toMatchObject({ q1: "c2", q2: "paris" });
  });

  it("survives an app restart: answers on disk are resent after rejoining", async () => {
    await joinAndOpen();
    server.responding = false; // the teacher's PC stops answering; the student keeps working
    st().setAnswer("q1", "c1");
    await vi.advanceTimersByTimeAsync(10);
    expect(Object.keys(loadQueue("att-1"))).toEqual(["q1"]);

    __resetStudentForTests({ keepDisk: true }); // app closed and reopened
    expect(st().pendingCount()).toBe(0);
    server.responding = true;
    server.received = [];
    await joinAndOpen();
    expect(st().pendingCount()).toBe(0);
    const batch = server.syncs()[0];
    expect(batch.payload.answers[0]).toMatchObject({ questionId: "q1", answer: "c1" });
    expect(st().answers.q1).toBe("c1");
    expect(loadQueue("att-1")).toEqual({});
  });

  it("a newer edit made while an upload is in flight is not lost to the older acknowledgement", async () => {
    await joinAndOpen();
    server.responding = false;
    st().setAnswer("q1", "c1");
    await vi.advanceTimersByTimeAsync(10);
    st().setAnswer("q1", "c2"); // changed mind before the first was acknowledged
    expect(st().pending.q1.answer).toBe("c2");
    server.responding = true;
    await vi.advanceTimersByTimeAsync(9000); // first request times out, retry loop resends
    expect(st().pendingCount()).toBe(0);
    const last = server.syncs().at(-1).payload.answers[0];
    expect(last).toMatchObject({ questionId: "q1", answer: "c2" });
  });

  it("keeps answers while the exam is paused and retries until the teacher resumes", async () => {
    await joinAndOpen();
    server.verdict = () => "exam_not_running";
    st().setAnswer("q1", "c1");
    await vi.advanceTimersByTimeAsync(10);
    expect(st().pendingCount()).toBe(1);
    const before = server.syncs().length;
    await vi.advanceTimersByTimeAsync(3100);
    expect(server.syncs().length).toBeGreaterThan(before); // retry loop
    expect(st().pendingCount()).toBe(1);
    server.verdict = () => "stored";
    await vi.advanceTimersByTimeAsync(3100);
    expect(st().pendingCount()).toBe(0);
  });

  it("a time_up verdict drops the entry, locks the exam and tells the student", async () => {
    await joinAndOpen();
    server.verdict = () => "time_up";
    st().setAnswer("q1", "c1");
    await vi.advanceTimersByTimeAsync(10);
    expect(st().timeUp).toBe(true);
    expect(st().examError).toMatch(/Time is up/);
    expect(st().pendingCount()).toBe(0);
    st().setAnswer("q2", "late");
    expect(st().pendingCount()).toBe(0); // locked locally too
  });

  it("refuses to submit while answers are unsent, then submits once they reach the server", async () => {
    await joinAndOpen();
    server.responding = false;
    st().setAnswer("q1", "c1");
    const attempt = st().submit();
    await vi.advanceTimersByTimeAsync(20_000); // two request timeouts (the edit mid-flight forces a second pass)
    expect(await attempt).toBe(false);
    expect(st().submitted).toBe(false);
    expect(st().examError).toMatch(/have not reached/);

    server.responding = true;
    await vi.advanceTimersByTimeAsync(2500); // the silent connection was dropped; it reconnects on its own
    expect(st().connection).toBe("online");
    const retry = st().submit();
    await vi.advanceTimersByTimeAsync(50);
    expect(await retry).toBe(true);
    expect(st().submitted).toBe(true);
    expect(loadQueue("att-1")).toEqual({});
    expect(server.received.some((m) => m.type === "submit")).toBe(true);
  });

  it("only uploads answers that are not yet acknowledged and never double-submits", async () => {
    await joinAndOpen();
    st().setAnswer("q1", "c1");
    await vi.advanceTimersByTimeAsync(10);
    const first = server.syncs().length;
    await vi.advanceTimersByTimeAsync(10_000); // nothing pending: no more uploads
    expect(server.syncs().length).toBe(first);
    const [a, b] = await Promise.all([st().submit(), st().submit()]);
    await vi.advanceTimersByTimeAsync(50);
    expect([a, b]).toContain(true);
    expect(server.received.filter((m) => m.type === "submit")).toHaveLength(1);
  });

  it("closes this window when the attempt is taken over elsewhere and does not reconnect", async () => {
    await joinAndOpen();
    server.push(server.sockets[0], "replaced", { attemptId: "att-1" });
    await vi.advanceTimersByTimeAsync(0);
    expect(st().phase).toBe("form");
    expect(st().error).toMatch(/another device or window/);
    const sockets = server.sockets.length;
    await vi.advanceTimersByTimeAsync(30_000);
    expect(server.sockets.length).toBe(sockets);
  });

  it("the countdown follows the server's number, not the computer's calendar clock", async () => {
    await joinAndOpen();
    expect(st().remainingSeconds).toBe(1800);
    vi.setSystemTime(new Date("2035-01-01T00:00:00Z")); // student fiddles with the system date
    const { displayRemaining, monotonicNow } = await import("../src/utils/sessionClock");
    const shown = displayRemaining({ status: st().status, remainingSeconds: st().remainingSeconds }, st().receivedAt, monotonicNow());
    expect(shown).toBeGreaterThan(1790);
    expect(shown).toBeLessThanOrEqual(1800);
  });
});
