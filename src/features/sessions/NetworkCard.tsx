import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/form";
import { Field } from "@/components/Field";
import { isTauri } from "@/services/tauri";
import { isSessionExpired, toMessage } from "@/services/auth";
import { networkApi } from "@/services/sessions";
import { refreshAuth } from "@/stores/auth";
import type { NetworkInfo } from "@/types/session";

/** Settings → Network: which address and port students connect to. */
export function NetworkCard() {
  const [info, setInfo] = useState<NetworkInfo | null>(null);
  const [ip, setIp] = useState("");
  const [port, setPort] = useState("");
  const [notice, setNotice] = useState<{ kind: "ok" | "error"; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const fail = useCallback((e: unknown) => {
    const text = toMessage(e);
    if (isSessionExpired(text)) refreshAuth();
    setNotice({ kind: "error", text });
  }, []);

  const adopt = useCallback((i: NetworkInfo) => {
    setInfo(i);
    setIp(i.preferredIp ?? "");
    setPort(String(i.configuredPort));
  }, []);

  useEffect(() => { if (isTauri()) networkApi.info().then(adopt).catch(fail); }, [adopt, fail]);

  if (!isTauri()) return null;
  if (!info) return <Card className="p-6"><h2 className="font-medium">Network</h2><p className="text-sm">Loading…</p></Card>;

  const portNum = Number(port);
  const portError = !Number.isInteger(portNum) || portNum < 1024 || portNum > 65535 ? "Use a whole number from 1024 to 65535." : null;
  const dirty = ip !== (info.preferredIp ?? "") || portNum !== info.configuredPort;
  const st = info.status;

  async function save() {
    setBusy(true);
    setNotice(null);
    try {
      const next = await networkApi.update(ip || null, portNum);
      adopt(next);
      setNotice(next.status.running
        ? { kind: "ok", text: `Server restarted on ${next.joinAddress}.` }
        : { kind: "error", text: next.status.error ?? "The server did not start." });
    } catch (e) { fail(e); }
    setBusy(false);
  }

  return (
    <Card className="p-6">
      <div className="mb-4 flex items-center justify-between">
        <h2 className="font-medium">Network</h2>
        <Badge tone={st.running ? "green" : "red"}>{st.running ? "Server running" : "Server stopped"}</Badge>
      </div>
      {st.running && <p className="mb-3 text-sm">Students join at <span className="font-mono font-semibold">{info.joinAddress}</span>{st.discovery ? " (also announced on the network)" : ""}.</p>}
      {st.error && !st.running && <p role="alert" className="mb-3 rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950 dark:text-red-300">{st.error}</p>}
      {st.running && /^127\./.test(st.ip) && (
        <p role="alert" className="mb-3 rounded-md bg-amber-50 p-3 text-sm text-amber-800 dark:bg-amber-950 dark:text-amber-300">
          No local network was found, so only this computer can connect. Connect to Wi-Fi or Ethernet, then press Save to restart.
        </p>
      )}
      <div className="grid max-w-xl gap-4 sm:grid-cols-2">
        <div className="space-y-1.5">
          <label htmlFor="net-if" className="text-sm font-medium">Network address</label>
          <Select id="net-if" value={ip} onChange={(e) => setIp(e.target.value)}>
            <option value="">Automatic (recommended)</option>
            {info.interfaces.map((i) => <option key={i.ip} value={i.ip}>{i.ip} — {i.name}{i.isPrivate ? "" : " (not a typical classroom network)"}</option>)}
          </Select>
        </div>
        <Field label="Port" inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value)} error={portError} hint={`Default ${info.defaultPort}.`} />
      </div>
      {notice && <p role={notice.kind === "error" ? "alert" : "status"} className={`mt-3 rounded-md p-3 text-sm ${notice.kind === "ok" ? "bg-green-50 text-green-800 dark:bg-green-950 dark:text-green-300" : "bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-300"}`}>{notice.text}</p>}
      <div className="mt-4 flex items-center gap-3">
        <Button onClick={save} disabled={!!portError || busy || (!dirty && st.running)}>{busy ? "Applying…" : "Save and restart server"}</Button>
        <p className="text-xs text-slate-500">Changing this is blocked while a session is open.</p>
      </div>
    </Card>
  );
}
