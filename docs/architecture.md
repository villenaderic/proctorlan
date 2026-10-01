# ProctorLAN — Architecture

> Working name: **ProctorLAN** (formerly "Offline Quiz Exam Proctor"). The name lives in `src/config.ts`,
> `src-tauri/src/config.rs`, and `src-tauri/tauri.conf.json`; renaming is a three-file change.

## 1. System diagram

```
 TEACHER PC (authoritative host)                       STUDENT PCs (same app, student mode)
 ┌──────────────────────────────────────┐              ┌───────────────────────────────┐
 │ Tauri window (React/TS UI)           │              │ Tauri window (React/TS UI)    │
 │   │ invoke()  (teacher-only IPC)     │              │   │ local answer store        │
 │   ▼                                  │              │   │ + sync queue (persisted)  │
 │ Rust core ── services ── SQLx ─ SQLite              │   ▼                           │
 │   │                                  │   HTTP/JSON  │ HTTP client + WS client       │
 │ Axum LAN server  ◄───────────────────┼──────────────┼───┘                           │
 │   ├─ REST  /api/*   (students)       │  WebSocket   │                               │
 │   └─ WS    /ws      (events)         ◄──────────────┤ focus/blur detection          │
 │ mDNS advertiser (optional)           │              └───────────────────────────────┘
 └──────────────────────────────────────┘
 Students never touch SQLite. Teacher IPC commands are never reachable over the LAN.
```

## 2. Component responsibilities

| Component | Responsibility |
|---|---|
| React UI | Rendering, form validation (UX only), local exam-taking state. Never trusted for scores/time. |
| Tauri commands | Teacher-only operations (exams, results, backups). Not exposed on the network. |
| Axum server | Student-facing REST + WS. Validates every request; separate auth from teacher. |
| Services (Rust) | Business rules: grading, session lifecycle, timer, randomization. No SQL in handlers. |
| Database layer | SQLx repositories, migrations, transactions. |
| Session manager | In-memory live state (connections, heartbeats) mirrored to DB at lifecycle points. |
| Platform trait | `PlatformExamControls` — fullscreen/focus helpers per OS with graceful fallback. |

## 3. Database (ER-style)

```
users ─< audit_logs
exams ─< questions ─< choices
exams ─< exam_sessions ─< attempts >─ students
attempts ─< answers >─ questions
attempts ─< proctor_events
application_settings (key/value)
```
Key constraints: `UNIQUE(session_id, student_id)` on attempts; `UNIQUE(attempt_id, question_id)` on answers
(idempotent sync); `UNIQUE(session_code)` among non-ended sessions; foreign keys ON; UUIDv4 text ids; UTC ISO-8601 timestamps.
Planned schema deviations (documented in `docs/database.md` in Phase 2): answers get `client_seq` for idempotency;
questions get `required`, `explanation`, `accepted_answers`; exams get `total_points` computed server-side.

## 4. REST API (student-facing, Axum)

| Route | Purpose |
|---|---|
| `GET /api/health` | Liveness + server time (clock sync) |
| `POST /api/sessions/join` | code + student name/ID → attempt token |
| `GET /api/attempts/me/exam` | Exam payload **without answer keys**, per-student randomization |
| `PUT /api/answers/{question_id}` | Idempotent answer upsert (`client_seq`) |
| `POST /api/attempts/me/submit` | Final answers → server grades |
| `GET /api/attempts/me/result` | Only if `show_results` |

Teacher operations (`exams`, `sessions`, `results`, `backups`, `auth`) are Tauri IPC commands, so there is no teacher HTTP surface on the LAN.

## 5. WebSocket protocol

Envelope: `{ "id": "uuid", "type": "...", "session_id": "...", "student_id": "...", "timestamp": "RFC3339", "payload": {} }`

- Student → server: `student_join`, `student_ready`, `heartbeat`, `answer_update`, `focus_lost`, `focus_restored`, `student_submit`
- Server → student: `session_started|paused|resumed|ended`, `timer_sync`, `answer_acknowledged`, `submission_acknowledged`, `student_status_changed`
- Server → teacher UI: `student_joined`, `student_left`, `answer_saved`, `student_submitted`, `proctor_event`, `connection_restored`
- Message `id` enables de-duplication; unknown/oversized/malformed frames are rejected and logged.

## 6. Authentication model
- Teacher: local account, Argon2id hash, in-memory session in the Rust core. First launch → "Create Administrator Account".
- Student: no account. Session code + name + student ID → server-issued random attempt token (bearer) scoped to one attempt.

## 7. Exam / session lifecycle
`CREATED → WAITING → RUNNING ⇄ PAUSED → ENDED`. Joins allowed in WAITING/RUNNING/PAUSED (rejoin). Server stores `started_at`/`ends_at`
(pause extends `ends_at` by paused duration). Students compute remaining time from server offset (`timer_sync`), not local clock.

## 8. Student lifecycle
`JOINING → WAITING_ROOM → IN_PROGRESS → SUBMITTED` with connection overlay `CONNECTED / RECONNECTING / DISCONNECTED`.
Answers: save locally → try sync → queue on failure → retry with backoff → flush on reconnect.

## 9. LAN connection lifecycle
mDNS browse (optional) → manual IP:port fallback → `GET /api/health` → join → WS connect → heartbeat every 5 s → 4 missed = Disconnected → auto-reconnect + resume with token.

## 10. UI screen map
Welcome → (Teacher) Setup/Login → Dashboard | Exams (list, builder: Basic/Questions/Settings/Preview) | Sessions (live monitor) | Students | Results | Backups | Settings.
Welcome → (Student) Join (discover/manual) → Waiting Room → Exam → Submit dialog → Confirmation/Result.
Design target: the 12-screen mockup supplied with the project.

## 11. Folder structure
See repository root; frontend under `src/` (features/*, stores/, services/), Rust under `src-tauri/src/`
(commands, server, websocket, database, models, services, auth, networking, proctoring, errors, config).

## 12. Dependencies
Phase 1 (installed): tauri 2, serde, thiserror, tracing; react, react-router, zustand, tailwind 4, lucide-react, cva, vitest.
Later phases: sqlx(sqlite), tokio, axum, argon2, uuid, chrono, mdns-sd, rand, recharts, csv, tower-http.

## 13. Phases
1 Foundation · 2 Database · 3 Auth · 4 Exam Builder · 5 LAN Server · 6 Student Client · 7 Exam Engine · 8 Sync · 9 Proctoring · 10 Results · 11 Backup/Export · 12 Packaging · 13 Testing.

## 14. Testing strategy
Rust unit tests (grading, timer, codes, validation, randomization) → SQLx integration tests on in-memory SQLite → Axum/WS tests with real sockets on 127.0.0.1 → Vitest for frontend logic → manual multi-machine plan. Anything not executed is labelled NOT VERIFIED.

## 15. Risks & limitations
- No app can fully prevent cheating or lock an OS; we deter and record (focus loss, disconnects) and leave judgement to the teacher.
- Focus detection and kiosk/fullscreen behaviour differ by OS (Wayland vs X11, macOS Spaces); degrade gracefully.
- Firewalls/AP client isolation commonly block LAN traffic; mDNS often blocked on school Wi-Fi → manual IP is the guaranteed path.
- Unsigned installers trigger SmartScreen/Gatekeeper warnings; documented in Phase 12.
- Plain HTTP on a trusted classroom LAN; the attempt token mitigates impersonation but not a hostile network.
