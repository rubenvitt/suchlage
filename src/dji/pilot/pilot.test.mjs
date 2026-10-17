import { test } from "node:test";
import assert from "node:assert/strict";
import { PairingError, connect, pair, readRcSn, startManualLive, verify } from "./pilot.js";

const lic = { appId: "1", appKey: "k", license: "l", workspaceId: "768d4a2e-1f34-477c-9fcc-872285e895f6", rcLabel: "RC Plus 1" };
const conn = { mqttHost: "tcp://127.0.0.1:1883", mqttUser: "rc-RCSN01", mqttPass: "p", rtmpUrl: "rtmp://127.0.0.1:1935/live/RCSN01?user=rc-RCSN01&pass=x" };

/** JSBridge-Attrappe: jede Antwort ist eine JSON-Hülle wie in Pilot 2. */
function fakeBridge({ failing = {}, data = {}, raw = {} } = {}) {
  const calls = [];
  const handler = { get: (_, name) => (...args) => {
    calls.push([name, ...args]);
    if (name in raw) return raw[name];
    if (failing[name]) return JSON.stringify({ code: -1, message: "nope" });
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
