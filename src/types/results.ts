/** Mirrors the Rust results models (serde camelCase). */
import type { AttemptStatus, ProctorEventType } from "./session";
import type { QuestionType } from "./exam";

export interface ResultRow {
  attemptId: string;
  studentNumber: string;
  name: string;
  status: AttemptStatus;
  startedAt: string | null;
  submittedAt: string | null;
  score: number | null;
  totalPoints: number | null;
  percentage: number | null;
  passed: boolean | null;
  answered: number;
  timeTakenSeconds: number | null;
  focusLostCount: number;
  focusLostMs: number;
  disconnectCount: number;
}

export interface ResultSessionRow {
  sessionId: string;
  examTitle: string;
  sessionCode: string;
  status: string;
  createdAt: string;
  endedAt: string | null;
  joined: number;
  submitted: number;
  passed: number;
  averagePercentage: number | null;
}

export interface ScoreStats {
  submitted: number;
  notSubmitted: number;
  passed: number;
  failed: number;
  passRate: number;
  mean: number;
  median: number;
  highest: number;
  lowest: number;
  stdDev: number;
  averageTimeSeconds: number | null;
}

export interface Bucket { label: string; from: number; to: number; count: number }
export interface OptionStat { text: string; isCorrect: boolean; picked: number }
export interface WrongAnswer { text: string; count: number }

export interface QuestionStat {
  questionId: string;
  position: number;
  text: string;
  questionType: QuestionType;
  points: number;
  attempts: number;
  answered: number;
  correct: number;
  percentCorrect: number;
  options: OptionStat[];
  commonWrong: WrongAnswer[];
}

export interface SessionResults {
  sessionId: string;
  examTitle: string;
  sessionCode: string;
  status: string;
  createdAt: string;
  endedAt: string | null;
  passingScore: number;
  totalPoints: number;
  questionCount: number;
  rows: ResultRow[];
  stats: ScoreStats;
  distribution: Bucket[];
  questions: QuestionStat[];
}

export interface AnswerDetail {
  position: number;
  questionId: string;
  text: string;
  questionType: QuestionType;
  points: number;
  pointsAwarded: number;
  answered: boolean;
  isCorrect: boolean;
  given: string[];
  correct: string[];
  explanation: string | null;
}

export interface AttemptDetail {
  attemptId: string;
  sessionId: string;
  examTitle: string;
  studentNumber: string;
  name: string;
  status: AttemptStatus;
  startedAt: string | null;
  submittedAt: string | null;
  score: number | null;
  totalPoints: number | null;
  percentage: number | null;
  passed: boolean | null;
  passingScore: number;
  answers: AnswerDetail[];
  events: Array<{ id: string; eventType: ProctorEventType; description: string; createdAt: string }>;
}

export interface StudentSummary {
  id: string;
  studentNumber: string;
  name: string;
  attempts: number;
  averagePercentage: number | null;
  lastAttemptAt: string | null;
}

export interface StudentAttemptRow {
  attemptId: string;
  sessionId: string;
  examTitle: string;
  status: AttemptStatus;
  submittedAt: string | null;
  score: number | null;
  totalPoints: number | null;
  percentage: number | null;
  passed: boolean | null;
  createdAt: string;
}

export interface ExportInfo { path: string; fileName: string; rows: number }
