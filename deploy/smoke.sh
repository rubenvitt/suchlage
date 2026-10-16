#!/bin/sh
# Lokaler Ende-zu-Ende-Test des Spike-Stacks. Startet den Stack auf 127.0.0.1 und prüft jeden Baustein.
# Läuft gegen Testgeheimnisse (op-Attrappe), nie gegen 1Password: kein Freigabedialog, keine echten DJI-Schlüssel in lokalen Dateien.
set -u
cd "$(dirname "$0")"
# Schutz vor einem Lauf auf dem vServer: Der Test überschreibt die Geheimnisse, leert ../records und stoppt den Stack.
[ -z "$(docker ps -aq --filter label=com.docker.compose.project=suchlage)" ] || {
  echo "Stack suchlage existiert – smoke.sh nur auf einer leeren lokalen Umgebung"; exit 1; }
# Nur eigene Testdaten (Marker .smoke) werden kommentarlos gelöscht, echte Mitschnitte nur mit --force.
[ "${1:-}" = --force ] || [ -f ../records/.smoke ] || [ -z "$(ls -A ../records 2>/dev/null)" ] || {
  echo "../records enthält Daten, die nicht von smoke.sh stammen – Abbruch (überschreiben mit --force)"; exit 1; }
export SUCHLAGE_BIND_IP=127.0.0.1
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# op-Attrappe: `op read <ref>` und `op inject -f [--file-mode <m>] -i <in> -o <out>`; Wert deterministisch je Referenz,
# damit render.sh und smoke.sh unabhängig voneinander dieselben Geheimnisse sehen.
cat > "$TMP/op" <<'EOF'
#!/bin/sh
set -eu
val() { printf 'test%s' "$(printf '%s' "$1" | cksum | cut -d' ' -f1)"; }
case $1 in
  read) val "$2" ;;
  inject)
    shift; in= out= mode=0600
    while [ $# -gt 0 ]; do case $1 in -i) in=$2; shift ;; -o) out=$2; shift ;; --file-mode) mode=$2; shift ;; esac; shift; done
    cp "$in" "$out" && chmod "$mode" "$out"  # wie echtes op: Default 0600
    grep -o '{{ op://[^ ]* }}' "$in" | sort -u | while read -r _ ref _; do
      sed -i.bak "s|{{ $ref }}|$(val "$ref")|g" "$out" && rm -f "$out.bak"
    done ;;
  *) echo "op-Attrappe: $1 nicht unterstützt" >&2; exit 2 ;;
esac
EOF
chmod +x "$TMP/op"
export PATH="$TMP:$PATH"

