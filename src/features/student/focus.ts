/**
 * Window-focus tracking for the exam screen.
 *
 * This only ever reports two facts: "the exam window lost focus" and "it came back after N ms".
 * It is a signal for the teacher, never a penalty, and it cannot see anything outside this window.
 */
export type FocusKind = "FOCUS_LOST" | "FOCUS_RESTORED";

export interface FocusReport { type: FocusKind; lostForMs?: number }

export interface FocusTrackerOptions {
  report(r: FocusReport): void;
  /** Called on return, only if the absence was reported (so the student sees exactly what the teacher sees). */
  onReturn?(awayMs: number): void;
  /** Absences shorter than this are ignored (a notification popping up, a tooltip, a fast Alt-Tab flicker). */
  debounceMs?: number;
  now?: () => number;
}

export const FOCUS_DEBOUNCE_MS = 300;

export class FocusTracker {
  private awaySince: number | null = null;
  private reported = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private readonly debounce: number;
  private readonly now: () => number;

  constructor(private readonly opts: FocusTrackerOptions) {
    this.debounce = opts.debounceMs ?? FOCUS_DEBOUNCE_MS;
    this.now = opts.now ?? (() => performance.now());
  }

  /** Feed the combined state: true when the window is hidden or unfocused. Repeats are harmless. */
  setAway(away: boolean) {
    if (away) {
      if (this.awaySince !== null) return;
      this.awaySince = this.now();
      this.timer = setTimeout(() => {
        this.timer = undefined;
        this.reported = true;
        this.opts.report({ type: "FOCUS_LOST" });
      }, this.debounce);
      return;
    }
    if (this.awaySince === null) return;
    const ms = Math.max(0, Math.round(this.now() - this.awaySince));
    const wasReported = this.reported;
    this.awaySince = null;
    this.reported = false;
    if (this.timer) { clearTimeout(this.timer); this.timer = undefined; }
    if (!wasReported) return; // a blip shorter than the debounce leaves no trace
    this.opts.report({ type: "FOCUS_RESTORED", lostForMs: ms });
    this.opts.onReturn?.(ms);
  }

  /** Stops timers. If the student is mid-absence the restore is sent so the teacher is not left with an open "lost". */
  dispose() {
    if (this.timer) { clearTimeout(this.timer); this.timer = undefined; }
    if (this.reported && this.awaySince !== null) {
      this.opts.report({ type: "FOCUS_RESTORED", lostForMs: Math.max(0, Math.round(this.now() - this.awaySince)) });
    }
    this.awaySince = null;
    this.reported = false;
  }
}

/** Is the exam window currently out of the student's view? */
export function isAway(doc: Pick<Document, "hidden" | "hasFocus"> = document): boolean {
  return doc.hidden || !doc.hasFocus();
}
