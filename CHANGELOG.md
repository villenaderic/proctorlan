# Changelog

## 1.0.0
- Phase 1–4: app shell, SQLite database, teacher accounts (Argon2), exam builder with four question types.
- Phase 5–8: LAN server (HTTP + WebSocket, mDNS), student client, server-authoritative exam engine and timer, offline answer queue and synchronisation.
- Phase 9: proctoring signals (window focus, disconnects), live feed and per-student timeline.
- Phase 10: results, statistics, charts, answer review, student history, CSV export.
- Phase 11: verified backups, automatic backups, staged safe restore, exam file import/export.
- Phase 12: real app icons, installer configuration (Windows NSIS firewall rule, optional offline WebView2 installer), CI and release workflows, version consistency check, packaging docs.
  Verified: Linux .deb builds and the release binary runs in a real window (setup, dashboard, backup). Not verified: Windows/macOS installers and the workflows.
- Phase 13: testing and hardening. Clippy clean, `npm audit` 0 vulnerabilities, security test suite (route exposure, cross-session tokens, injection payloads, mass-assignment, answer-key leakage, secret serialisation, identity validation), 100-student load test, browser-student end-to-end run against the real server, and a real-window teacher + student run (join, start, answer, submit, 100% in Results).
  1.0.0 means feature complete. Verified on Linux only; see `docs/known-limitations.md`.
