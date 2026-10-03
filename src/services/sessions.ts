import { invoke } from "@tauri-apps/api/core";
import type { NetworkInfo, SessionAction, SessionRow, SessionSnapshot } from "@/types/session";

export const sessionsApi = {
  list: () => invoke<SessionRow[]>("list_sessions"),
  snapshot: (id: string) => invoke<SessionSnapshot>("get_session_snapshot", { id }),
  create: (examId: string) => invoke<SessionSnapshot>("create_session", { examId }),
  act: (id: string, action: SessionAction) => invoke<SessionSnapshot>("session_action", { id, action }),
};

export const networkApi = {
  info: () => invoke<NetworkInfo>("network_info"),
  update: (ip: string | null, port: number) => invoke<NetworkInfo>("update_network", { ip, port }),
};
