# ProctorLAN — Testing

## Run everything
```bash
npm run typecheck && npm test && npm run build
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
npm run check:version
npm run e2e        # needs a running server + Playwright; see tests/e2e/student-flow.mjs
```

## What is covered
| Layer | Where | What it proves |
|---|---|---|
| Rust unit + integration | `src-tauri/tests/*.rs`, in-module tests | database, auth, exam CRUD, grading, sessions, WebSocket protocol, sync/idempotency, proctoring, results, backup/restore |
| Security | `src-tauri/tests/security.rs` | LAN server exposes only health/lookup/ws; tokens don't cross sessions; injection text stays inert; extra fields can't self-award points; no key in post-submit payloads; secrets not serialised; absurd identities refused |
| Load | `src-tauri/tests/load.rs` | 100 students join and finish an exam; the 101st is refused (debug build, in-memory DB, one machine: join 0.6 s, exam round 1.9 s) |
| Frontend | `tests/*.test.ts` | formatting, validation, version consistency, CSV/results helpers |
| Student UI E2E | `tests/e2e/student-flow.mjs` | Real browser (Chromium) student against the real Rust server: join, answer, submit |
| Real window | manual, Linux (Xvfb) | Real Tauri window: setup, import exam, open session, start, student answers and submits, Results shows 100% |

## Honest limits of the testing
- Everything above ran on **Linux** only.
- The load test is one machine over loopback, not 100 physical computers on Wi-Fi.
- No Windows/macOS build, installer, firewall rule or WebView2 behaviour was tested.
- GitHub Actions workflows have never run.
- Focus-loss detection depends on the platform webview; it is a signal, not proof.
