#!/bin/sh
# Ende-zu-Ende ohne Hardware: echte Container, echter Server, Simulator mit Testbild.
set -u
cd "$(dirname "$0")/.."
FAIL=0
check() { if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAIL $2"; FAIL=1; fi; }
. scripts/lib/ports.sh
pick_ports
URL=http://127.0.0.1:$SL_SERVER_PORT
TMP=$(mktemp -d)
export SUCHLAGE_DATA_DIR="$TMP/data" SUCHLAGE_BIND=127.0.0.1:$SL_SERVER_PORT SUCHLAGE_PUBLIC_URL=$URL
# Eigenes Compose-Projekt: `down -v` darf nie den laufenden Dev-Stack (suchlage-dev) samt Broker-Volume treffen.
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
i=0; until curl -sf $URL/api/health >/dev/null; do i=$((i+1)); [ $i -lt 60 ] || break; sleep 1; done
curl -sf $URL/api/health >/dev/null; check $? "Server läuft"

JAR="$TMP/jar"
curl -sf -c "$JAR" -X POST "$URL/api/auth/dev-login" >/dev/null
use_ports "$JAR" "$URL" || exit 1
api() { curl -sf -b "$JAR" -H 'content-type: application/json' "$@"; }
code_for() { api -X POST -d "{\"label\":\"$1\"}" $URL/api/remote-controllers | jq -r '.pairing_url | split("#")[1]'; }
connect() { curl -sf -X POST -H 'content-type: application/json' -d "{\"code\":\"$1\",\"rc_sn\":\"$2\"}" $URL/pilot/api/connect; }
CODE_A=$(code_for "RC A"); CODE_B=$(code_for "RC B")
# Die Broker-Sitzung braucht nach dem Start einen Moment; bis dahin antwortet connect mit 503.
i=0; until A=$(connect "$CODE_A" RCA); do i=$((i+1)); [ $i -lt 30 ] || break; sleep 1; done
B=$(connect "$CODE_B" RCB)
PASS_A=$(printf '%s' "$A" | jq -r .mqttPass); PASS_B=$(printf '%s' "$B" | jq -r .mqttPass)
[ -n "$PASS_A" ] && [ "$PASS_A" != null ] && [ -n "$PASS_B" ] && [ "$PASS_B" != null ]; check $? "Kopplung liefert Zugangsdaten"
curl -sf -X POST -H 'content-type: application/json' -d "{\"code\":\"$CODE_A\"}" $URL/pilot/api/license | jq -e '.appKey != null and .mqttPass == null' >/dev/null
check $? "Lizenzschritt liefert Lizenz, aber keinen Broker-Zugang"
for ep in license connect; do
  curl -s -X POST -H 'content-type: application/json' -d '{"code":"falsch","rc_sn":"RCA"}' "$URL/pilot/api/$ep" | grep -qE 'mqttPass|appKey'
  [ $? -ne 0 ]; check $? "ohne gültigen Code nichts Geheimes ($ep)"
done

# M2: Kartenelemente über die Pilot-Schnittstelle, mit dem Token aus dem Koppeln.
WS=$(curl -sf -X POST -H 'content-type: application/json' -d "{\"code\":\"$CODE_A\"}" $URL/pilot/api/license | jq -r .workspaceId)
TOKEN_A=$(printf '%s' "$A" | jq -r .apiToken)
MAP="$URL/map/api/v1/workspaces/$WS"
GROUP=$(curl -sf -H "x-auth-token: $TOKEN_A" "$MAP/element-groups" | jq -r '.data[] | select(.type == 2) | .id')
[ -n "$GROUP" ]; check $? "Pilot findet die App-Shared-Gruppe"
PIN='{"id":"smoke-pin","name":"Smoke","resource":{"type":0,"user_name":"","content":{"type":"Feature","properties":{"color":"#E23C39","clampToGround":true},"geometry":{"type":"Point","coordinates":[10.01,52.43]}}}}'
curl -sf -H "x-auth-token: $TOKEN_A" -H 'content-type: application/json' -d "$PIN" "$MAP/element-groups/$GROUP/elements" | jq -e '.code == 0' >/dev/null
check $? "Pilot legt einen Punkt an"
api $URL/api/map/elements | jq -e 'map(select(.id == "smoke-pin" and .origin == "pilot" and .author == "RC A")) | length == 1' >/dev/null
check $? "Punkt aus Pilot 2 erscheint im Browser"
[ "$(curl -s -o /dev/null -w '%{http_code}' -H 'x-auth-token: falsch' "$MAP/element-groups")" = 401 ]
check $? "Kartenelemente ohne gültigen Token: 401"
# M3: Upload-Zugang nur mit Token, Ablageort je Fernsteuerung.
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST -H 'x-auth-token: falsch' "$URL/storage/api/v1/workspaces/$WS/sts")" = 401 ]
check $? "Foto-Upload ohne gültigen Token: 401"
STS=$(curl -sf -X POST -H "x-auth-token: $TOKEN_A" "$URL/storage/api/v1/workspaces/$WS/sts")
printf '%s' "$STS" | jq -e '.data.object_key_prefix == "fotos/RCA" and .data.credentials.access_key_id != null' >/dev/null
check $? "sts liefert einen Upload-Schlüssel für fotos/RCA"

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
ffmpeg -loglevel error -re -f lavfi -i testsrc=size=320x180:rate=10 -t 2 -c:v libx264 -preset ultrafast -f flv "rtmp://127.0.0.1:$SL_RTMP_PORT/live/RCA?user=rc-RCA&pass=falsch" 2>/dev/null
[ $? -ne 0 ]; check $? "RTMP mit falschem Schlüssel abgelehnt"

