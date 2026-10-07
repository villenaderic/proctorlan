# ProctorLAN — Teacher Guide (grows with each phase)

## First launch
Choose **I'm a Teacher → Continue** and create the administrator account. There is no online password recovery, so keep the password safe.

## Creating an exam (Exams → Create exam)
The builder has four steps. A badge on a step shows how many things still need fixing.

1. **Basic info** — title (required), description, instructions shown to students, duration in minutes, passing percentage. Total points is the sum of all question points.
2. **Questions** — add questions of four types:
   - **Multiple choice:** two or more choices, exactly one correct.
   - **True / False:** choose which is correct.
   - **Multiple select:** two or more choices, tick every correct one.
   - **Identification:** one or more accepted answers; capitals and extra spaces are ignored when grading.

   Each question has points, a *required* flag and an optional explanation. Use the arrows to reorder, the copy icon to duplicate, and the bin to delete. Changing a question's type keeps what still makes sense (for example, correct choices become accepted answers).
3. **Settings** — randomize question order, randomize choice order, allow answer review (when off, students cannot go back), auto-submit at timeout, show results after submission.
4. **Preview** — the student experience: navigator, timer, mark-for-review, review list, submit dialog. Nothing is saved or graded in a preview.

### Draft vs. ready
You can save an unfinished exam as a draft. Before an exam can be **activated** it must be complete: at least one question, every question has text and a valid correct answer, choices are filled in and distinct. Click the power icon in the exam list to activate; if something is missing you will be told what.

### Locked exams
Once an exam has been used in a session it can no longer be edited or deleted, so recorded results stay accurate. Use **Duplicate** to make a changed copy.

## Watching a live exam (Phase 9)

On the session page, the **Proctoring** column shows how often each student left the exam window or lost connection. The **Live events** list shows what happened and when; the clock button on a row opens that student's timeline. These are signals, not proof — see `docs/proctoring.md` for what they can and cannot tell you.

## Results and export (Phase 10)

After a session, open **Results**, pick the session, and review scores, charts and the question analysis. Click a student to see their answers next to the correct ones. **Export CSV** saves a spreadsheet file and shows where it was saved. See `docs/results.md`.

## Backups and sharing exams (Phase 11)

Open **Backups** and press **Create backup now** before an important exam, and keep a copy on a USB drive. Automatic backups are on by default. To move or share a single exam, use the download icon in **Exams** and **Import exam** on the other computer. See `docs/backup.md`.
