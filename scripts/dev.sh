#!/bin/sh
# Ein Kommando für die lokale Oberfläche mit Simulator: mise run dev
# Kein 1Password, keine echten Geheimnisse – Dev-Werte stehen in deploy/compose.dev.yaml und im Feature `dev`.
set -eu
cd "$(dirname "$0")/.."
for tool in docker cargo pnpm ffmpeg curl jq nc; do
  command -v "$tool" >/dev/null || { echo "Fehlt: $tool"; exit 1; }
done

# Ein belegter Port ließe einen Teil still ausfallen (Vite startet mit strictPort gar nicht).
for port in 8080 5173 1883 3900; do
  if nc -z 127.0.0.1 "$port" 2>/dev/null; then
    echo "Port $port ist belegt – läuft schon \`mise run dev\` oder \`mise run ui\`? Erst beenden."; exit 1
  fi
done

export SUCHLAGE_DATA_DIR=./data/dev SUCHLAGE_BIND=127.0.0.1:8080 SUCHLAGE_PUBLIC_URL=http://127.0.0.1:5173
mkdir -p data/dev/video
DC="docker compose -f deploy/compose.dev.yaml"
$DC up -d --wait

pids=""
cleanup() {
  for p in $pids; do kill "$p" 2>/dev/null || true; done
  $DC stop >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

pnpm -C frontend install --frozen-lockfile >/dev/null
cargo build --features dev --bin suchlage --bin suchlage-sim
# Das Zielverzeichnis kann per Cargo-Konfiguration außerhalb des Repos liegen.
BIN="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)/debug"
"$BIN/suchlage" & pids="$pids $!"
pnpm -C frontend dev --host 127.0.0.1 & pids="$pids $!"

i=0
until curl -sf http://127.0.0.1:8080/api/health >/dev/null; do
  i=$((i + 1)); [ $i -lt 60 ] || { echo "Server startet nicht"; exit 1; }; sleep 1
done

# Die Simulator-Fernsteuerung legt der Server mit Feature `dev` selbst an (fester Code, B14). Der
# Simulator wartet selbst, bis die Broker-Sitzung steht (503 → neuer Versuch, B16).
"$BIN/suchlage-sim" --server http://127.0.0.1:8080 --code dev-simulator --loop & pids="$pids $!"

echo
echo "Suchlage läuft: http://127.0.0.1:5173  → „Als Entwicklung anmelden“"
echo "Großbild: Einstellungen → Großbild → Zugang erzeugen, Link in zweitem Fenster öffnen."
echo "Mobil: Browserfenster schmaler als 768 px ziehen. Beenden mit Strg+C."
wait
