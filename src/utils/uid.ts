export const uid = (): string =>
  typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : `u-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
