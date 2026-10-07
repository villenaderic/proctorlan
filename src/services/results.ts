import { invoke } from "@tauri-apps/api/core";
import type { AttemptDetail, ExportInfo, ResultSessionRow, SessionResults, StudentAttemptRow, StudentSummary } from "@/types/results";

export const resultsApi = {
  sessions: () => invoke<ResultSessionRow[]>("list_result_sessions"),
  session: (id: string) => invoke<SessionResults>("get_session_results", { id }),
  attempt: (attemptId: string) => invoke<AttemptDetail>("get_attempt_detail", { attemptId }),
  exportCsv: (id: string) => invoke<ExportInfo>("export_session_csv", { id }),
  students: () => invoke<StudentSummary[]>("list_students"),
  studentHistory: (studentId: string) => invoke<StudentAttemptRow[]>("get_student_history", { studentId }),
};
