export type BackupKind = "manual" | "auto" | "prerestore" | "corrupt";

export interface BackupInfo { fileName: string; kind: BackupKind; sizeBytes: number; createdAt: string; path: string }
export interface BackupContents { schemaVersion: number; exams: number; sessions: number; students: number; attempts: number }
export interface BackupOverview { directory: string; backups: BackupInfo[]; autoEnabled: boolean; pendingRestore: BackupContents | null }
