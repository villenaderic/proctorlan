export type QuestionType = "multiple_choice" | "true_false" | "multiple_select" | "identification";
export type ExamStatus = "active" | "inactive";

export const QUESTION_TYPE_LABELS: Record<QuestionType, string> = {
  multiple_choice: "Multiple choice",
  true_false: "True / False",
  multiple_select: "Multiple select",
  identification: "Identification",
};

/** Mirrors the Rust models (serde camelCase). */
export interface Exam {
  id: string;
  title: string;
  description: string;
  instructions: string;
  durationMinutes: number;
  passingScore: number;
  randomizeQuestions: boolean;
  randomizeChoices: boolean;
  allowReview: boolean;
  autoSubmit: boolean;
  showResults: boolean;
  status: ExamStatus;
  createdAt: string;
  updatedAt: string;
}

export interface ExamSummary extends Exam {
  questionCount: number;
  totalPoints: number;
  sessionCount: number;
}

export interface Choice { id: string; questionId: string; choiceText: string; isCorrect: boolean; sortOrder: number }

export interface Question {
  id: string;
  questionText: string;
  questionType: QuestionType;
  points: number;
  sortOrder: number;
  required: boolean;
  explanation: string | null;
  choices: Choice[];
}

export interface ExamFull extends Exam {
  questions: Question[];
  sessionCount: number;
}

export interface ValidationIssue { path: string; message: string }

/** Client-side editable shapes. `uid` is a stable React key and never sent to the backend. */
export interface DraftChoice { uid: string; choiceText: string; isCorrect: boolean }

export interface DraftQuestion {
  uid: string;
  questionText: string;
  questionType: QuestionType;
  points: number;
  required: boolean;
  explanation: string;
  choices: DraftChoice[];
}

export interface ExamDraft {
  title: string;
  description: string;
  instructions: string;
  durationMinutes: number;
  passingScore: number;
  randomizeQuestions: boolean;
  randomizeChoices: boolean;
  allowReview: boolean;
  autoSubmit: boolean;
  showResults: boolean;
  questions: DraftQuestion[];
}
