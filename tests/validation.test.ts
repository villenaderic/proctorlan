import { describe, expect, it } from "vitest";
import { passwordError, usernameError } from "../src/utils/validation";

describe("username validation", () => {
  it("accepts normal usernames", () => expect(usernameError("ms.reyes_1")).toBeNull());
  it("rejects too short, spaces and symbols", () => {
    expect(usernameError("ab")).not.toBeNull();
    expect(usernameError("has space")).not.toBeNull();
    expect(usernameError("x'; DROP")).not.toBeNull();
  });
});

describe("password validation", () => {
  it("requires 8+ characters", () => {
    expect(passwordError("short", "teacher")).not.toBeNull();
    expect(passwordError("long-enough", "teacher")).toBeNull();
  });
  it("rejects password equal to username", () => expect(passwordError("Teacher01", "teacher01")).not.toBeNull());
  it("counts characters not bytes", () => expect(passwordError("😀😀😀😀😀😀😀😀", "x")).toBeNull());
});
