# Synchronization (Phase 8)

Goal: a student's answers are never silently lost, and the server never double-counts or trusts a confused client.

## Student side (offline queue)
1. Every edit gets a **sequence number** (time-based, always increasing, even across restarts) and is written to the computer's local storage **before** it is sent.
2. Unsent answers go to the server as one `answers_sync` batch. An entry leaves the queue only when the server says `stored` or `duplicate` (or gives a final verdict: `time_up`, `already_submitted`, `invalid_answer`).
3. While the exam is paused the server answers `exam_not_running`; entries stay queued and a retry runs every 3 s, plus immediately on resume.
4. After a reconnect or an **app restart + rejoin** the queue is loaded from disk and uploaded (newest edit per question wins).
5. **Submit** first uploads the queue. If it cannot be uploaded, Submit is refused with a clear message instead of submitting a stale paper.
6. The badge shows "All answers saved", "Saving…" or "Offline — N answers kept on this computer, will sync".

If local storage is unavailable or full the exam still works; only restart-survival is lost.

## Server side
- `answers_sync {answers:[{questionId, answer, clientSeq}]}` (max 200). Each item gets its own verdict: `stored`, `duplicate`, or an error code. One bad item does not block the rest. A terminal condition (paused, time up, submitted) applies to the remaining items.
- Idempotent: the highest `clientSeq` per question wins; replays and out-of-order delivery change nothing.
- **Takeover:** when a student's attempt connects again, any older connection for that attempt is told `replaced` and closed. The student client treats `replaced` as final (so two windows cannot fight each other).
- **Flood protection:** per connection, more than 40 messages in a second are refused with `rate_limited` (connection stays); more than 200 closes it.
- Heartbeats every 5 s; a connection silent for 20 s (four missed beats) is dropped. One missed beat never disconnects.

## Clocks
The countdown uses the server's `remainingSeconds` plus the browser's **monotonic** timer. Changing the computer's date or time during the exam does not change the countdown. All deadlines are decided on the teacher's computer.

## Verified
- 14 real-socket tests (batches, replay, out-of-order, pause, after-submit, malformed/oversized, takeover, 15-round reconnect storm, burst throttling, flood close, connection vanishing mid-exam).
- 15 frontend tests of the queue and store against a protocol-faithful fake server, including restart survival, edits made mid-upload, pause retry, refused submit while unsent.
- Real-server interop (`PROCTORLAN_INTEROP=1 npx vitest run tests/interop.test.ts`): answers queued on disk before a simulated crash upload after rejoining and are graded.

## Not verified / known limits
- Behaviour on real flaky Wi-Fi and on real student machines.
- Queue is stored in the app's local storage, which is not encrypted. It only holds the student's own typed answers.
- If a student wipes app data AND loses the saved token they must be removed by the teacher to rejoin.
