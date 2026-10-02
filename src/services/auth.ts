import { invoke } from "@tauri-apps/api/core";
import type { AuthStatus, User } from "@/types/auth";
import type { Stats } from "@/types/app";

/** Tauri rejects with the user-safe string produced by the Rust AppError serializer. */
export function toMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Something went wrong. Please try again.";
}

export const isSessionExpired = (msg: string) => msg.includes("sign in again");

export const authApi = {
  status: () => invoke<AuthStatus>("auth_status"),
  setupAdmin: (username: string, password: string, displayName: string) =>
    invoke<User>("setup_admin", { username, password, displayName }),
  login: (username: string, password: string) => invoke<User>("login", { username, password }),
  logout: () => invoke<void>("logout"),
  changePassword: (currentPassword: string, newPassword: string) =>
    invoke<void>("change_password", { currentPassword, newPassword }),
  updateDisplayName: (displayName: string) => invoke<User>("update_display_name", { displayName }),
  dashboardStats: () => invoke<Stats>("dashboard_stats"),
};
