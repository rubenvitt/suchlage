// Anmeldeseite für DJI Pilot 2 – aus M0 übernommen. Die Zugangsdaten kommen jetzt in zwei Schritten
// vom Server: erst die Lizenz (ohne sie gibt Pilot 2 die Seriennummer nicht heraus), dann der Zugang.

export class PairingError extends Error {}

function caller(bridge, log) {
  return (name, ...args) => {
    const res = JSON.parse(bridge[name](...args));
    if (res.code === 0) log(`${name}: ok`);
    else log(`${name}: FEHLER ${res.code} ${res.message}`, "error");
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
    log(`Kartenelemente nicht verfügbar (${e.message})`, "warn");
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

/** `onThingConnect`/`onWsConnect` liefern `true`/`false`, je nach Version als Boolean oder Text. */
export function isConnected(state) {
  return String(state) === "true";
}

/**
 * `onLiveStatus` liefert ein Objekt oder JSON-Text, je nach Pilot-2-Version. `text` ist die lesbare
 * Form, `key` ändert sich nur, wenn sich der Zustand ändert (nicht bei jeder neuen Bitrate).
 */
export function liveStatus(raw) {
  let value = raw;
  if (typeof raw === "string") {
    try {
      value = JSON.parse(raw);
    } catch {
      return { text: raw, key: raw };
    }
  }
  if (value === null || typeof value !== "object") return { text: String(value), key: String(value) };
  const text = JSON.stringify(value);
  const state = value.status ?? value.liveStatus ?? value.state ?? value.code;
  return { text, key: state === undefined ? text : JSON.stringify(state) };
}

const FLUSH_MS = 2_000;
const BATCH = 50;
const KEEP = 200;

/**
 * Schickt Protokollzeilen gebündelt an `/pilot/api/log`, damit die Systemseite sie live zeigt.
 * Fehler gehen sofort raus. Ohne Netz bleiben bis zu 200 Zeilen für den nächsten Versuch.
 */
export function remoteLog(code, post, { now = Date.now, schedule = setTimeout } = {}) {
  let queue = [];
  let timer = null;
  let sending = false;
  const flush = async () => {
    timer = null;
    if (sending || queue.length === 0) return;
    sending = true;
    const lines = queue.slice(0, BATCH);
    try {
      // Hat der Server geantwortet, hilft Wiederholen nicht (z. B. Code ungültig): Zeilen fallen weg.
      await post("/pilot/api/log", { code, lines });
      queue = queue.slice(lines.length);
    } catch {
      // kein Netz: dieselben Zeilen beim nächsten Versuch
    }
    sending = false;
    if (queue.length > 0 && timer === null) timer = schedule(flush, FLUSH_MS);
  };
  const add = (text, level = "info") => {
    queue.push({ at_ms: now(), level, text });
    if (queue.length > KEEP) queue = queue.slice(-KEEP);
    if (level === "error") return flush();
    if (timer === null) timer = schedule(flush, FLUSH_MS);
  };
  return { add, flush };
}
