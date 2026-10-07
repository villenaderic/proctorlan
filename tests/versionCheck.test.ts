// @vitest-environment node
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

const run = (...a: string[]) => spawnSync("node", ["scripts/check-version.mjs", ...a], { encoding: "utf8" });
const dirs: string[] = [];
function fixture(edit?: (f: { pkg: any; conf: any; cargo: string }) => void) {
  const d = mkdtempSync(join(tmpdir(), "ver-"));
  dirs.push(d);
  mkdirSync(join(d, "src-tauri"));
  cpSync("package.json", join(d, "package.json"));
  cpSync("src-tauri/tauri.conf.json", join(d, "src-tauri/tauri.conf.json"));
  cpSync("src-tauri/Cargo.toml", join(d, "src-tauri/Cargo.toml"));
  if (edit) {
    const f = { pkg: JSON.parse(readFileSync(join(d, "package.json"), "utf8")), conf: JSON.parse(readFileSync(join(d, "src-tauri/tauri.conf.json"), "utf8")), cargo: readFileSync(join(d, "src-tauri/Cargo.toml"), "utf8") };
    edit(f);
    writeFileSync(join(d, "package.json"), JSON.stringify(f.pkg));
    writeFileSync(join(d, "src-tauri/tauri.conf.json"), JSON.stringify(f.conf));
    writeFileSync(join(d, "src-tauri/Cargo.toml"), f.cargo);
  }
  return d;
}
afterEach(() => { while (dirs.length) rmSync(dirs.pop()!, { recursive: true, force: true }); });

describe("release version check", () => {
  it("the real project is consistent", () => {
    const r = run();
    expect(r.status, r.stderr).toBe(0);
  });
  it("detects a mismatch between files", () => {
    const d = fixture((f) => { f.conf.version = "9.9.9"; });
    const r = run("--root", d);
    expect(r.status).toBe(1);
    expect(r.stderr).toContain("versions differ");
  });
  it("rejects a version that is not x.y.z", () => {
    const d = fixture((f) => { f.pkg.version = "banana"; f.conf.version = "banana"; f.cargo = f.cargo.replace(/^version\s*=.*$/m, 'version = "banana"'); });
    expect(run("--root", d).status).toBe(1);
  });
  it("checks the git tag against the version", () => {
    const v = JSON.parse(readFileSync("package.json", "utf8")).version;
    expect(run("--tag", `v${v}`).status).toBe(0);
    const bad = run("--tag", "v99.0.0");
    expect(bad.status).toBe(1);
    expect(bad.stderr).toContain("does not match");
  });
});
