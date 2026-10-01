# ProctorLAN — Database

SQLite via SQLx 0.8, migrations in `src-tauri/migrations/` (embedded in the binary with `sqlx::migrate!`, applied on startup).
File location: the OS app-data directory (`app_info.dataDir`), file `proctorlan.db`, WAL mode, foreign keys ON.

## Relationships
```
users ─< exams ─< questions ─< choices
          exams ─< exam_sessions ─< attempts >─ students
                                    attempts ─< answers >─ questions
                                    attempts ─< proctor_events
users ─< audit_logs          application_settings (key/value)
```

## Deviations from the original spec (and why)
| Change | Reason |
|---|---|
| `exams.created_by`, `exams.status` limited to `active/inactive` | Audit trail; matches the UI status badge. |
| `questions.required`, `questions.explanation` | Required by the question feature list. |
| Identification answers stored as `choices` rows with `is_correct=1` | Reuses one table for accepted answers; no JSON column to parse. |
| `exam_sessions.paused_at`, `paused_total_seconds` | Server-authoritative timer must extend `ends_at` by paused time (Phase 7). |
| `attempts.token_hash`, `total_points`, `passed` | Bearer token stored only as SHA-256; grading results cached on the attempt. |
| `answers.client_seq` | Monotonic counter per answer so retries/duplicates/out-of-order messages are harmless. |
| `total_points` on exams is **computed** (`SUM(questions.points)`) | Avoids a stored value that can drift from the questions. |

## Integrity rules (all enforced by the database, covered by tests)
- `UNIQUE(attempts.session_id, student_id)` — one attempt per student per session.
- `UNIQUE(answers.attempt_id, question_id)` + `client_seq` upsert — no duplicate answers; stale updates ignored.
- Partial unique index on `exam_sessions.session_code WHERE status <> 'ENDED'` — codes unique while joinable, reusable afterwards.
- `exam_sessions.exam_id ... ON DELETE RESTRICT` — exams with sessions can't be deleted or edited (results stay reproducible). Use *inactive* or *duplicate*.
- `CHECK` constraints on duration > 0, passing score 0–100, points > 0, enums.
- Exam create/replace, answer save, session transitions run in transactions; a failed multi-row write rolls back entirely.

## Access layer
`src-tauri/src/database/` — methods on `Database`, one file per area (`exams`, `sessions`, `attempts`, `users`, `misc`).
All SQL is parameterised. Password hashes and token hashes are `#[serde(skip)]` so they can never be serialised to the UI.
Answer keys (`choices.is_correct`) exist only in teacher-side models; student payloads are built separately in Phase 6.

## Verified
`cargo test`: 29 tests pass on Linux (22 integration tests on in-memory SQLite and a real file DB, 7 unit tests).
Not yet verified: behaviour with 50 concurrent writers (Phase 8/13 load test).
