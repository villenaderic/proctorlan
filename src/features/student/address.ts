import { NETWORK_DEFAULTS } from "@/config";

export interface HostAddress { host: string; port: number }

const IPV4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/;
const HOSTNAME = /^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*$/;

/**
 * Parses what a student types: "192.168.1.20", "192.168.1.20:38123" or "teacher-pc.local:38123".
 * Deliberately strict (no scheme, path, credentials) so the value can safely build a ws:// URL.
 */
export function parseHostAddress(input: string): HostAddress | { error: string } {
  const text = input.trim();
  if (!text) return { error: "Enter the address your teacher shows on screen." };
  const m = /^([^:]+)(?::(\d{1,5}))?$/.exec(text);
  if (!m) return { error: "Use the form 192.168.1.20:38123." };
  const host = m[1];
  const port = m[2] === undefined ? NETWORK_DEFAULTS.defaultPort : Number(m[2]);
  if (port < 1 || port > 65535) return { error: "The port must be between 1 and 65535." };
  const v4 = IPV4.exec(host);
  if (v4) {
    if (v4.slice(1).some((p) => Number(p) > 255)) return { error: "That IP address is not valid." };
  } else if (!HOSTNAME.test(host) || host.length > 253) {
    return { error: "That address is not valid. Copy it exactly as your teacher shows it." };
  }
  return { host, port };
}

export const isAddressError = (r: HostAddress | { error: string }): r is { error: string } => "error" in r;

/** Session codes use no look-alike characters (no 0/O/1/I/L); normalise what students type. */
export function normalizeCode(input: string): string {
  return input.toUpperCase().replace(/\s+/g, "");
}
