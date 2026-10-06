import { useEffect } from "react";
import { FocusTracker, isAway } from "./focus";
import { useStudent } from "@/stores/student";

/** Watches window focus and tab visibility while `active`, reporting to the teacher through the student store. */
export function useFocusMonitor(active: boolean, onReturn: (awayMs: number) => void) {
  useEffect(() => {
    if (!active) return;
    const tracker = new FocusTracker({
      report: (r) => useStudent.getState().reportFocus(r.type, r.lostForMs),
      onReturn,
    });
    const check = () => tracker.setAway(isAway());
    window.addEventListener("blur", check);
    window.addEventListener("focus", check);
    document.addEventListener("visibilitychange", check);
    check(); // the window may already be unfocused when the exam opens
    return () => {
      window.removeEventListener("blur", check);
      window.removeEventListener("focus", check);
      document.removeEventListener("visibilitychange", check);
      tracker.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active]);
}
