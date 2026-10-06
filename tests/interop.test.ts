// @vitest-environment node
/**
 * Real interop: the TypeScript StudentClient against the real Rust LAN server.
 * Skipped unless PROCTORLAN_INTEROP=1 (needs a compiled Rust toolchain):
 *   PROCTORLAN_INTEROP=1 npx vitest run tests/interop.test.ts
 */
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createInterface } from "node:readline";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { StudentClient, type JoinedInfo } from "../src/features/student/client";
import { __resetStudentForTests, useStudent } from "../src/stores/student";
import { saveQueue } from "../src/features/student/queue";
import { MemoryStorage } from "./helpers/fakeServer";

const enabled = process.env.PROCTORLAN_INTEROP === "1";

describe.skipIf(!enabled)("StudentClient ↔ Rust server", () => {
  let proc: ChildProcessWithoutNullStreams;
  let lines: AsyncIterator<string>;
  let info: { port: number; code: string };
  let code: string;

  const command = async (c: string) => { proc.stdin.write(c + "\n"); return JSON.parse((await lines.next()).value); };
  const until = async (cond: () => boolean, ms = 5000) => {
    const end = Date.now() + ms;
    while (!cond()) { if (Date.now() > end) throw new Error("timed out"); await new Promise((r) => setTimeout(r, 20)); }
  };

  beforeAll(async () => {
    (globalThis as any).localStorage = new MemoryStorage();
    proc = spawn("cargo", ["run", "--quiet", "--example", "dev_server"], { cwd: "src-tauri", env: { ...process.env, CARGO_PROFILE_DEV_DEBUG: "0" } });
    lines = createInterface({ input: proc.stdout })[Symbol.asyncIterator]();
    info = JSON.parse((await lines.next()).value);
    code = info.code;
  }, 600_000);
  afterAll(() => { proc?.stdin.write("quit\n"); proc?.kill(); });

  function connect(name: string, id: string, token: string | null = null) {
    const log = { joined: [] as JoinedInfo[], messages: [] as string[], fatal: [] as any[], states: [] as string[] };
    const client = new StudentClient(
      { host: "127.0.0.1", port: info.port, sessionCode: code, studentName: name, studentId: id, token },
      { onState: (s) => log.states.push(s), onJoined: (j) => log.joined.push(j), onMessage: (m) => log.messages.push(m.type), onFatal: (f) => log.fatal.push(f) },
      { backoffMs: [100, 200] },
    );
    client.start();
    return { client, log };
  }

  it("joins, receives the teacher's start, reconnects with its token, and is refused when impersonated", async () => {
    const a = connect("Ana Reyes", "2024-001");
    await until(() => a.log.joined.length === 1);
    expect(a.log.joined[0]).toMatchObject({ studentName: "Ana Reyes", studentId: "2024-001", resumed: false, status: "WAITING" });
    expect(a.log.joined[0].exam).toMatchObject({ title: "Dev Exam", questionCount: 2, durationMinutes: 30 });
    expect(a.log.joined[0].token).toHaveLength(64);

    const roster = await command("roster");
    expect(roster).toHaveLength(1);
    expect(roster[0]).toMatchObject({ name: "Ana Reyes", online: true });

    await command("start");
    await until(() => a.log.messages.includes("session_started"));

    // Impostor with the same ID but no token is refused (fatal, no retry loop).
    const imp = connect("Mallory", "2024-001");
    await until(() => imp.log.fatal.length === 1);
    expect(imp.log.fatal[0].code).toBe("already_joined");

    // Ana's real client resumes the same attempt with her saved token.
    a.client.stop();
    const back = connect("Ana Reyes", "2024-001", a.log.joined[0].token);
    await until(() => back.log.joined.length === 1);
    expect(back.log.joined[0]).toMatchObject({ resumed: true, attemptId: a.log.joined[0].attemptId, status: "RUNNING" });
    expect(back.log.joined[0].remainingSeconds).toBeGreaterThan(1700);

    await command("end");
    await until(() => back.log.messages.includes("session_ended"));
    back.client.stop();
  }, 30_000);

  it("full exam through the real student store: open, answer, autosave, submit, graded result", async () => {
    code = (await command("new")).code;
    __resetStudentForTests();
    const st = () => useStudent.getState();
    st().join({ host: "127.0.0.1", port: info.port, sessionCode: code, studentName: "Ana Reyes", studentId: "S-1", token: null });
    await until(() => st().phase === "in_session");
    expect(st().paper).toBeNull(); // still waiting: no questions before the teacher starts

    await command("start");
    await until(() => st().paper !== null);
    const paper = st().paper!;
    expect(paper.questions.map((q) => q.text)).toEqual(["1+1?", "Capital of France?"]);
    expect(JSON.stringify(paper)).not.toMatch(/isCorrect|Paris/); // answer keys never reach the student

    const mc = paper.questions[0];
    st().setAnswer(mc.id, mc.choices.find((c) => c.text === "2")!.id);
    st().setAnswer(paper.questions[1].id, "  PARIS ");
    await until(() => Object.keys(st().pending).length === 0); // both acknowledged by the server
    expect(st().saveState()).toBe("saved");

    st().reportFocus("FOCUS_LOST");
    st().reportFocus("FOCUS_RESTORED", 3000);
    await new Promise((r) => setTimeout(r, 300));

    expect(await st().submit()).toBe(true);
    expect(st().submitted).toBe(true);
    expect(st().result).toMatchObject({ score: 2, totalPoints: 2, percentage: 100, passed: true });
    const roster = await command("roster");
    expect(roster[0]).toMatchObject({ studentNumber: "S-1", status: "SUBMITTED", percentage: 100, passed: true, answered: 2, focusLostCount: 1, focusLostMs: 3000 });
    __resetStudentForTests();
  }, 30_000);

  it("answers queued on disk before a crash are uploaded as a batch after rejoining with the saved token", async () => {
    code = (await command("new")).code;
    __resetStudentForTests();
    const st = () => useStudent.getState();
    st().join({ host: "127.0.0.1", port: info.port, sessionCode: code, studentName: "Ben Cruz", studentId: "S-2", token: null });
    await until(() => st().phase === "in_session");
    await command("start");
    await until(() => st().paper !== null);
    const { attemptId, token } = st().info!;
    const [q1, q2] = st().paper!.questions;
    const right = q1.choices.find((c) => c.text === "2")!.id;

    // The app "crashes" with two answers still queued on disk (never reached the server).
    __resetStudentForTests({ keepDisk: true });
    saveQueue(attemptId, { [q1.id]: { answer: right, seq: Date.now() }, [q2.id]: { answer: "paris", seq: Date.now() + 1 } });

    st().join({ host: "127.0.0.1", port: info.port, sessionCode: code, studentName: "Ben Cruz", studentId: "S-2", token });
    await until(() => st().paper !== null && Object.keys(st().pending).length === 0);
    expect(st().info!.resumed).toBe(true);
    expect(st().answers).toMatchObject({ [q1.id]: right, [q2.id]: "paris" });
    const roster = await command("roster");
    expect(roster.find((r: any) => r.studentNumber === "S-2")).toMatchObject({ answered: 2, status: "IN_PROGRESS" });

    expect(await st().submit()).toBe(true);
    expect(st().result).toMatchObject({ percentage: 100, passed: true });
    __resetStudentForTests();
  }, 30_000);
});
