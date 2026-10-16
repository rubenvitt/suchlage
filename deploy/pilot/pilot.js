// Spike-Anmeldeseite für DJI Pilot 2 (M0). Wird in M1 durch die Seite des Rust-Servers ersetzt.

function caller(bridge, log) {
  return (name, ...args) => {
    const res = JSON.parse(bridge[name](...args));
    log(`${name}: ${res.code === 0 ? "ok" : `FEHLER ${res.code} ${res.message}`}`);
    if (res.code !== 0) throw new Error(name);
    return res.data;
  };
}

export function connect(cfg, bridge, log) {
  const call = caller(bridge, log);
  call("platformVerifyLicense", cfg.appId, cfg.appKey, cfg.license);
  if (!call("platformIsVerified")) throw new Error("platformIsVerified");
  call("platformSetWorkspaceId", cfg.workspaceId);
  call("platformSetInformation", "Suchlage", "DRK I&K", "Spike M0");
  call("platformLoadComponent", "thing", JSON.stringify({
    host: cfg.mqttHost, username: cfg.mqttUser, password: cfg.mqttPass, connectCallback: "onThingConnect",
  }));
  call("platformLoadComponent", "liveshare", JSON.stringify({
    videoPublishType: "video-demand-aux-manual", statusCallback: "onLiveStatus",
  }));
}

export function startManualLive(cfg, bridge, log) {
  const call = caller(bridge, log);
  call("liveshareSetConfig", 2, JSON.stringify({ url: cfg.rtmpUrl }));
  call("liveshareStartLive");
}