./render.sh || { echo "FAIL render.sh"; exit 1; }
dc() { docker compose --progress quiet "$@"; }
FAIL=0
check() { if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAIL $2"; FAIL=1; fi; }
TOOL_PASS=$(op read op://DRK/suchlage-mqtt/suchlage)
RC_PASS=$(op read op://DRK/suchlage-mqtt/rc-plus-01)
# sub_one <user> <pass> <topic>: erste Nachricht (max. 5 s) ausgeben
sub_one() { dc exec -T mqtt mosquitto_sub -h localhost -u "$1" -P "$2" -t "$3" -C 1 -W 5; }
# pub <user> <pass> <topic> <payload>
pub() { dc exec -T mqtt mosquitto_pub -h localhost -u "$1" -P "$2" -t "$3" -m "$4"; }

rm -rf ../records && mkdir -p ../records && touch ../records/.smoke
dc up -d --build --wait mqtt recorder || dc up -d --build mqtt recorder
sleep 2

# Anonymer Zugriff wird abgelehnt
dc exec -T mqtt mosquitto_pub -h localhost -t thing/product/TEST/osd -m x 2>/dev/null
[ $? -ne 0 ]; check $? "anonymer Publish abgelehnt"

# rc-plus-01 darf auf thing/product/# publizieren, der Mitschnitt speichert es
pub rc-plus-01 "$RC_PASS" thing/product/TEST/osd '{"data":{"latitude":52.96}}'
sleep 1
grep -q '"topic":"thing/product/TEST/osd"' ../records/*.jsonl; check $? "osd landet im Mitschnitt"

# Payload mit Zeilenumbruch bleibt eine Zeile (Review Focus 1)
before=$(cat ../records/*.jsonl | wc -l)
pub rc-plus-01 "$RC_PASS" thing/product/TEST/state "$(printf '{"a":\n1}')"
sleep 1
after=$(cat ../records/*.jsonl | wc -l)
[ $((after - before)) -eq 1 ]; check $? "mehrzeiliger Payload = eine Mitschnittzeile"

# ACL: rc-plus-01 darf nicht außerhalb seiner Topics publizieren
( sub_one suchlage "$TOOL_PASS" foo/bar > "$TMP/acl.out" 2>/dev/null ) &
sleep 1; pub rc-plus-01 "$RC_PASS" foo/bar verboten; wait
[ ! -s "$TMP/acl.out" ]; check $? "ACL blockt fremde Topics"

# Broker-Neustart: Mitschnitt läuft weiter (Review Focus 4)
dc restart mqtt; sleep 3
pub rc-plus-01 "$RC_PASS" thing/product/TEST/osd '{"after":"restart"}'
sleep 2
grep -q 'after.*restart' ../records/*.jsonl; check $? "Mitschnitt überlebt Broker-Neustart"

dc up -d --build topo; sleep 2

# Ungültiges JSON bricht den Responder nicht (Review Focus 2), gültiges update_topo wird beantwortet
pub rc-plus-01 "$RC_PASS" sys/product/TEST/status 'kein json'
( sub_one rc-plus-01 "$RC_PASS" sys/product/TEST/status_reply > "$TMP/topo.out" ) &
sleep 1
pub rc-plus-01 "$RC_PASS" sys/product/TEST/status "$(printf '{"bid": "b1", "tid": "t1",\n "timestamp": 1, "method": "update_topo", "data": {"sub_devices": []}}')"
wait
jq -e '.bid=="b1" and .tid=="t1" and .method=="update_topo" and .data.result==0' "$TMP/topo.out" >/dev/null
check $? "update_topo wird beantwortet, auch nach ungültigem JSON und mit Zeilenumbruch"
# Ein Absturz würde durch restart: unless-stopped verdeckt – daher Neustarts zählen
[ "$(docker inspect -f '{{.RestartCount}}' "$(dc ps -q topo)")" = 0 ]; check $? "Responder ohne Neustart"

# Live-Befehl erreicht das Gerät mit korrektem Format
( sub_one rc-plus-01 "$RC_PASS" thing/product/TEST/services > "$TMP/live.out" ) &
sleep 1
dc run --rm -T topo /tools/live.sh start TEST TEST/39-0-7/normal-0 rtmp://127.0.0.1:1935/live/m350 >/dev/null
wait
jq -e '.method=="live_start_push" and .data.url_type==1 and .data.video_id=="TEST/39-0-7/normal-0" and (.bid|length)>0' "$TMP/live.out" >/dev/null
check $? "live_start_push hat das DJI-Format"

dc up -d media
RTMP_PASS=$(op read op://DRK/suchlage-mqtt/rtmp-pilot)
push() { ffmpeg -loglevel error -re -f lavfi -i testsrc=size=640x360:rate=25 -t "$2" -c:v libx264 -preset ultrafast -g 25 -f flv "$1"; }
# poll <sekunden> <befehl…>: wiederholt, bis der Befehl gelingt
poll() { n=$1; shift; while [ "$n" -gt 0 ]; do "$@" && return 0; n=$((n - 1)); sleep 1; done; return 1; }
# Erst wenn MediaMTX lauscht, beweisen die Ablehnungen unten etwas (sonst scheitert ffmpeg schon am Verbindungsaufbau)
poll 10 curl -s -o /dev/null http://127.0.0.1:8888/; check $? "MediaMTX lauscht"

# Publish ohne bzw. mit falschem Passwort wird abgelehnt (Review Focus 3)
push "rtmp://127.0.0.1:1935/live/smoke" 2 2>/dev/null
[ $? -ne 0 ]; check $? "RTMP ohne Passwort abgelehnt"
push "rtmp://127.0.0.1:1935/live/smoke?user=pilot&pass=falsch" 2 2>/dev/null
[ $? -ne 0 ]; check $? "RTMP mit falschem Passwort abgelehnt"

# Gültiger Publish ist per HLS abrufbar und wird aufgezeichnet
push "rtmp://127.0.0.1:1935/live/smoke?user=pilot&pass=$RTMP_PASS" 15 &
# -L: MediaMTX 1.21 leitet die erste HLS-Anfrage für einen Cookie-Test einmal um (302 auf ?cookieCheck=1)
hls_ok() { curl -sfL http://127.0.0.1:8888/live/smoke/index.m3u8 | grep -q EXTM3U; }
poll 12 hls_ok; check $? "HLS liefert Playlist"
curl -sf -o /dev/null http://127.0.0.1:8889/live/smoke; check $? "WebRTC-Player erreichbar"
wait
sleep 2
ls ../records/video/live/smoke/*.mp4 >/dev/null 2>&1; check $? "Aufzeichnung liegt vor"

dc up -d pilot; sleep 1
curl -sf http://127.0.0.1:8080/ | grep -q "Suchlage"; check $? "Pilot-Seite erreichbar"
curl -sf http://127.0.0.1:8080/config.js | grep -q "tcp://127.0.0.1:1883"; check $? "config.js gerendert"
[ -f pilot/config.js ] && ! grep -q "op://" pilot/config.js; check $? "keine offenen op://-Referenzen"
# Docker Desktop liefert auch 0600-Dateien aus, auf dem vServer gäbe das 403 – daher den Modus direkt prüfen
[ "$(stat -f %Lp pilot/config.js 2>/dev/null || stat -c %a pilot/config.js)" = 644 ]; check $? "config.js für nginx lesbar (0644)"
node --test pilot/ >/dev/null 2>&1; check $? "pilot.js-Unit-Tests"

dc down
exit $FAIL
