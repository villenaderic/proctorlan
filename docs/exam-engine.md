# Exam engine (Phase 7)

The teacher's computer decides everything that matters: what a student may see, whether an answer is still accepted, and the score. The student app only displays and sends.

## Messages (after `hello` + `join`)
| Client → server | Reply | Notes |
|---|---|---|
| `start_exam` | `exam_paper` | Only while the session is RUNNING. Marks the attempt IN_PROGRESS. Returns the questions plus answers already saved. |
| `answer {questionId, answer, clientSeq}` | `answer_ack {stored}` | `stored:false` = duplicate or older retry, ignored. |
| `submit` | `submitted {result?}` | Idempotent. |

Pushes: `time_up`, `submitted {auto:true}` (auto-submit), plus the Phase 5 `session_*` messages.
Error codes: `exam_not_running`, `exam_not_started`, `already_submitted`, `time_up`, `invalid_answer`, `join_required`.

## What students never receive
Answer keys, `isCorrect`, explanations, and the accepted answers of identification questions (those question types are sent with no choices). Tested by scanning the whole paper for leaks.

## Answer formats
- multiple choice / true-false: a choice id string
- multiple select: an array of choice ids
- identification: text (max 500 characters)

## Grading rules
- Single answer types: all or nothing.
- Multiple select: **all or nothing**: every correct choice and no wrong one. No partial credit.
- Identification: ignores upper/lower case, extra spaces; spelling must match an accepted answer.
- Unanswered = 0. Score, total, percentage and pass mark are computed on the server when the attempt closes, in one transaction.
- Students see their score only if the exam has **Show results** on.

## Timer rules
- `ends_at` is set when the teacher starts. Pause freezes it; Resume pushes it back by the paused time.
- Answers are accepted up to **3 seconds** after the deadline (network delay allowance), then refused with `time_up`.
- A background check runs every second:
  - **Auto-submit on:** everyone still working is submitted and graded (`AUTO_SUBMITTED`) and told.
  - **Auto-submit off:** answers lock, but students can still press Submit; the teacher ending the session submits the rest.
- Teacher **End session** always grades everyone who started. Students who joined but never opened the exam stay "Joined" with no score.
- Paused sessions never expire, even if the stored deadline is in the past.

## Randomisation
When enabled, question order and (for multiple choice / multiple select) choice order are shuffled **per student**, deterministically from their attempt id: reconnecting shows the same order. True/False keeps its order.

## Review off
"Allow review" off hides the navigator and Previous button. This is a student-screen rule; the server does not block re-answering the current question.

## Student screen
One question at a time, navigator, progress bar, HH:MM:SS server clock (amber at 5 min, red at 1 min, spoken announcements for screen readers), "All answers saved / Saving… / Offline" badge, submit dialog that lists unanswered questions, paused and time-up banners.

## Reliability
Answers get increasing sequence numbers. Unacknowledged answers stay queued and are resent after a reconnect or resume. **Not yet built (Phase 8):** persisting that queue across an app restart, and extra sync tests.

## Verified
- 16 real-socket engine tests, grading unit tests, TypeScript store run against the real server (`PROCTORLAN_INTEROP=1 npx vitest run tests/interop.test.ts`).
- **Not verified:** the exam screen visually; behaviour on real student machines and networks.
