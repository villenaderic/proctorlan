/** What the server sends a student. Never contains answer keys or explanations. */
export type QuestionKind = "multiple_choice" | "true_false" | "multiple_select" | "identification";

export interface PaperChoice { id: string; text: string }

export interface PaperQuestion {
  id: string;
  text: string;
  type: QuestionKind;
  points: number;
  required: boolean;
  /** Empty for identification questions. */
  choices: PaperChoice[];
}

/** string = single choice id or typed text; string[] = multiple select. */
export type AnswerValue = string | string[];

export interface Paper {
  questions: PaperQuestion[];
  answers: Record<string, AnswerValue>;
  allowReview: boolean;
  totalPoints: number;
}

export interface ResultSummary { score: number; totalPoints: number; percentage: number; passed: boolean }
