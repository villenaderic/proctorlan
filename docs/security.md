# ProctorLAN — Security (living document)

## Teacher authentication (Phase 3)
- **Hashing:** Argon2id (RustCrypto `argon2` 0.5, default m=19 MiB, t=2, p=1), random 128-bit salt per password. Plaintext never reaches the database layer or the logs.
- **Session model:** the signed-in teacher is held in Rust memory (`auth/session.rs`). The React UI holds no token or credential. Teacher operations are Tauri IPC commands, not HTTP routes, so they are not reachable from student machines.
- **Authorization:** every teacher command calls `AuthService::require_user()`; signed-out and idle (8 h) sessions get "Your session has expired".
- **Brute-force throttling:** 5 failed logins per username → 60 s lockout (even the right password is refused during lockout). Constants in `config.rs`.
- **No user enumeration:** unknown username and wrong password give the same message and similar timing (verification against a dummy hash).
- **First-run setup:** `INSERT … WHERE NOT EXISTS (SELECT 1 FROM users)` makes admin creation atomic; a racing second setup is rejected (tested).
- **Audit log:** setup, login, failed login, logout, password change. Never records passwords or what was typed in a failed login (people sometimes type a password into the username box).
- **Serialisation safety:** `password_hash` and `token_hash` are `#[serde(skip)]`.

## Known limits (honest list)
- **No password recovery.** The app is offline, so there is no email reset. Losing the only admin password means the local database must be reset (Phase 11 will document a safe procedure). Keep the password somewhere safe.
- Lockout and sessions are in memory; restarting the app clears them.
- Whoever has physical/OS-level access to the teacher's computer can read the SQLite file. The app does not encrypt data at rest.
- The password policy is length-based (8–128 chars), following current NIST guidance; it does not check against breached-password lists (needs internet).

## Planned (later phases)
Student attempt tokens + LAN request validation (Phase 5–6), payload size limits and WebSocket validation (Phase 5/8), answer keys never sent to students (Phase 6), full security test list (Phase 13).
