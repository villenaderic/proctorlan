import { describe, expect, it } from "vitest";
import { cleanPath, describeContents, fileSize, looksAbsolute } from "../src/features/backups/format";

describe("backup formatting", () => {
  it("formats sizes", () => {
    expect(fileSize(500)).toBe("500 B");
    expect(fileSize(2048)).toBe("2 KB");
    expect(fileSize(5.5 * 1024 * 1024)).toBe("5.5 MB");
  });
  it("describes contents with correct plurals", () => {
    expect(describeContents({ schemaVersion: 1, exams: 1, sessions: 2, students: 0, attempts: 1 })).toBe("1 exam, 2 sessions, 0 students, 1 attempt");
  });
  it("cleans pasted paths", () => {
    expect(cleanPath('  "E:\\Backups\\a.db" ')).toBe("E:\\Backups\\a.db");
    expect(cleanPath("/home/a/b.db")).toBe("/home/a/b.db");
  });
  it("recognises absolute paths on Windows and POSIX only", () => {
    for (const ok of ["C:\\x\\a.db", "e:/x/a.db", "\\\\server\\share\\a.db", "/home/a/a.db", '"D:\\a.db"']) expect(looksAbsolute(ok), ok).toBe(true);
    for (const bad of ["", "a.db", "..\\a.db", "x/y.db", "C:a.db"]) expect(looksAbsolute(bad), bad).toBe(false);
  });
});
