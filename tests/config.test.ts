import { describe, expect, it } from "vitest";
import { NETWORK_DEFAULTS } from "../src/config";

describe("config", () => {
  it("uses a non-privileged default port", () => {
    expect(NETWORK_DEFAULTS.defaultPort).toBeGreaterThan(1024);
    expect(NETWORK_DEFAULTS.defaultPort).toBeLessThan(65536);
  });
  it("reconnects faster than the heartbeat declares a client dead", () => {
    expect(NETWORK_DEFAULTS.reconnectIntervalMs).toBeLessThanOrEqual(NETWORK_DEFAULTS.heartbeatIntervalMs);
  });
});
