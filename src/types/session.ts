/** Mirrors the Rust session/network models (serde camelCase; statuses are SCREAMING_SNAKE_CASE). */
export type SessionStatus = "CREATED" | "WAITING" | "RUNNING" | "PAUSED" | "ENDED";
export type SessionAction = "start" | "pause" | "resume" | "end";

export interface SessionRow {
  id: string;
  examId: string;
  examTitle: string;
  sessionCode: string;
  status: SessionStatus;
  hostIp: string;
  hostPort: number;
  startedAt: string | null;
  endsAt: string | null;
  pausedAt: string | null;
  pausedTotalSeconds: number;
  createdAt: string;
  endedAt: string | null;
}

export interface SessionSnapshot extends SessionRow {
  questionCount: number;
  passingScore: number;
  joined: number;
  connected: number;
  online: number;
  submitted: number;
  remainingSeconds: number | null;
  serverTime: string;
}

export interface NetInterface { name: string; ip: string; isPrivate: boolean }

export interface ServerStatus {
  running: boolean;
  ip: string;
  port: number;
  discovery: boolean;
  error: string | null;
}

export interface NetworkInfo {
  status: ServerStatus;
  interfaces: NetInterface[];
  preferredIp: string | null;
  configuredPort: number;
  defaultPort: number;
  joinAddress: string | null;
}

export type AttemptStatus = "JOINED" | "IN_PROGRESS" | "SUBMITTED" | "AUTO_SUBMITTED";

export interface RosterRow {
  attemptId: string;
  studentNumber: string;
  name: string;
  status: AttemptStatus;
  joinedAt: string;
  submittedAt: string | null;
  answered: number;
  percentage: number | null;
  passed: boolean | null;
  online: boolean;
}
