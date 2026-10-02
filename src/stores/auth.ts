import { create } from "zustand";
import { authApi } from "@/services/auth";
import { isTauri } from "@/services/tauri";
import type { User } from "@/types/auth";

export type AuthPhase = "loading" | "unavailable" | "setup" | "login" | "ready";

interface AuthState {
  phase: AuthPhase;
  user: User | null;
  refresh: () => Promise<void>;
  setupAdmin: (username: string, password: string, displayName: string) => Promise<void>;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  setUser: (user: User) => void;
}

export const useAuth = create<AuthState>((set) => ({
  phase: "loading",
  user: null,

  async refresh() {
    if (!isTauri()) return set({ phase: "unavailable", user: null });
    try {
      const s = await authApi.status();
      set({ user: s.user, phase: s.setupRequired ? "setup" : s.user ? "ready" : "login" });
    } catch {
      set({ phase: "unavailable", user: null });
    }
  },

  // Action errors propagate to the calling form so it can show the message.
  async setupAdmin(username, password, displayName) {
    set({ user: await authApi.setupAdmin(username, password, displayName), phase: "ready" });
  },
  async login(username, password) {
    set({ user: await authApi.login(username, password), phase: "ready" });
  },
  async logout() {
    await authApi.logout().catch(() => undefined);
    set({ user: null, phase: "login" });
  },
  setUser: (user) => set({ user }),
}));

export const refreshAuth = () => useAuth.getState().refresh();
