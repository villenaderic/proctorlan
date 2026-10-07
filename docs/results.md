# Results (Phase 10)

Everything here is **read-only** over graded data: viewing or exporting results never changes a score.

## Where to find it
- **Results** lists every session that had at least one student, with submitted count, average and pass count. Open one for the full page.
- The session page shows the average, median, pass rate, highest/lowest, spread (standard deviation) and average time, a **score distribution** chart (10-point bands) and a **percent-correct-by-question** chart, a sortable student table, and a **question analysis** table (how often each option was picked; the most common wrong typed answers for identification questions).
- Click a student for their **answer review**: what they answered, the correct answer, points per question, the explanation, and their proctoring timeline.
- **Students** lists everyone who ever joined, with an attempt history and average.
- The ended-session page on **Sessions** has a "View results" link.

## What counts
Statistics use only **submitted** attempts (submitted by the student, auto-submitted at time-up, or finished by the teacher ending the session). Students who joined but never finished are reported as "did not finish" and are left out of averages and charts. An unanswered question counts as incorrect in the per-question percentages.

## CSV export
**Export CSV** writes a file into the `exports` folder inside the app's data directory and shows its full path. One row per student with identity, status, score, percentage, result, start/submit time, minutes taken, questions answered, proctoring counts, and one column per question holding the points earned.

- UTF-8 with a byte-order mark and CRLF line endings, so Excel opens accents correctly.
- Names, IDs and other student-typed text are protected against spreadsheet formula injection: a cell beginning with `=`, `+`, `-`, `@` or a tab is prefixed with `'`.
- File names are built from the exam title (letters and digits only), session code and a timestamp.

## Limits and honesty
- Grading is automatic and all-or-nothing per question; there is no manual regrade yet.
- The time-taken figure runs from the first time the student opened the exam to submission.
- Proctoring counts in the table are signals; see `proctoring.md`.
- Backup/restore and other export formats arrive in Phase 11.
