# ProctorLAN

Offline classroom exam proctoring over a local network. The teacher's computer hosts; student computers join with a session code. No internet, no cloud, no telemetry.

**Status: 1.0.0 — all 13 phases built.** Verified on Linux only. Windows/macOS installers are NOT VERIFIED; run `docs/release-checklist.md` before using it with real students. See `docs/known-limitations.md`.

## Features
- Teacher accounts (Argon2id), exam builder (multiple choice, multiple select, true/false, identification), exam import/export
- Session code join (name + student ID), up to 100 students, server-authoritative timer
- Server-side grading; answer keys never sent to students
- Offline answer queue with idempotent sync after reconnect
- Proctoring signals (focus lost, disconnects) as a timeline — never automatic penalties
- Results, statistics, charts, answer review, CSV export
- Verified backups, automatic backups, safe staged restore

## Quick start (teacher)
1. Install the app (`npm run tauri:build`, or the installer from a release) on the teacher PC.
2. First run: create the admin account.
3. Exams → create or import → activate → Sessions → open a session.
4. Give students the address and code shown. They open it in a browser on the same network.
5. Press **Start exam**. Results appear under Results when students submit.

Guides: `docs/teacher-guide.md`, `docs/student-guide.md`, `docs/networking.md`.

## Development
Requirements: Node.js 20+, Rust 1.90+, Linux libs `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev build-essential pkg-config libxdo-dev`; Windows needs WebView2 + MSVC build tools; macOS needs Xcode CLT.
```bash
npm install
npm run tauri:dev     # full desktop app
npm test && npm run typecheck
cd src-tauri && cargo test
npm run tauri:build   # installer for the current OS only
```
More: `docs/architecture.md`, `docs/testing.md`, `docs/packaging.md`, `docs/security.md`.
