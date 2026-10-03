import { describe, expect, it } from "vitest";
import { isAddressError, normalizeCode, parseHostAddress } from "../src/features/student/address";

describe("parseHostAddress", () => {
  it("parses ip with and without port", () => {
    expect(parseHostAddress("192.168.1.20:38123")).toEqual({ host: "192.168.1.20", port: 38123 });
    expect(parseHostAddress(" 10.0.0.5 ")).toEqual({ host: "10.0.0.5", port: 38123 });
    expect(parseHostAddress("teacher-pc.local:40000")).toEqual({ host: "teacher-pc.local", port: 40000 });
  });
  it("rejects anything that is not a plain host[:port]", () => {
    for (const bad of ["", "   ", "http://10.0.0.5", "10.0.0.5/ws", "user@10.0.0.5", "10.0.0.256", "10.0.0.5:0", "10.0.0.5:70000", "10.0.0.5:abc", "a b", "-bad.host", "[::1]:80", "10.0.0.5:1:2"]) {
      expect(isAddressError(parseHostAddress(bad)), bad).toBe(true);
    }
  });
});

describe("normalizeCode", () => {
  it("uppercases and removes spaces", () => expect(normalizeCode(" ab c2 d ")).toBe("ABC2D"));
});
