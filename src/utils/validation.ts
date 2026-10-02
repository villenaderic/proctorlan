import { AUTH_LIMITS } from "@/config";

/** Instant-feedback validators. The Rust backend repeats every check authoritatively. */
export function usernameError(value: string): string | null {
  const u = value.trim();
  if (u.length < AUTH_LIMITS.usernameMin || u.length > AUTH_LIMITS.usernameMax)
    return `Username must be ${AUTH_LIMITS.usernameMin}–${AUTH_LIMITS.usernameMax} characters.`;
  if (!/^[A-Za-z0-9._-]+$/.test(u)) return "Use only letters, numbers, dots, dashes and underscores.";
  return null;
}

export function passwordError(password: string, username: string): string | null {
  const len = [...password].length;
  if (len < AUTH_LIMITS.minPasswordLength) return `Password must be at least ${AUTH_LIMITS.minPasswordLength} characters.`;
  if (len > AUTH_LIMITS.maxPasswordLength) return `Password must be at most ${AUTH_LIMITS.maxPasswordLength} characters.`;
  if (password.toLowerCase() === username.trim().toLowerCase()) return "Password must not be the same as the username.";
  return null;
}
