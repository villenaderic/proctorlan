import { invoke } from "@tauri-apps/api/core";
import type { ExamFull } from "@/types/exam";
import type { ExportInfo } from "@/types/results";
import type { BackupContents, BackupInfo, BackupOverview } from "@/types/backup";

export const backupApi = {
  overview: () => invoke<BackupOverview>("backup_overview"),
  create: (destDir: string | null) => invoke<BackupInfo>("create_backup", { destDir }),
  remove: (name: string) => invoke<void>("delete_backup", { name }),
  stageNamed: (name: string) => invoke<BackupContents>("stage_restore", { name }),
  stagePath: (path: string) => invoke<BackupContents>("stage_restore", { path }),
  cancelRestore: () => invoke<void>("cancel_restore"),
  setAuto: (enabled: boolean) => invoke<void>("set_auto_backup", { enabled }),
  restart: () => invoke<void>("restart_app"),
  exportExam: (id: string) => invoke<ExportInfo>("export_exam_file", { id }),
  importExam: (path: string) => invoke<ExamFull>("import_exam_file", { path }),
};
