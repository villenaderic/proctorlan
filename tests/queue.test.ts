import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clearQueue, loadQueue, mergeQueues, saveQueue } from "../src/features/student/queue";
import { MemoryStorage } from "./helpers/fakeServer";

let storage: MemoryStorage;
beforeEach(() => { storage = new MemoryStorage(); vi.stubGlobal("localStorage", storage); });
afterEach(() => vi.unstubAllGlobals());

describe("persisted answer queue", () => {
  it("round-trips per attempt and keeps attempts separate", () => {
    saveQueue("a1", { q1: { answer: "c1", seq: 5 }, q2: { answer: ["x", "y"], seq: 6 } });
    saveQueue("a2", { q1: { answer: "other", seq: 1 } });
    expect(loadQueue("a1")).toEqual({ q1: { answer: "c1", seq: 5 }, q2: { answer: ["x", "y"], seq: 6 } });
    expect(loadQueue("a2").q1.answer).toBe("other");
    expect(loadQueue("nobody")).toEqual({});
  });

  it("saving an empty queue or clearing removes the entry", () => {
    saveQueue("a1", { q1: { answer: "c1", seq: 1 } });
    saveQueue("a1", {});
    expect(storage.data.size).toBe(0);
    saveQueue("a1", { q1: { answer: "c1", seq: 1 } });
    clearQueue("a1");
    expect(storage.data.size).toBe(0);
  });

  it("ignores corrupt, malformed or hostile stored data", () => {
    for (const bad of ["not json", "null", "[]", "42", '{"q1":{"answer":5,"seq":1}}', '{"q1":{"answer":"x","seq":"1"}}', '{"q1":{"answer":"x","seq":-3}}', '{"q1":null}']) {
      storage.setItem("proctorlan.student.queue.a1", bad);
      expect(loadQueue("a1"), bad).toEqual({});
    }
    storage.setItem("proctorlan.student.queue.a1", '{"good":{"answer":"x","seq":2},"bad":{"answer":{},"seq":1}}');
    expect(Object.keys(loadQueue("a1"))).toEqual(["good"]);
  });

  it("never throws when storage is unavailable or full", () => {
    vi.stubGlobal("localStorage", { getItem() { throw new Error("blocked"); }, setItem() { throw new Error("quota"); }, removeItem() { throw new Error("blocked"); } });
    expect(() => saveQueue("a1", { q: { answer: "x", seq: 1 } })).not.toThrow();
    expect(loadQueue("a1")).toEqual({});
    expect(() => clearQueue("a1")).not.toThrow();
  });

  it("merging keeps the newest edit per question", () => {
    const merged = mergeQueues({ q1: { answer: "old", seq: 1 }, q2: { answer: "keep", seq: 9 } }, { q1: { answer: "new", seq: 2 }, q2: { answer: "stale", seq: 3 }, q3: { answer: "add", seq: 1 } });
    expect(merged).toEqual({ q1: { answer: "new", seq: 2 }, q2: { answer: "keep", seq: 9 }, q3: { answer: "add", seq: 1 } });
  });
});
