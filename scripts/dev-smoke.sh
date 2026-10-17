#!/bin/sh
# Ende-zu-Ende ohne Hardware: echte Container, echter Server, Simulator mit Testbild.
set -u
cd "$(dirname "$0")/.."
FAIL=0
check() { if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAIL $2"; FAIL=1; fi; }
# Eigenes Compose-Projekt: `down -v` darf nie den laufenden Dev-Stack (suchlage-dev) samt Broker-Volume treffen.
if curl -s -o /dev/null http://127.0.0.1:8080/api/health || nc -z 127.0.0.1 1883 2>/dev/null; then
  echo "Port 8080 oder 1883 ist belegt – läuft noch \`mise run dev\`? Erst beenden."; exit 1
fi
TMP=$(mktemp -d)
export SUCHLAGE_DATA_DIR="$TMP/data" SUCHLAGE_BIND=127.0.0.1:8080 SUCHLAGE_PUBLIC_URL=http://127.0.0.1:8080
DC="docker compose -p suchlage-smoke -f deploy/compose.dev.yaml"
pids=""
cleanup() { for p in $pids; do kill "$p" 2>/dev/null || true; done; $DC down -v >/dev/null 2>&1; rm -rf "$TMP"; }
trap cleanup EXIT

grep -q 'recordDeleteAfter: 0s' deploy/mediamtx.m1.yml; check $? "MediaMTX löscht Aufnahmen nicht"

$DC up -d --wait || { echo "FAIL Container starten"; exit 1; }
cargo build -q --features dev --bin suchlage --bin suchlage-sim || exit 1
# Das Zielverzeichnis kann per Cargo-Konfiguration außerhalb des Repos liegen.
BIN="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)/debug"
"$BIN/suchlage" >"$TMP/server.log" 2>&1 & pids="$pids $!"
i=0; until curl -sf http://127.0.0.1:8080/api/health >/dev/null; do i=$((i+1)); [ $i -lt 60 ] || break; sleep 1; done
curl -sf http://127.0.0.1:8080/api/health >/dev/null; check $? "Server läuft"

JAR="$TMP/jar"
curl -sf -c "$JAR" -X POST http://127.0.0.1:8080/api/auth/dev-login >/dev/null
api() { curl -sf -b "$JAR" -H 'content-type: application/json' "$@"; }
code_for() { api -X POST -d "{\"label\":\"$1\"}" http://127.0.0.1:8080/api/remote-controllers | jq -r '.pairing_url | split("#")[1]'; }
connect() { curl -sf -X POST -H 'content-type: application/json' -d "{\"code\":\"$1\",\"rc_sn\":\"$2\"}" http://127.0.0.1:8080/pilot/api/connect; }
CODE_A=$(code_for "RC A"); CODE_B=$(code_for "RC B")
# Die Broker-Sitzung braucht nach dem Start einen Moment; bis dahin antwortet connect mit 503.
i=0; until A=$(connect "$CODE_A" RCA); do i=$((i+1)); [ $i -lt 30 ] || break; sleep 1; done
B=$(connect "$CODE_B" RCB)
PASS_A=$(printf '%s' "$A" | jq -r .mqttPass); PASS_B=$(printf '%s' "$B" | jq -r .mqttPass)
[ -n "$PASS_A" ] && [ "$PASS_A" != null ] && [ -n "$PASS_B" ] && [ "$PASS_B" != null ]; check $? "Kopplung liefert Zugangsdaten"
curl -sf -X POST -H 'content-type: application/json' -d "{\"code\":\"$CODE_A\"}" http://127.0.0.1:8080/pilot/api/license | jq -e '.appKey != null and .mqttPass == null' >/dev/null
check $? "Lizenzschritt liefert Lizenz, aber keinen Broker-Zugang"
for ep in license connect; do
  curl -s -X POST -H 'content-type: application/json' -d '{"code":"falsch","rc_sn":"RCA"}' "http://127.0.0.1:8080/pilot/api/$ep" | grep -qE 'mqttPass|appKey'
  [ $? -ne 0 ]; check $? "ohne gültigen Code nichts Geheimes ($ep)"
done

mq() { $DC exec -T mqtt "$@"; }
# update_topo wird beantwortet (Serverrolle darf senden, RC darf ihr status_reply abonnieren)
# Nur auf die eigenen Abonnenten warten: ein bloßes `wait` wartete auch auf den Server.
( mq mosquitto_sub -u rc-RCA -P "$PASS_A" -t sys/product/RCA/status_reply -C 1 -W 6 > "$TMP/topo" 2>/dev/null ) & sub=$!
sleep 1
mq mosquitto_pub -u rc-RCA -P "$PASS_A" -t sys/product/RCA/status -m '{"tid":"t1","bid":"b1","method":"update_topo","data":{"sub_devices":[]}}'
wait $sub
jq -e '.tid=="t1" and .data.result==0' "$TMP/topo" >/dev/null 2>&1; check $? "update_topo beantwortet"

# Review Focus 5: RC A sieht und schreibt nichts von RC B
mq mosquitto_sub -u rc-RCA -P "$PASS_A" -t thing/product/RCB/services -C 1 -W 2 > "$TMP/acl1" 2>&1
grep -qi "denied" "$TMP/acl1"; check $? "RC A darf services von RC B nicht abonnieren"
# Gegenprobe im selben Lauf: RC B bekommt die Antwort auf die eigene Meldung. Sonst hieße eine leere
# Datei nur, dass RC B nicht verbunden war, nicht dass die ACL RC A abgewiesen hat.
# „eigen“ geht zuerst raus: Kommt seine Antwort an, lauschte das Abo schon, als „fremd“ gesendet wurde.
( mq mosquitto_sub -u rc-RCB -P "$PASS_B" -t sys/product/RCB/status_reply -C 2 -W 6 > "$TMP/acl2" 2>&1 ) & sub=$!
sleep 1
mq mosquitto_pub -u rc-RCB -P "$PASS_B" -t sys/product/RCB/status -m '{"tid":"eigen","bid":"x","method":"update_topo","data":{"sub_devices":[]}}'
mq mosquitto_pub -u rc-RCA -P "$PASS_A" -t sys/product/RCB/status -m '{"tid":"fremd","bid":"x","method":"update_topo","data":{"sub_devices":[]}}'
wait $sub
grep -q '"tid":"eigen"' "$TMP/acl2"; check $? "RC B bekommt die Antwort auf die eigene Meldung"
! grep -q '"tid":"fremd"' "$TMP/acl2"; check $? "RC A kann nicht als RC B melden"
# Rollenpriorität (ADR 0003): RC A darf Fluggeräte-Telemetrie senden, aber nicht die Topics von RC B.
# Ein Abonnent auf beiden Topics, das erlaubte Publish zuerst: Kommt es an, lauschte das Abo schon beim verbotenen.
( mq mosquitto_sub -u suchlage-admin -P dev-admin -v -t thing/product/RCB/state -t thing/product/ACX/osd -C 2 -W 4 > "$TMP/acl34" 2>/dev/null ) & sub=$!
sleep 1
# Legt im Kern ein Phantom-Fluggerät ACX an RCA an; die Flugprüfung unten zählt nur gelandete Flüge und bleibt davon unberührt.
mq mosquitto_pub -u rc-RCA -P "$PASS_A" -t thing/product/ACX/osd -m '{"gateway":"RCA","data":{"latitude":52.0,"longitude":10.0}}'
mq mosquitto_pub -u rc-RCA -P "$PASS_A" -t thing/product/RCB/state -m '{"gateway":"RCB","data":{"live_status":[]}}'
wait $sub
grep -q '^thing/product/ACX/osd' "$TMP/acl34"; check $? "RC A darf Fluggeräte-Telemetrie senden"
! grep -q '^thing/product/RCB/state' "$TMP/acl34"; check $? "RC A kann nicht als RC B telemetrieren"

# RTMP mit falschem Schlüssel wird abgewiesen
ffmpeg -loglevel error -re -f lavfi -i testsrc=size=320x180:rate=10 -t 2 -c:v libx264 -preset ultrafast -f flv "rtmp://127.0.0.1:1935/live/RCA?user=rc-RCA&pass=falsch" 2>/dev/null
[ $? -ne 0 ]; check $? "RTMP mit falschem Schlüssel abgelehnt"

# Ganzer Flug im Zeitraffer, mit Livebild (Auto-Start beim Abheben)
"$BIN/suchlage-sim" --server http://127.0.0.1:8080 --code "$CODE_A" --rc-sn RCA --speed 10 >"$TMP/sim.log" 2>&1
check $? "Simulator spielt den Flug ab"
sleep 3
FLIGHT=$(api http://127.0.0.1:8080/api/flights | jq '.[0]')
[ "$(printf '%s' "$FLIGHT" | jq -r .end_reason)" = landed ]; check $? "Flug erkannt und gelandet"
ID=$(printf '%s' "$FLIGHT" | jq -r .id)
api "http://127.0.0.1:8080/api/flights/$ID" | jq -e '.segments | length >= 1' >/dev/null; check $? "Spur gespeichert"
api "http://127.0.0.1:8080/api/flights/$ID" | jq -e '.video != null' >/dev/null; check $? "Flugvideo aufgezeichnet"
SIZE=$(curl -sf -b "$JAR" "http://127.0.0.1:8080/api/flights/$ID/video?download=1" | wc -c)
[ "$SIZE" -gt 1000 ]; check $? "Flugvideo herunterladbar ($SIZE Bytes)"

[ $FAIL = 0 ] && echo ok || { echo "--- Server-Log (Ende)"; tail -40 "$TMP/server.log"; echo "--- Simulator-Log"; tail -20 "$TMP/sim.log"; }
exit $FAIL
