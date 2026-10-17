// Anmeldeseite für DJI Pilot 2 – aus M0 übernommen. Die Zugangsdaten kommen jetzt in zwei Schritten
// vom Server: erst die Lizenz (ohne sie gibt Pilot 2 die Seriennummer nicht heraus), dann der Zugang.

export class PairingError extends Error {}

function caller(bridge, log) {
  return (name, ...args) => {
    const res = JSON.parse(bridge[name](...args));
    log(`${name}: ${res.code === 0 ? "ok" : `FEHLER ${res.code} ${res.message}`}`);
    if (res.code !== 0) throw new Error(name);
    return res.data;
  };
}

/** Seriennummer der Fernsteuerung: `data` aus der JSON-Hülle, sonst der Rohwert. */
export function readRcSn(bridge) {
  const raw = bridge.platformGetRemoteControllerSN();
  let sn = raw;
  let envelope = null;
  try {
    envelope = JSON.parse(raw);
  } catch {
    // kein JSON: Rohwert
  }
  if (envelope && typeof envelope === "object" && "code" in envelope) {
    if (envelope.code !== 0) throw new Error("platformGetRemoteControllerSN");
    sn = envelope.data;
  }
  sn = String(sn ?? "").trim();
  if (!/^[A-Za-z0-9]{1,64}$/.test(sn)) throw new Error("Seriennummer der Fernsteuerung nicht lesbar");
  return sn;
}

export function verify(lic, bridge, log) {
  const call = caller(bridge, log);
  call("platformVerifyLicense", lic.appId, lic.appKey, lic.license);
  if (!call("platformIsVerified")) throw new Error("platformIsVerified");
}

export function connect(cfg, bridge, log) {
  const call = caller(bridge, log);
  call("platformSetWorkspaceId", cfg.workspaceId);
  call("platformSetInformation", "Suchlage", "DRK I&K", cfg.rcLabel);
  call("platformLoadComponent", "thing", JSON.stringify({
    host: cfg.mqttHost, username: cfg.mqttUser, password: cfg.mqttPass, connectCallback: "onThingConnect",
  }));
  call("platformLoadComponent", "liveshare", JSON.stringify({
    videoPublishType: "video-demand-aux-manual", statusCallback: "onLiveStatus",
  }));
  connectMap(cfg, bridge, log);
}

/**
 * Kartenelemente (M2): erst `api` und `ws`, dann `map` (`30.jsbridge.md`). Scheitert ein Modul,
 * laufen Telemetrie und Livebild trotzdem; das Protokoll nennt den Grund. `true`, wenn alles geladen ist.
 */
export function connectMap(cfg, bridge, log) {
  if (!cfg.apiToken) return false; // Server ohne Kartenelemente
  const call = caller(bridge, log);
  try {
    call("platformLoadComponent", "api", JSON.stringify({ host: cfg.apiHost, token: cfg.apiToken }));
    call("platformLoadComponent", "ws", JSON.stringify({
      host: cfg.wsHost, token: cfg.apiToken, connectCallback: "onWsConnect",
    }));
    call("platformLoadComponent", "map", JSON.stringify({ userName: cfg.rcLabel, elementPreName: "Pilot" }));
    return true;
  } catch (e) {
    log(`Kartenelemente nicht verfügbar (${e.message})`);
    return false;
  }
}

/** Ganzer Ablauf. `post(path, body)` liefert `{ ok, body }`. Gibt die zusammengeführte Konfiguration zurück. */
export async function pair(code, bridge, log, post) {
  const lic = await post("/pilot/api/license", { code });
  if (!lic.ok) throw new PairingError(lic.body?.error ?? "Kopplung abgelehnt.");
  verify(lic.body, bridge, log);
  const rcSn = readRcSn(bridge);
  const conn = await post("/pilot/api/connect", { code, rc_sn: rcSn });
  if (!conn.ok) throw new PairingError(conn.body?.error ?? "Kopplung abgelehnt.");
  const cfg = { ...lic.body, ...conn.body };
  connect(cfg, bridge, log);
  return cfg;
}

export function startManualLive(cfg, bridge, log) {
  const call = caller(bridge, log);
  call("liveshareSetConfig", 2, JSON.stringify({ url: cfg.rtmpUrl }));
  call("liveshareStartLive");
}
