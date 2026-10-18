import { test } from "node:test";
import assert from "node:assert/strict";
import { MEDIA_PARAMS, PairingError, connect, connectMap, connectMedia, isConnected, liveStatus, pair, readRcSn, remoteLog, startManualLive, verify } from "./pilot.js";

const lic = { appId: "1", appKey: "k", license: "l", workspaceId: "768d4a2e-1f34-477c-9fcc-872285e895f6", rcLabel: "RC Plus 1" };
const conn = { mqttHost: "tcp://127.0.0.1:1883", mqttUser: "rc-RCSN01", mqttPass: "p", rtmpUrl: "rtmp://127.0.0.1:1935/live/RCSN01?user=rc-RCSN01&pass=x" };
const map = { apiHost: "https://suchlage.test", wsHost: "wss://suchlage.test/pilot/ws", apiToken: "t0k" };

/** JSBridge-Attrappe: jede Antwort ist eine JSON-Hülle wie in Pilot 2. `failsOn(name, args)` wählt Fehlschläge je Aufruf. */
function fakeBridge({ failing = {}, data = {}, raw = {}, failsOn = () => false } = {}) {
  const calls = [];
  const handler = { get: (_, name) => (...args) => {
    calls.push([name, ...args]);
    if (name in raw) return raw[name];
    if (failing[name] || failsOn(name, args)) return JSON.stringify({ code: -1, message: "nope" });
    return JSON.stringify({ code: 0, message: "", data: name in data ? data[name] : true });
  } };
  return { bridge: new Proxy({}, handler), calls };
}

function fakePost(answers) {
  const sent = [];
  const post = async (path, body) => { sent.push([path, body]); return answers[path]; };
  return { post, sent };
}

test("connect lädt Workspace, Information, thing und liveshare", () => {
  const { bridge, calls } = fakeBridge();
  connect({ ...lic, ...conn }, bridge, () => {});
  assert.deepEqual(calls.map((c) => c[0]), [
    "platformSetWorkspaceId", "platformSetInformation", "platformLoadComponent", "platformLoadComponent",
  ]);
  assert.deepEqual(calls[1].slice(1), ["Suchlage", "DRK I&K", "RC Plus 1"]);
  assert.deepEqual(JSON.parse(calls[2][2]), { host: conn.mqttHost, username: conn.mqttUser, password: "p", connectCallback: "onThingConnect" });
  assert.equal(JSON.parse(calls[3][2]).videoPublishType, "video-demand-aux-manual");
});

test("mit API-Token lädt connect danach api, ws und map in dieser Reihenfolge", () => {
  const { bridge, calls } = fakeBridge();
  connect({ ...lic, ...conn, ...map }, bridge, () => {});
  const modules = calls.filter((c) => c[0] === "platformLoadComponent").map((c) => c[1]);
  assert.deepEqual(modules, ["thing", "liveshare", "api", "ws", "map"]);
  const param = (name) => JSON.parse(calls.find((c) => c[1] === name)[2]);
  assert.deepEqual(param("api"), { host: "https://suchlage.test", token: "t0k" });
  assert.deepEqual(param("ws"), { host: "wss://suchlage.test/pilot/ws", token: "t0k", connectCallback: "onWsConnect" });
  assert.deepEqual(param("map"), { userName: "RC Plus 1", elementPreName: "Pilot" });
});

test("scheitert ein Kartenmodul, bleibt die Kopplung bestehen und das Protokoll nennt den Grund", () => {
  const failsOn = (name, [module]) => name === "platformLoadComponent" && module === "ws";
  const { bridge, calls } = fakeBridge({ failsOn });
  const log = [];
  assert.equal(connectMap({ ...lic, ...map }, bridge, (m) => log.push(m)), false);
  assert.ok(log.some((m) => m.includes("Kartenelemente nicht verfügbar")), log.join("\n"));
  assert.deepEqual(calls.map((c) => c[1]), ["api", "ws"], "map wird ohne ws nicht geladen");
  assert.doesNotThrow(() => connect({ ...lic, ...conn, ...map }, fakeBridge({ failsOn }).bridge, () => {}));
});

