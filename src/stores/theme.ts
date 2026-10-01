import { create } from "zustand";
import { APP } from "@/config";

export type Theme = "light" | "dark";

const initial = (): Theme => {
  try {
    const saved = localStorage.getItem(APP.themeStorageKey);
    if (saved === "light" || saved === "dark") return saved;
  } catch { /* storage unavailable */ }
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
};

export const applyTheme = (t: Theme) => document.documentElement.classList.toggle("dark", t === "dark");

interface ThemeState { theme: Theme; toggle: () => void }

export const useTheme = create<ThemeState>((set, get) => ({
  theme: initial(),
  toggle: () => {
    const next: Theme = get().theme === "dark" ? "light" : "dark";
    try { localStorage.setItem(APP.themeStorageKey, next); } catch { /* ignore */ }
    applyTheme(next);
    set({ theme: next });
  },
}));
