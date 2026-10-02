import { invoke } from "@tauri-apps/api/core";
import type { ExamFull, ExamSummary, ValidationIssue } from "@/types/exam";

/** Payload accepted by the Rust `NewExam` type. */
export type ExamPayload = Record<string, unknown>;

export const examsApi = {
  list: () => invoke<ExamSummary[]>("list_exams"),
  get: (id: string) => invoke<ExamFull>("get_exam", { id }),
  validate: (exam: ExamPayload) => invoke<ValidationIssue[]>("validate_exam", { exam }),
  create: (exam: ExamPayload) => invoke<ExamFull>("create_exam", { exam }),
  update: (id: string, exam: ExamPayload) => invoke<ExamFull>("update_exam", { id, exam }),
  setActive: (id: string, active: boolean) => invoke<ExamFull>("set_exam_active", { id, active }),
  duplicate: (id: string) => invoke<ExamFull>("duplicate_exam", { id }),
  remove: (id: string) => invoke<void>("delete_exam", { id }),
};
