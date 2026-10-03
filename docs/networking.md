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

Client → server: `hello {sessionCode}` (must be first, within 10 s), `heartbeat`.
Server → client: `welcome`, `heartbeat_ack`, `timer_sync` (every 5 s), `session_started|paused|resumed|ended`, `error {code,message}`.

- Replies carry `payload.inReplyTo`.
- Malformed/unknown messages get an `error` and the connection stays up; oversize frames close it.
- A client silent for 20 s (4 missed 5 s heartbeats) is dropped. One missed beat never disconnects.
- Timer values are computed by the server (`endsAt`, `remainingSeconds`, `serverTime`); clients only display them.
- Answer keys never appear in any frame (tested).

Student name/ID join, answers, submit and proctor events are added in Phases 6–9.

## Not verified here
Tests run over real sockets on 127.0.0.1. **Not yet verified:** a second physical machine on a real LAN,
mDNS on real networks, and Windows Firewall prompts (first run may ask to allow ProctorLAN on private networks — allow it).
