import { test } from "node:test";
import assert from "node:assert/strict";
import { connect, startManualLive } from "./pilot.js";

const cfg = {
  appId: "1", appKey: "k", license: "l",
  workspaceId: "768d4a2e-1f34-477c-9fcc-872285e895f6",
  mqttHost: "tcp://127.0.0.1:1883", mqttUser: "rc-plus-01", mqttPass: "p",
  rtmpUrl: "rtmp://127.0.0.1:1935/live/m350?user=pilot&pass=x",
};

function fakeBridge(failing = {}) {
  const calls = [];
  const handler = { get: (_, name) => (...args) => {
    calls.push([name, ...args]);
    if (failing[name]) return JSON.stringify({ code: -1, message: "nope" });
    return JSON.stringify({ code: 0, message: "", data: true });
  } };
  return { bridge: new Proxy({}, handler), calls };
}

test("connect verifies license, then loads thing and liveshare", () => {
  const { bridge, calls } = fakeBridge();
  connect(cfg, bridge, () => {});
  assert.deepEqual(calls.map((c) => c[0]), [
    "platformVerifyLicense", "platformIsVerified", "platformSetWorkspaceId",
    "platformSetInformation", "platformLoadComponent", "platformLoadComponent",
  ]);
  assert.deepEqual(calls[0].slice(1), ["1", "k", "l"]);
  assert.equal(calls[4][1], "thing");
  assert.deepEqual(JSON.parse(calls[4][2]), {
    host: "tcp://127.0.0.1:1883", username: "rc-plus-01", password: "p", connectCallback: "onThingConnect",
  });
  assert.equal(calls[5][1], "liveshare");
  assert.equal(JSON.parse(calls[5][2]).videoPublishType, "video-demand-aux-manual");
});

test("connect stops before loading modules when license check fails", () => {
  const { bridge, calls } = fakeBridge({ platformVerifyLicense: true });
  const log = [];
  assert.throws(() => connect(cfg, bridge, (m) => log.push(m)), /platformVerifyLicense/);
  assert.equal(calls.length, 1);
  assert.match(log[0], /FEHLER -1 nope/);
});

test("startManualLive sets RTMP config and starts", () => {
  const { bridge, calls } = fakeBridge();
  startManualLive(cfg, bridge, () => {});
  assert.deepEqual(calls[0], ["liveshareSetConfig", 2, JSON.stringify({ url: cfg.rtmpUrl })]);
  assert.equal(calls[1][0], "liveshareStartLive");
});