# Einsätze: Der Flug gehört zu dem Einsatz, der beim Start läuft.
MISSION=$(api -X POST -d '{"name":"Smoke-Einsatz","location":null}' $URL/api/missions | jq -r .id)
api -X POST "$URL/api/missions/$MISSION/start" | jq -e '.state == "running"' >/dev/null
check $? "Einsatz angelegt und gestartet"

# Ganzer Flug im Zeitraffer, mit Livebild (Auto-Start beim Abheben)
"$BIN/suchlage-sim" --server $URL --code "$CODE_A" --rc-sn RCA --speed 10 >"$TMP/sim.log" 2>&1
check $? "Simulator spielt den Flug ab"
sleep 3
FLIGHT=$(api $URL/api/flights | jq '.[0]')
[ "$(printf '%s' "$FLIGHT" | jq -r .end_reason)" = landed ]; check $? "Flug erkannt und gelandet"
ID=$(printf '%s' "$FLIGHT" | jq -r .id)
api "$URL/api/flights?mission=$MISSION" | jq -e --argjson id "$ID" 'map(select(.id == $id)) | length == 1' >/dev/null
check $? "Flug steht in der Flugliste des Einsatzes"
api "$URL/api/flights?mission=none" | jq -e --argjson id "$ID" 'map(select(.id == $id)) | length == 0' >/dev/null
check $? "Flug fehlt unter „ohne Einsatz“"
api "$URL/api/flights/$ID" | jq -e '.segments | length >= 1' >/dev/null; check $? "Spur gespeichert"
api "$URL/api/flights/$ID" | jq -e '.video != null' >/dev/null; check $? "Flugvideo aufgezeichnet"
SIZE=$(curl -sf -b "$JAR" "$URL/api/flights/$ID/video?download=1" | wc -c)
[ "$SIZE" -gt 1000 ]; check $? "Flugvideo herunterladbar ($SIZE Bytes)"

# M3: Der Simulator lädt nach der Landung eine Auslösung (Wärme und Weit) über Garage hoch.
SHOT=$(api $URL/api/photos | jq '.[-1]')
[ "$(printf '%s' "$SHOT" | jq -r .flight_id)" = "$ID" ]; check $? "Foto dem Flug zugeordnet"
printf '%s' "$SHOT" | jq -e '.position != null and ([.photos[].lens] == ["thermal", "wide"])' >/dev/null
check $? "Foto mit Aufnahmeort, Wärmebild zuerst"
PHOTO=$(printf '%s' "$SHOT" | jq -r '.photos[0].id')
i=0; until curl -sf -b "$JAR" -o /dev/null "$URL/api/photos/$PHOTO/thumb"; do i=$((i+1)); [ $i -lt 20 ] || break; sleep 1; done
curl -sf -b "$JAR" -o /dev/null "$URL/api/photos/$PHOTO/thumb"; check $? "Vorschaubild erzeugt"
SIZE=$(curl -sf -b "$JAR" "$URL/api/photos/$PHOTO/original?download=1" | wc -c)
[ "$SIZE" -gt 1000 ]; check $? "Original herunterladbar ($SIZE Bytes)"

[ $FAIL = 0 ] && echo ok || { echo "--- Server-Log (Ende)"; tail -40 "$TMP/server.log"; echo "--- Simulator-Log"; tail -20 "$TMP/sim.log"; }
exit $FAIL
