# Proctoring (Phase 9)

ProctorLAN records **signals** that help a teacher notice something worth a conversation. It does not decide anything, never changes a grade, and never blocks a student.

## What is recorded

| Event | Who reports it | When |
|---|---|---|
| `FOCUS_LOST` | Student app | The exam window was hidden or lost focus for more than 300 ms while the exam was running |
| `FOCUS_RESTORED` | Student app | The window came back; carries how long the student was away |
| `DISCONNECTED` | Teacher server | A student mid-exam stayed offline longer than the grace period (20 s) |
| `RECONNECTED` | Teacher server | That student came back; carries how long they were offline |
| `SUBMISSION` | Teacher server | Submitted by the student, or automatically when the teacher ended the session |
| `TIMEOUT` | Teacher server | Auto-submitted because time ran out |

Only the two focus events can come from a student. The server refuses anything else (`invalid_event`), so a student cannot forge a disconnect or a submission. Events are recorded only for an attempt that is in progress; focus loss counts only while the exam is running (not while paused). Duplicates and out-of-order events are ignored, durations are clamped, and each attempt is capped at 1,000 events.

Short interruptions are filtered on purpose: a flicker under 300 ms (a notification, a fast Alt-Tab) leaves no trace, and a connection drop shorter than the grace period leaves no trace. A student who is replaced by their own second window is not marked disconnected.

## Teacher view

The live session page shows per-student badges (“Left window ×3 (45 s)”, “Disconnected ×1”), a newest-first live feed, and a per-student timeline. Students who are offline mid-exam or have more flags sort to the top.

## Student transparency

The exam screen always says that leaving the window, switching tabs or losing connection is reported, and shows the student “You were away for N s. Your teacher can see this.” after they return. Nothing is watched before the exam opens or after submission. Focus events are kept in memory and resent after a reconnect; if the app is closed while offline they are lost (the server-side disconnect record remains).

## What it cannot do (please read)

- It does **not** see other devices, phones, notes, books, a second monitor, or another person in the room.
- It does **not** detect screenshots, virtual machines, remote-desktop tools, or OS-level overlays.
- It is **not** a kiosk or lockdown mode: students are not prevented from leaving the window.
- Focus loss has innocent causes (a system pop-up, a mis-click, a dialog from another app). Treat counts as a prompt to look, not as evidence.
- Disconnect events appear after the grace period, not instantly. A student who fails network-wise looks the same as one who disconnected on purpose.
- A modified student client could suppress focus reports. Server-side events (disconnects, submissions) cannot be forged this way.

Use ProctorLAN alongside normal invigilation, not instead of it.

## Wire format

Client → server: `{"type":"proctor_event","payload":{"type":"FOCUS_LOST"}}` or `{"type":"FOCUS_RESTORED","lostForMs":12000}`.
Server → client: `proctor_ack {recorded: bool}` or `error {code:"invalid_event"}`.
Teacher command: `list_session_events(id)` returns up to 500 events, newest first.