test("ohne API-Token (älterer Server) lädt connectMap nichts", () => {
  const { bridge, calls } = fakeBridge();
  assert.equal(connectMap({ ...lic, ...conn }, bridge, () => {}), false);
  assert.equal(calls.length, 0);
});

test("mit Fotospeicher lädt connect nach den Kartenmodulen media mit Auto-Upload der Originale", () => {
  const { bridge, calls } = fakeBridge();
  const log = [];
  const loaded = connect({ ...lic, ...conn, ...map, media: true }, bridge, (m) => log.push(m));
  assert.deepEqual(loaded, { map: true, media: true });
  const modules = calls.filter((c) => c[0] === "platformLoadComponent").map((c) => c[1]);
  assert.deepEqual(modules, ["thing", "liveshare", "api", "ws", "map", "media"]);
  const media = calls.find((c) => c[1] === "media");
  assert.deepEqual(JSON.parse(media[2]), { autoUploadPhoto: true, autoUploadPhotoType: 0, autoUploadVideo: false });
  assert.deepEqual(MEDIA_PARAMS, JSON.parse(media[2]));
  assert.ok(log.includes("Foto-Upload: automatisch (Original)"), log.join("\n"));
});

test("ohne Fotospeicher oder ohne API-Token lädt connectMedia nichts", () => {
  for (const cfg of [{ ...lic, ...map, media: false }, { ...lic, media: true }]) {
    const { bridge, calls } = fakeBridge();
    assert.equal(connectMedia(cfg, bridge, () => {}), false);
    assert.equal(calls.length, 0);
  }
});

test("scheitert media oder ist der Auto-Upload aus, bleibt die Kopplung und das Protokoll sagt es", () => {
  const failsOn = (name, [module]) => name === "platformLoadComponent" && module === "media";
  const log = [];
  const { bridge } = fakeBridge({ failsOn });
  assert.deepEqual(connect({ ...lic, ...conn, ...map, media: true }, bridge, (m) => log.push(m)), { map: true, media: false });
  assert.ok(log.some((m) => m.startsWith("Foto-Upload nicht verfügbar")), log.join("\n"));
  const off = [];
  const { bridge: offBridge } = fakeBridge({ data: { mediaGetAutoUploadPhoto: false } });
  assert.equal(connectMedia({ ...lic, ...map, media: true }, offBridge, (m) => off.push(m)), false);
  assert.ok(off.some((m) => m.startsWith("Foto-Upload: aus")), off.join("\n"));
});

test("verify bricht bei abgelehnter Lizenz ab", () => {
  const { bridge, calls } = fakeBridge({ failing: { platformVerifyLicense: true } });
  const log = [];
  assert.throws(() => verify(lic, bridge, (m) => log.push(m)), /platformVerifyLicense/);
  assert.equal(calls.length, 1);
  assert.match(log[0], /FEHLER -1 nope/);
});

test("readRcSn liest data aus der Hülle, sonst den Rohwert, und prüft das Format", () => {
  assert.equal(readRcSn(fakeBridge({ data: { platformGetRemoteControllerSN: "RCSN01" } }).bridge), "RCSN01");
  assert.equal(readRcSn(fakeBridge({ raw: { platformGetRemoteControllerSN: "RCSN02" } }).bridge), "RCSN02");
  assert.throws(() => readRcSn(fakeBridge({ failing: { platformGetRemoteControllerSN: true } }).bridge), /platformGetRemoteControllerSN/);
  assert.throws(() => readRcSn(fakeBridge({ raw: { platformGetRemoteControllerSN: '{"x":1' } }).bridge), /nicht lesbar/);
});

test("pair prüft die Lizenz, bevor die Seriennummer gelesen wird, und sendet sie als reinen Text", async () => {
  const { bridge, calls } = fakeBridge({ data: { platformGetRemoteControllerSN: "RCSN01" } });
  const { post, sent } = fakePost({ "/pilot/api/license": { ok: true, body: lic }, "/pilot/api/connect": { ok: true, body: conn } });
  const cfg = await pair("abc", bridge, () => {}, post);
  const names = calls.map((c) => c[0]);
  assert.ok(names.indexOf("platformIsVerified") < names.indexOf("platformGetRemoteControllerSN"));
  assert.deepEqual(sent, [["/pilot/api/license", { code: "abc" }], ["/pilot/api/connect", { code: "abc", rc_sn: "RCSN01" }]]);
  assert.equal(cfg.mqttUser, "rc-RCSN01");
  assert.equal(names.at(-1), "platformLoadComponent");
});

