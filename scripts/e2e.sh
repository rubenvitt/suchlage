#!/bin/sh
# Playwright-Smoke: Arbeitsplatz, Großbild und mobil gegen den Simulator (mise run e2e).
# Baut wie dev.sh, liefert die SPA aber aus dem Server aus und läuft ohne Vite.
set -eu
cd "$(dirname "$0")/.."
for tool in docker cargo pnpm ffmpeg curl jq nc; do
  command -v "$tool" >/dev/null || { echo "Fehlt: $tool"; exit 1; }
done
. scripts/lib/ports.sh
pick_ports
URL=http://127.0.0.1:$SL_SERVER_PORT
TMP=$(mktemp -d)
# E2E_BASE_URL liest nur playwright.config.ts.
export SUCHLAGE_DATA_DIR="$TMP/data" SUCHLAGE_BIND=127.0.0.1:$SL_SERVER_PORT SUCHLAGE_PUBLIC_URL=$URL E2E_BASE_URL=$URL
# MediaMTX zeichnet in die Videoablage des Servers auf (compose.dev.yaml).
export SL_VIDEO_DIR="$SUCHLAGE_DATA_DIR/video"
# Vorher anlegen: Fehlt die Quelle, legt Docker sie (unter Linux als root) samt data/ selbst an.
mkdir -p "$SL_VIDEO_DIR"
# Eigenes Compose-Projekt wie dev-smoke: `down -v` trifft nie den Dev-Stack.
DC="docker compose -p suchlage-e2e -f deploy/compose.dev.yaml"
pids=""
cleanup() { for p in $pids; do kill "$p" 2>/dev/null || true; done; $DC down -v >/dev/null 2>&1; rm -rf "$TMP"; }
trap cleanup EXIT
$DC up -d --wait
pnpm -C frontend install --frozen-lockfile >/dev/null
# Vor cargo build: Der Server bettet frontend/dist ein.
pnpm -C frontend build >/dev/null
cargo build -q --features dev --bin suchlage --bin suchlage-sim
# Das Zielverzeichnis kann per Cargo-Konfiguration außerhalb des Repos liegen.
BIN="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)/debug"
"$BIN/suchlage" >"$TMP/server.log" 2>&1 & pids="$pids $!"
i=0
until curl -sf "$URL/api/health" >/dev/null; do
  i=$((i + 1)); [ $i -lt 60 ] || { echo "Server startet nicht"; tail -40 "$TMP/server.log"; exit 1; }; sleep 1
done
# Vor dem Simulator: Er bekommt die Broker-Adresse beim Koppeln aus den Einstellungen.
curl -sf -c "$TMP/jar" -X POST "$URL/api/auth/dev-login" >/dev/null
use_ports "$TMP/jar" "$URL"
# Simulator-Fernsteuerung mit festem Code legt der Server im Feature `dev` selbst an (B14).
"$BIN/suchlage-sim" --server "$URL" --code dev-simulator --speed 2 --loop >"$TMP/sim.log" 2>&1 & pids="$pids $!"
pnpm -C frontend exec playwright install chromium >/dev/null
pnpm -C frontend e2e "$@"
