# Networking (Phase 5)

The teacher app embeds an Axum server for students. Teacher operations (login, exams, session
control, settings) use Tauri IPC and are **not reachable over the network**.

## Address
- Binds to one private LAN IPv4 (10.x, 172.16–31.x, 192.168.x, 169.254.x), never `0.0.0.0` by default.
- Falls back to `127.0.0.1` when no LAN is found (same-computer testing only; Settings shows a warning).
- Default port `38123`. Change both in **Settings → Network** (refused while a session is open).
- If the port is busy the app still starts; Settings shows the error and you pick another port.

## Discovery
mDNS `_proctorlan._tcp.local.` is advertised as a convenience (app name and version only, never a
session code). Many school networks block multicast: **entering `IP:port` + code always works.**

## HTTP
| Route | Purpose |
|---|---|
| `GET /api/health` | `{app, version, ok}` — lets a client confirm it found ProctorLAN |
| `POST /api/sessions/lookup` | `{sessionCode}` → `{sessionId, examTitle, status}` for open sessions only |
| `GET /ws` | WebSocket upgrade |

Bodies over 256 KB are rejected. 10 wrong codes in 60 s from one address → HTTP 429 / WS `too_many_attempts`.

## WebSocket protocol
Envelope: `{ id, type, sessionId, timestamp, payload }`. Text frames only, max 64 KB.

Client → server: `hello {sessionCode}` (must be first, within 10 s), `join {studentName, studentId, token?}`, `heartbeat`.
Server → client: `welcome`, `joined`, `removed`, `heartbeat_ack`, `timer_sync` (every 5 s), `session_started|paused|resumed|ended`, `error {code,message}`.

- Replies carry `payload.inReplyTo`.
- Malformed/unknown messages get an `error` and the connection stays up; oversize frames close it.
- A client silent for 20 s (4 missed 5 s heartbeats) is dropped. One missed beat never disconnects.
- Timer values are computed by the server (`endsAt`, `remainingSeconds`, `serverTime`); clients only display them.
- Answer keys never appear in any frame (tested).

## Joining (Phase 6)
- `join` creates one attempt per (session, student ID). IDs are trimmed and upper-cased, so `a1` and `A1` are the same student.
- The reply `joined` carries a 64-character bearer **token, once**. Only its SHA-256 hash is stored.
- Reconnecting sends the same `join` with that token and resumes the same attempt (`resumed: true`, no new token).
- A second person using an ID that already joined, or a wrong token, gets `already_joined` and nothing changes (not even the stored name).
- Simultaneous joins for one ID: exactly one wins.
- The teacher can **Remove** a student who has not started (status JOINED): the socket gets `removed` and closes, and the student can join again fresh. Students who started cannot be removed.
- `joined` includes the exam title, description, instructions, duration and question count, but **no questions or answers**.
- Students can join while the session is WAITING, RUNNING or PAUSED, never after it ENDS.

Answers and submit (Phase 7) are described in `docs/exam-engine.md`. Proctor events arrive in Phase 9.

## Not verified here
Tests run over real sockets on 127.0.0.1. **Not yet verified:** a second physical machine on a real LAN,
mDNS on real networks, and Windows Firewall prompts (first run may ask to allow ProctorLAN on private networks — allow it).

## Verified in Phase 6
- The TypeScript student client was run against the real Rust server (`PROCTORLAN_INTEROP=1 npx vitest run tests/interop.test.ts`, which spawns `cargo run --example dev_server`): join, session_started push, impostor refusal, rejoin by token, session_ended.
- **Not verified:** the student screens visually, and the Tauri WebView on Windows/macOS/Linux opening `ws://` to another machine (the CSP allows it; needs a two-computer test).

## Firewall
The Windows installer adds an inbound rule for ProctorLAN on Private and Domain networks. If students cannot connect and the teacher's own machine works on `127.0.0.1`, the firewall is the usual cause: check that the Wi-Fi is classed *Private*, that the rule "ProctorLAN" exists (Windows Defender Firewall → Inbound rules), or allow the port with `sudo ufw allow 38123/tcp` on Linux. Details: `packaging.md`.
