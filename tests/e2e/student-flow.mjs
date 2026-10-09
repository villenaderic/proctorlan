#!/usr/bin/env node
/**
 * End-to-end: the real student screens in a real browser against the real Rust LAN server.
 *
 *   npm run build
 *   node tests/e2e/student-flow.mjs
 *
 * Needs Playwright with a Chromium (`npm i -D playwright-core` and set PW_CHROMIUM=/path/to/chromium, or have
 * `playwright` installed globally) and a Rust toolchain. Not part of `npm test` because it starts processes.
 */
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { createRequire } from "node:module";
import assert from "node:assert/strict";

const require = createRequire(import.meta.url);
let chromium;
for (const m of ["playwright", "playwright-core", `${process.env.NPM_GLOBAL ?? ""}/playwright`]) {
  try { ({ chromium } = require(m)); break; } catch { /* try next */ }
}
if (!chromium) { console.error("Playwright not found. Install it: npm i -D playwright-core"); process.exit(2); }

const log = (m) => console.log(`  ✓ ${m}`);
const procs = [];
const kill = () => procs.forEach((p) => { try { p.kill(); } catch { /* gone */ } });
process.on("exit", kill);

// 1. The real Rust server with a prepared exam.
const server = spawn("cargo", ["run", "--quiet", "--example", "dev_server"], { cwd: "src-tauri", env: { ...process.env, CARGO_PROFILE_DEV_DEBUG: "0" } });
procs.push(server);
const lines = createInterface({ input: server.stdout })[Symbol.asyncIterator]();
const command = async (c) => { server.stdin.write(c + "\n"); return JSON.parse((await lines.next()).value); };
const info = JSON.parse((await lines.next()).value);

// 2. The built frontend, served statically.
const preview = spawn("npx", ["vite", "preview", "--port", "4174", "--strictPort"], { env: process.env });
procs.push(preview);
await new Promise((r) => { preview.stdout.on("data", (d) => String(d).includes("4174") && r()); setTimeout(r, 15000); });

const browser = await chromium.launch(process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {});
const page = await browser.newPage({ viewport: { width: 1100, height: 800 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => m.type() === "error" && errors.push(m.text()));

try {
  console.log("Student flow");
  await page.goto("http://localhost:4174/#/student");
  await page.getByLabel("Teacher's address").fill(`127.0.0.1:${info.port}`);
  await page.getByLabel("Session code").fill(info.code);
  await page.getByLabel("Full name").fill("Ana Reyes");
  await page.getByLabel("Student ID").fill("S-1");
  await page.getByRole("button", { name: /join/i }).first().click();
  await page.getByText(/Waiting for the teacher/i).waitFor({ timeout: 10000 });
  log("joins with code, name and ID, and waits in the room");

  await command("start");
  await page.getByText("1+1?").waitFor({ timeout: 10000 });
  assert.equal(await page.getByText(/Isecorrect|isCorrect/).count(), 0);
  log("exam appears when the teacher starts; no answer key in the page");

  await page.getByRole("group").getByText("2", { exact: true }).click();
  await page.getByRole("button", { name: /next/i }).click();
  await page.getByText("Capital of France?").waitFor();
  await page.getByRole("textbox").fill("  paris ");
  await page.getByText(/All answers saved/i).waitFor({ timeout: 8000 });
  log("answers autosave to the teacher's computer");

  // Leave the window for a moment: it is reported and the student is told.
  await page.evaluate(() => { Object.defineProperty(document, "hasFocus", { value: () => false, configurable: true }); window.dispatchEvent(new Event("blur")); });
  await page.waitForTimeout(600);
  await page.evaluate(() => { Object.defineProperty(document, "hasFocus", { value: () => true, configurable: true }); window.dispatchEvent(new Event("focus")); });
  await page.getByText(/You were away from the exam/i).waitFor({ timeout: 5000 });
  log("leaving the window is reported to the student on return");
  await page.waitForTimeout(500);
  const mid = (await command("roster"))[0];
  assert.equal(mid.focusLostCount, 1);
  assert.equal(mid.answered, 2);
  log("teacher's roster shows 2 answers and 1 focus-loss");

  await page.getByRole("button", { name: /submit exam/i }).first().click();
  await page.getByRole("dialog").getByRole("button", { name: "Submit" }).click();
  await page.getByText(/Your exam has been submitted/i).waitFor({ timeout: 10000 });
  log("submit confirmation and final screen");

  const done = (await command("roster"))[0];
  assert.equal(done.status, "SUBMITTED");
  assert.equal(done.percentage, 100);
  assert.equal(done.passed, true);
  log("graded on the server: 100%, passed");

  assert.deepEqual(errors, [], `browser errors: ${errors.join(" | ")}`);
  log("no console errors");
  console.log("\nStudent flow: PASSED");
} catch (e) {
  await page.screenshot({ path: "e2e-failure.png" }).catch(() => {});
  console.error("\nStudent flow: FAILED —", e.message, "\n(screenshot: e2e-failure.png)");
  process.exitCode = 1;
} finally {
  await browser.close();
  server.stdin.write("quit\n");
  kill();
}
