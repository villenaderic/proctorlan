/** Central frontend configuration. Keep tunables here, not scattered in components. */
export const APP = {
  name: "ProctorLAN",
  tagline: "Run classroom exams on a LAN with no internet.",
  themeStorageKey: "proctorlan.theme",
} as const;

export const NETWORK_DEFAULTS = {
  /** Mirrors src-tauri/src/config.rs — host default port. */
  defaultPort: 38123,
  heartbeatIntervalMs: 5_000,
  reconnectIntervalMs: 3_000,
  sessionCodeLength: 5,
} as const;

/** Mirrors src-tauri/src/config.rs; the backend is the authoritative validator. */
export const AUTH_LIMITS = {
  minPasswordLength: 8,
  maxPasswordLength: 128,
  usernameMin: 3,
  usernameMax: 32,
  displayNameMax: 64,
} as const;