test("pair zeigt die Ablehnung des Servers und ruft Pilot 2 dann nicht auf", async () => {
  const { bridge, calls } = fakeBridge();
  const { post } = fakePost({ "/pilot/api/license": { ok: false, body: { error: "Kopplungscode ungültig" } } });
  await assert.rejects(pair("falsch", bridge, () => {}, post), (e) => e instanceof PairingError && e.message === "Kopplungscode ungültig");
  assert.equal(calls.length, 0);
});

test("startManualLive setzt RTMP und startet", () => {
  const { bridge, calls } = fakeBridge();
  startManualLive(conn, bridge, () => {});
  assert.deepEqual(calls[0], ["liveshareSetConfig", 2, JSON.stringify({ url: conn.rtmpUrl })]);
  assert.equal(calls[1][0], "liveshareStartLive");
});

test("verify meldet Fehler der JSBridge als Fehlerzeile", () => {
  const { bridge } = fakeBridge({ failing: { platformVerifyLicense: true } });
  const levels = [];
  assert.throws(() => verify(lic, bridge, (m, level) => levels.push(level)));
  assert.deepEqual(levels, ["error"]);
});

test("isConnected versteht Boolean und Text", () => {
  assert.equal(isConnected(true), true);
  assert.equal(isConnected("true"), true);
  assert.equal(isConnected(false), false);
  assert.equal(isConnected("false"), false);
});

test("liveStatus macht aus dem Objekt lesbaren Text und ändert den Schlüssel nur mit dem Zustand", () => {
  const a = liveStatus({ status: 1, fps: 30 });
  assert.equal(a.text, '{"status":1,"fps":30}');
  assert.equal(liveStatus('{"status":1,"fps":25}').key, a.key, "nur die Bildrate ist anders");
  assert.notEqual(liveStatus({ status: 2, fps: 30 }).key, a.key);
  assert.equal(liveStatus({ fps: 30 }).key, '{"fps":30}', "ohne Zustandsfeld zählt alles");
  assert.deepEqual(liveStatus("kein json"), { text: "kein json", key: "kein json" });
  assert.equal(liveStatus(3).text, "3");
});

function manualTimers() {
  const pending = [];
  return { schedule: (fn) => { pending.push(fn); return pending.length; }, run: async () => { const fns = pending.splice(0); for (const f of fns) await f(); } };
}

test("remoteLog bündelt Zeilen, schickt Fehler sofort und behält sie ohne Netz", async () => {
  const sent = [];
  let online = true;
  const post = async (path, body) => {
    if (!online) throw new Error("offline");
    sent.push([path, body]);
    return { ok: true, body: {} };
  };
  const timers = manualTimers();
  const log = remoteLog("CODE", post, { now: () => 5, schedule: timers.schedule });
  log.add("a");
  log.add("b", "warn");
  assert.equal(sent.length, 0, "gebündelt");
  await timers.run();
  assert.deepEqual(sent[0], ["/pilot/api/log", { code: "CODE", lines: [
    { at_ms: 5, level: "info", text: "a" }, { at_ms: 5, level: "warn", text: "b" },
  ] }]);

  online = false;
  await log.add("kaputt", "error");
  assert.equal(sent.length, 1, "ohne Netz nichts gesendet");
  online = true;
  await timers.run();
  assert.equal(sent[1][1].lines[0].text, "kaputt", "beim nächsten Versuch nachgeholt");
});

test("remoteLog verwirft Zeilen, die der Server abgelehnt hat", async () => {
  const sent = [];
  const post = async (_, body) => { sent.push(body); return { ok: false, body: {} }; };
  const timers = manualTimers();
  const log = remoteLog("FALSCH", post, { schedule: timers.schedule });
  await log.add("x", "error");
  await timers.run();
  assert.equal(sent.length, 1, "kein Wiederholen ohne Aussicht");
});
