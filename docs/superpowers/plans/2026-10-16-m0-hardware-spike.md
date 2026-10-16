# M0 Hardware-Spike – Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auf dem I&K-vServer läuft im Tailnet ein minimaler Stack (MQTT-Broker mit Mitschnitt, Video-Ingest, Pilot-Anmeldeseite), mit dem ein Testflug der M350 die Fragen F1–F5 der Spec beantwortet.

**Architecture:** Fast nur Konfiguration: Docker Compose mit Mosquitto, einem Mitschnitt-Container (`mosquitto_sub -F %j`), einem Shell-Responder für `update_topo`, MediaMTX und nginx für eine statische Pilot-Seite. Alle Ports binden ausschließlich an die Tailscale-IP des Servers. Geheimnisse liegen in 1Password und werden lokal per `op inject` in ungetrackte Dateien gerendert. Kein Rust in M0 – der Code hier ist Spike-Werkzeug; Compose, Broker- und MediaMTX-Konfiguration übernimmt M1.

**Tech Stack:** Docker Compose, eclipse-mosquitto 2.0.22, bluenviron/mediamtx 1.21.2, nginx 1.30.6-alpine, POSIX-Shell + jq, Vanilla-JS (ES-Module) mit `node:test`, 1Password CLI (`op`), Tailscale.

**Spec:** `docs/superpowers/specs/2026-10-16-suchlage-design.md` (Abschnitte 2, 6, 7 „M0" und „Fragen für M0")

## Global Constraints

- Kein Port des Servers ist direkt aus dem Internet erreichbar; alle Compose-Ports binden an `${SUCHLAGE_BIND_IP}` (Tailscale-IP des vServers, lokal `127.0.0.1`).
- MQTT nur `tcp://` im VPN; Transportverschlüsselung leistet WireGuard.
- Geheimnisse (DJI App-ID/Key/License, MQTT-/RTMP-Passwörter) nie im Repo – nur in 1Password (Tresor `DRK`) und in gerenderten, per `.gitignore` ausgeschlossenen Dateien.
- DJI-Credentials: 1Password-Item `zy32h44zpdllk7vttcyi33ldce` (Felder `username` = App ID, `credential` = App Key, `license` = App Basic License).
- Workspace-ID für Pilot 2: `768d4a2e-1f34-477c-9fcc-872285e895f6`.
- Mitschnitte (`records/`) enthalten Positionen und kommen nicht ins Repo.
- Ziel-Hardware: M350 RTK, RC Plus, DJI Pilot 2. Keine weiteren Modelle.
- Image-Versionen exakt wie im Tech Stack angegeben.
- Deutsche Texte mit echten Umlauten; Bezeichner und Dateinamen ASCII.

## Review Focus

1. **Payload mit Zeilenumbruch oder Leerzeichen** – der Mitschnitt muss jede Nachricht als genau eine JSON-Zeile speichern, der Responder darf nicht zerbrechen. Test in Task 1 (Mitschnitt) und Task 2 (Responder).
2. **Ungültiges JSON auf `sys/product/+/status`** – der Responder läuft weiter und beantwortet die nächste gültige `update_topo`. Test in Task 2.
3. **RTMP-Publish ohne oder mit falschem Passwort** – wird abgelehnt, sonst könnte jeder im Netz einen Stream unterschieben. Test in Task 3.
4. **Broker-Neustart während des Mitschnitts** – Nachrichten danach werden weiter mitgeschnitten (Container startet neu, schreibt in neue Datei, nichts wird überschrieben). Test in Task 1.
5. **Ports landen auf der öffentlichen Schnittstelle** – auf dem vServer lauscht keiner der Ports auf `0.0.0.0` oder der öffentlichen IP. Prüfung in Task 5.

---

## Dateistruktur

```
.gitignore                     # ergänzt: deploy/.env, gerenderte Dateien, records/
deploy/
  compose.yaml                 # mqtt, recorder, topo, media, pilot
  render.sh                    # 1Password → .env, passwd, mediamtx.yml, pilot/config.js
  smoke.sh                     # lokaler Ende-zu-Ende-Test aller Bausteine
  mosquitto/mosquitto.conf
  mosquitto/acl
  tools/Dockerfile             # Mosquitto-Clients + jq
  tools/topo-responder.sh      # beantwortet update_topo
  tools/live.sh                # live_start_push / live_stop_push / live_lens_change
  mediamtx.tpl.yml
  pilot/index.html
  pilot/pilot.js               # Anmeldelogik gegen window.djiBridge
  pilot/pilot.test.mjs
  pilot/config.tpl.js
docs/m0-spike-protokoll.md     # Ablauf und Ergebnisse F1–F5
records/                       # Mitschnitte (ungetrackt)
```

---

### Task 1: MQTT-Broker mit Mitschnitt

**Files:**
- Create: `deploy/compose.yaml`, `deploy/mosquitto/mosquitto.conf`, `deploy/mosquitto/acl`, `deploy/tools/Dockerfile`, `deploy/render.sh`, `deploy/smoke.sh`
- Modify: `.gitignore`

**Interfaces:**
- Produces: Compose-Services `mqtt` (Port 1883) und `recorder`; MQTT-Benutzer `suchlage` (alles) und `rc-plus-01` (nur `thing/product/#`, `sys/product/#`); Env `SUCHLAGE_BIND_IP`, `MQTT_TOOL_PASS` in `deploy/.env`; 1Password-Item `op://DRK/suchlage-mqtt` mit Feldern `suchlage`, `rc-plus-01`, `rtmp-pilot`; Mitschnitt als JSONL unter `records/<UTC-Start>.jsonl`; Hilfsfunktionen `check`, `sub_one`, `pub` in `smoke.sh`.

- [ ] **Step 1: 1Password-Item für Spike-Passwörter anlegen**

```bash
op item create --vault DRK --category password --title suchlage-mqtt \
  "suchlage[password]=$(openssl rand -hex 24)" \
  "rc-plus-01[password]=$(openssl rand -hex 24)" \
  "rtmp-pilot[password]=$(openssl rand -hex 24)" \
  --tags suchlage >/dev/null
op read op://DRK/suchlage-mqtt/rc-plus-01 | wc -c   # erwartet: 49
```

- [ ] **Step 2: `.gitignore` ergänzen**

Anhängen:

```
deploy/.env
deploy/mosquitto/passwd
deploy/mediamtx.yml
deploy/pilot/config.js
records/
```

- [ ] **Step 3: Broker-Konfiguration schreiben**

`deploy/mosquitto/mosquitto.conf`:

```
listener 1883 0.0.0.0
allow_anonymous false
password_file /mosquitto/config/passwd
acl_file /mosquitto/config/acl
persistence false
log_dest stdout
```

`deploy/mosquitto/acl`:

```
# Spike: Seriennummern sind vor dem ersten Flug unbekannt, daher noch keine Einschränkung je Gerät (kommt in M1).
user suchlage
topic readwrite #

user rc-plus-01
topic readwrite thing/product/#
topic readwrite sys/product/#
```

`deploy/tools/Dockerfile`:

```dockerfile
FROM eclipse-mosquitto:2.0.22
RUN apk add --no-cache jq
COPY *.sh /tools/
ENTRYPOINT []
```

- [ ] **Step 4: Compose mit Broker und Mitschnitt**

`deploy/compose.yaml`:

```yaml
name: suchlage

services:
  mqtt:
    image: eclipse-mosquitto:2.0.22
    ports: ["${SUCHLAGE_BIND_IP:?}:1883:1883"]
    volumes: ["./mosquitto:/mosquitto/config:ro"]
    restart: unless-stopped

  recorder:
    build: ./tools
    depends_on: [mqtt]
    environment: { MQTT_TOOL_PASS: "${MQTT_TOOL_PASS:?}" }
    # %j: eine JSON-Zeile je Nachricht inkl. Zeitstempel, Payload als escapter String; neue Datei je Start, nichts wird überschrieben
    command: ["sh", "-c", "exec mosquitto_sub -h mqtt -u suchlage -P \"$$MQTT_TOOL_PASS\" -t 'thing/#' -t 'sys/#' -F '%j' >> /records/$$(date -u +%Y%m%dT%H%M%SZ).jsonl"]
    volumes: ["../records:/records"]
    restart: unless-stopped
```

- [ ] **Step 5: Render-Skript (MQTT-Teil)**

`deploy/render.sh`:

```sh
#!/bin/sh
# Rendert Geheimnisse aus 1Password in ungetrackte Deploy-Dateien. Lokal ausführen, nie auf dem Server.
set -eu
cd "$(dirname "$0")"
: "${SUCHLAGE_BIND_IP:?Tailscale-IP des vServers, lokal 127.0.0.1}"
ITEM=op://DRK/suchlage-mqtt
TOOL_PASS=$(op read "$ITEM/suchlage")
RC_PASS=$(op read "$ITEM/rc-plus-01")

printf 'SUCHLAGE_BIND_IP=%s\nMQTT_TOOL_PASS=%s\n' "$SUCHLAGE_BIND_IP" "$TOOL_PASS" > .env
docker run --rm -v "$PWD/mosquitto:/m" eclipse-mosquitto:2.0.22 sh -c \
  "mosquitto_passwd -b -c /m/passwd suchlage $TOOL_PASS && mosquitto_passwd -b /m/passwd rc-plus-01 $RC_PASS && chmod 0644 /m/passwd"
```

`chmod +x deploy/render.sh`

- [ ] **Step 6: Smoke-Test schreiben (MQTT-Teil)**

`deploy/smoke.sh`:

```sh
#!/bin/sh
# Lokaler Ende-zu-Ende-Test des Spike-Stacks. Startet den Stack auf 127.0.0.1 und prüft jeden Baustein.
set -u
cd "$(dirname "$0")"
export SUCHLAGE_BIND_IP=127.0.0.1
./render.sh
dc() { docker compose "$@"; }
FAIL=0
check() { if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAIL $2"; FAIL=1; fi; }
TOOL_PASS=$(op read op://DRK/suchlage-mqtt/suchlage)
RC_PASS=$(op read op://DRK/suchlage-mqtt/rc-plus-01)
# sub_one <user> <pass> <topic>: erste Nachricht (max. 5 s) ausgeben
sub_one() { dc exec -T mqtt mosquitto_sub -h localhost -u "$1" -P "$2" -t "$3" -C 1 -W 5; }
# pub <user> <pass> <topic> <payload>
pub() { dc exec -T mqtt mosquitto_pub -h localhost -u "$1" -P "$2" -t "$3" -m "$4"; }

rm -rf ../records && mkdir -p ../records
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
( sub_one suchlage "$TOOL_PASS" foo/bar > /tmp/suchlage-acl.out 2>&1 ) &
sleep 1; pub rc-plus-01 "$RC_PASS" foo/bar verboten; wait
[ ! -s /tmp/suchlage-acl.out ]; check $? "ACL blockt fremde Topics"

# Broker-Neustart: Mitschnitt läuft weiter (Review Focus 4)
dc restart mqtt; sleep 3
pub rc-plus-01 "$RC_PASS" thing/product/TEST/osd '{"after":"restart"}'
sleep 2
grep -q 'after.*restart' ../records/*.jsonl; check $? "Mitschnitt überlebt Broker-Neustart"

# --- weitere Prüfungen folgen in Task 2–4 ---

dc down
exit $FAIL
```

`chmod +x deploy/smoke.sh`

- [ ] **Step 7: Smoke-Test laufen lassen**

Run: `deploy/smoke.sh`
Expected: alle Zeilen `ok`, Exit 0. Falls „Mitschnitt überlebt Broker-Neustart" scheitert: `mosquitto_sub` endet beim Verbindungsabbruch, `restart: unless-stopped` startet ihn neu – dann im Test nach `dc restart mqtt` zusätzlich `sleep 5` und erneut prüfen; scheitert es weiter, `dc logs recorder` ansehen.

- [ ] **Step 8: Commit**

```bash
git add .gitignore deploy/compose.yaml deploy/mosquitto deploy/tools/Dockerfile deploy/render.sh deploy/smoke.sh
git commit -m "M0: MQTT-Broker mit Mitschnitt und Smoke-Test"
```

---

### Task 2: Topologie-Antwort und Live-Befehle

**Files:**
- Create: `deploy/tools/topo-responder.sh`, `deploy/tools/live.sh`
- Modify: `deploy/compose.yaml` (Service `topo`), `deploy/smoke.sh`

**Interfaces:**
- Consumes: `mqtt`, Benutzer `suchlage`, `MQTT_TOOL_PASS`, `sub_one`/`pub`/`check` aus `smoke.sh`.
- Produces: Service `topo` (antwortet auf `sys/product/{gw}/status` mit `update_topo` auf `…/status_reply`); Werkzeug `docker compose run --rm topo /tools/live.sh start|stop|lens <gateway_sn> <video_id> [rtmp_url|lens]`.

- [ ] **Step 1: Failing Tests in `smoke.sh` ergänzen** (vor der Zeile `# --- weitere Prüfungen folgen`)

```sh
dc up -d --build topo; sleep 2

# Ungültiges JSON bricht den Responder nicht (Review Focus 2), gültiges update_topo wird beantwortet
pub rc-plus-01 "$RC_PASS" sys/product/TEST/status 'kein json'
( sub_one rc-plus-01 "$RC_PASS" sys/product/TEST/status_reply > /tmp/suchlage-topo.out ) &
sleep 1
pub rc-plus-01 "$RC_PASS" sys/product/TEST/status '{"bid":"b1","tid":"t1","timestamp":1,"method":"update_topo","data":{"sub_devices":[]}}'
wait
jq -e '.bid=="b1" and .tid=="t1" and .method=="update_topo" and .data.result==0' /tmp/suchlage-topo.out >/dev/null
check $? "update_topo wird beantwortet, auch nach ungültigem JSON"

# Live-Befehl erreicht das Gerät mit korrektem Format
( sub_one rc-plus-01 "$RC_PASS" thing/product/TEST/services > /tmp/suchlage-live.out ) &
sleep 1
dc run --rm -T topo /tools/live.sh start TEST TEST/39-0-7/normal-0 rtmp://127.0.0.1:1935/live/m350
wait
jq -e '.method=="live_start_push" and .data.url_type==1 and .data.video_id=="TEST/39-0-7/normal-0" and (.bid|length)>0' /tmp/suchlage-live.out >/dev/null
check $? "live_start_push hat das DJI-Format"
```

- [ ] **Step 2: Test laufen lassen, Fehlschlag sehen**

Run: `deploy/smoke.sh`
Expected: Fehler beim Bauen bzw. `FAIL update_topo …` und `FAIL live_start_push …` (Skripte fehlen).

- [ ] **Step 3: Responder schreiben**

`deploy/tools/topo-responder.sh`:

```sh
#!/bin/sh
# Bestätigt die Geräte-Topologie (update_topo), die Pilot 2 nach dem MQTT-Login meldet.
set -u
mosquitto_sub -h mqtt -u suchlage -P "$MQTT_TOOL_PASS" -t 'sys/product/+/status' -F '%j' |
while IFS= read -r line; do
  # %j liefert topic + payload als String; ungültiges JSON im Payload wird übersprungen
  reply=$(printf '%s' "$line" | jq -c '
    .topic as $t | (.payload | fromjson? // empty)
    | select(.method == "update_topo")
    | {topic: ($t + "_reply"), msg: {bid, tid, timestamp: (now * 1000 | floor), method, data: {result: 0}}}' 2>/dev/null) || continue
  [ -n "$reply" ] || continue
  printf '%s' "$reply" | jq -c .msg |
    mosquitto_pub -h mqtt -u suchlage -P "$MQTT_TOOL_PASS" -t "$(printf '%s' "$reply" | jq -r .topic)" -s
done
```

- [ ] **Step 4: Live-Werkzeug schreiben**

`deploy/tools/live.sh`:

```sh
#!/bin/sh
# Spike-Werkzeug: Livestream-Befehle an Pilot 2.
#   live.sh start <gateway_sn> <video_id> <rtmp_url>
#   live.sh stop  <gateway_sn> <video_id>
#   live.sh lens  <gateway_sn> <video_id> <normal|thermal|wide|zoom>
set -eu
cmd=$1 gw=$2 vid=$3
case $cmd in
  start) method=live_start_push
         data=$(jq -nc --arg u "$4" --arg v "$vid" '{url_type: 1, url: $u, video_id: $v, video_quality: 0}') ;;
  stop)  method=live_stop_push
         data=$(jq -nc --arg v "$vid" '{video_id: $v}') ;;
  lens)  method=live_lens_change
         data=$(jq -nc --arg v "$vid" --arg t "$4" '{video_id: $v, video_type: $t}') ;;
  *) echo "Unbekannter Befehl: $cmd" >&2; exit 2 ;;
esac
jq -nc --arg m "$method" --arg b "$(cat /proc/sys/kernel/random/uuid)" --arg t "$(cat /proc/sys/kernel/random/uuid)" \
  --argjson d "$data" '{bid: $b, tid: $t, timestamp: (now * 1000 | floor), method: $m, data: $d}' |
  mosquitto_pub -h mqtt -u suchlage -P "$MQTT_TOOL_PASS" -t "thing/product/$gw/services" -s
echo "gesendet: $method an $gw – Antwort im Mitschnitt unter thing/product/$gw/services_reply"
```

`chmod +x deploy/tools/*.sh`

- [ ] **Step 5: Service in `compose.yaml` ergänzen**

```yaml
  topo:
    build: ./tools
    depends_on: [mqtt]
    environment: { MQTT_TOOL_PASS: "${MQTT_TOOL_PASS:?}" }
    command: ["/tools/topo-responder.sh"]
    restart: unless-stopped
```

- [ ] **Step 6: Tests laufen lassen**

Run: `deploy/smoke.sh`
Expected: alle Zeilen `ok`, Exit 0.

- [ ] **Step 7: Commit**

```bash
git add deploy/tools deploy/compose.yaml deploy/smoke.sh
git commit -m "M0: update_topo-Responder und Live-Befehle"
```

---

### Task 3: Video-Ingest mit MediaMTX

**Files:**
- Create: `deploy/mediamtx.tpl.yml`
- Modify: `deploy/compose.yaml` (Service `media`), `deploy/render.sh`, `deploy/smoke.sh`

**Interfaces:**
- Consumes: 1Password-Feld `op://DRK/suchlage-mqtt/rtmp-pilot`, `SUCHLAGE_BIND_IP`.
- Produces: RTMP-Ingest `rtmp://<BIND_IP>:1935/live/<name>?user=pilot&pass=<rtmp-pilot>`; WebRTC-Player `http://<BIND_IP>:8889/live/<name>`; HLS `http://<BIND_IP>:8888/live/<name>/index.m3u8`; Aufzeichnung unter `records/video/live/<name>/`.

- [ ] **Step 1: Failing Tests in `smoke.sh` ergänzen** (vor `# --- weitere Prüfungen folgen`)

```sh
dc up -d media; sleep 2
RTMP_PASS=$(op read op://DRK/suchlage-mqtt/rtmp-pilot)
push() { ffmpeg -loglevel error -re -f lavfi -i testsrc=size=640x360:rate=25 -t "$2" -c:v libx264 -preset ultrafast -g 25 -f flv "$1"; }

# Publish ohne bzw. mit falschem Passwort wird abgelehnt (Review Focus 3)
push "rtmp://127.0.0.1:1935/live/smoke" 2 2>/dev/null
[ $? -ne 0 ]; check $? "RTMP ohne Passwort abgelehnt"
push "rtmp://127.0.0.1:1935/live/smoke?user=pilot&pass=falsch" 2 2>/dev/null
[ $? -ne 0 ]; check $? "RTMP mit falschem Passwort abgelehnt"

# Gültiger Publish ist per HLS abrufbar und wird aufgezeichnet
push "rtmp://127.0.0.1:1935/live/smoke?user=pilot&pass=$RTMP_PASS" 12 &
sleep 8
curl -sf http://127.0.0.1:8888/live/smoke/index.m3u8 | grep -q EXTM3U; check $? "HLS liefert Playlist"
curl -sf -o /dev/null http://127.0.0.1:8889/live/smoke; check $? "WebRTC-Player erreichbar"
wait
sleep 2
ls ../records/video/live/smoke/*.mp4 >/dev/null 2>&1; check $? "Aufzeichnung liegt vor"
```

- [ ] **Step 2: Test laufen lassen, Fehlschlag sehen**

Run: `deploy/smoke.sh`
Expected: `no such service: media` bzw. `FAIL` bei den Video-Prüfungen.

- [ ] **Step 3: MediaMTX-Vorlage**

`deploy/mediamtx.tpl.yml`:

```yaml
logLevel: info
rtsp: no
srt: no
api: no
rtmp: yes
rtmpAddress: :1935
hls: yes
hlsAddress: :8888
webrtc: yes
webrtcAddress: :8889
webrtcLocalUDPAddress: :8189
webrtcAdditionalHosts: [__BIND_IP__]

authMethod: internal
authInternalUsers:
  - user: any
    pass:
    ips: []
    permissions:
      - action: read
      - action: playback
  - user: pilot
    pass: "{{ op://DRK/suchlage-mqtt/rtmp-pilot }}"
    ips: []
    permissions:
      - action: publish

pathDefaults:
  record: yes
  recordFormat: fmp4
  recordPath: /recordings/%path/%Y-%m-%d_%H-%M-%S-%f

paths:
  all_others:
```

- [ ] **Step 4: Render-Skript ergänzen** (ans Ende von `deploy/render.sh`)

```sh
op inject -f -i mediamtx.tpl.yml -o mediamtx.yml >/dev/null
sed -i.bak "s/__BIND_IP__/$SUCHLAGE_BIND_IP/g" mediamtx.yml && rm -f mediamtx.yml.bak
```

- [ ] **Step 5: Service in `compose.yaml` ergänzen**

```yaml
  media:
    image: bluenviron/mediamtx:1.21.2
    ports:
      - "${SUCHLAGE_BIND_IP:?}:1935:1935"
      - "${SUCHLAGE_BIND_IP:?}:8888:8888"
      - "${SUCHLAGE_BIND_IP:?}:8889:8889"
      - "${SUCHLAGE_BIND_IP:?}:8189:8189/udp"
    volumes:
      - ./mediamtx.yml:/mediamtx.yml:ro
      - ../records/video:/recordings
    restart: unless-stopped
```

- [ ] **Step 6: Tests laufen lassen**

Run: `deploy/smoke.sh`
Expected: alle Zeilen `ok`. Scheitert „RTMP ohne Passwort abgelehnt", prüfen, ob `authInternalUsers` die `any`-Zeile ohne `publish` hat; `docker compose logs media` zeigt die Auth-Entscheidung.

- [ ] **Step 7: Commit**

```bash
git add deploy/mediamtx.tpl.yml deploy/compose.yaml deploy/render.sh deploy/smoke.sh
git commit -m "M0: Video-Ingest mit MediaMTX, Auth und Aufzeichnung"
```

---

### Task 4: Pilot-Anmeldeseite

**Files:**
- Create: `deploy/pilot/index.html`, `deploy/pilot/pilot.js`, `deploy/pilot/pilot.test.mjs`, `deploy/pilot/config.tpl.js`
- Modify: `deploy/compose.yaml` (Service `pilot`), `deploy/render.sh`, `deploy/smoke.sh`

**Interfaces:**
- Consumes: DJI-Item `zy32h44zpdllk7vttcyi33ldce`, `op://DRK/suchlage-mqtt/rc-plus-01`, `…/rtmp-pilot`, Workspace-ID, `SUCHLAGE_BIND_IP`.
- Produces: `connect(cfg, bridge, log)` und `startManualLive(cfg, bridge, log)` in `pilot.js`; Seite unter `http://<BIND_IP>:8080/`.

DJI-JSBridge-Referenz (aus `dji-sdk/Cloud-API-Doc`, `60.api-reference/10.pilot-to-cloud/30.jsbridge.md`): Jede Funktion liefert einen JSON-String `{code, message, data}`, `code === 0` heißt Erfolg. `platformLoadComponent('thing', {host: 'tcp://…', username, password, connectCallback})`, `platformLoadComponent('liveshare', {videoPublishType, statusCallback})`; Callbacks werden als Name einer globalen Funktion übergeben. `liveshareSetConfig(2, '{"url": "rtmp://…"}')` (Typ 2 = RTMP), danach `liveshareStartLive()`.

- [ ] **Step 1: Failing Test schreiben**

`deploy/pilot/pilot.test.mjs`:

```js
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
```

- [ ] **Step 2: Test laufen lassen, Fehlschlag sehen**

Run: `node --test deploy/pilot/`
Expected: FAIL mit `Cannot find module …/pilot.js`.

- [ ] **Step 3: `pilot.js` schreiben**

```js
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
```

- [ ] **Step 4: Test laufen lassen**

Run: `node --test deploy/pilot/`
Expected: 3 Tests PASS.

- [ ] **Step 5: Seite und Konfigurationsvorlage**

`deploy/pilot/index.html`:

```html
<!doctype html>
<html lang="de">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Suchlage – Pilot (Spike)</title>
  <style>
    body { font: 16px system-ui, sans-serif; margin: 16px; background: #111; color: #eee; }
    button { font-size: 18px; padding: 12px 20px; margin: 0 8px 8px 0; }
    #log { white-space: pre-wrap; font: 13px ui-monospace, monospace; background: #000; padding: 8px; }
  </style>
</head>
<body>
  <h1>Suchlage – Spike M0</h1>
  <button id="connect">Verbinden</button>
  <button id="live">Livestream manuell starten</button>
  <div id="log"></div>
  <script type="module">
    import cfg from "./config.js";
    import { connect, startManualLive } from "./pilot.js";
    const out = document.getElementById("log");
    const log = (m) => { out.textContent += `${new Date().toLocaleTimeString("de-DE")} ${m}\n`; };
    window.onThingConnect = (state) => log(`MQTT verbunden: ${state}`);
    window.onLiveStatus = (status) => log(`Live-Status: ${status}`);
    const run = (fn) => { try { fn(cfg, window.djiBridge, log); } catch (e) { log(`Abbruch bei ${e.message}`); } };
    if (!window.djiBridge) log("Kein djiBridge – Seite in DJI Pilot 2 öffnen.");
    document.getElementById("connect").onclick = () => run(connect);
    document.getElementById("live").onclick = () => run(startManualLive);
    log(`RC: ${window.djiBridge?.platformGetRemoteControllerSN?.() ?? "-"}, Fluggerät: ${window.djiBridge?.platformGetAircraftSN?.() ?? "-"}`);
  </script>
</body>
</html>
```

`deploy/pilot/config.tpl.js`:

```js
export default {
  appId: "{{ op://DRK/zy32h44zpdllk7vttcyi33ldce/username }}",
  appKey: "{{ op://DRK/zy32h44zpdllk7vttcyi33ldce/credential }}",
  license: "{{ op://DRK/zy32h44zpdllk7vttcyi33ldce/license }}",
  workspaceId: "768d4a2e-1f34-477c-9fcc-872285e895f6",
  mqttHost: "tcp://__BIND_IP__:1883",
  mqttUser: "rc-plus-01",
  mqttPass: "{{ op://DRK/suchlage-mqtt/rc-plus-01 }}",
  rtmpUrl: "rtmp://__BIND_IP__:1935/live/m350?user=pilot&pass={{ op://DRK/suchlage-mqtt/rtmp-pilot }}",
};
```

- [ ] **Step 6: Render, Compose, Smoke ergänzen**

Ans Ende von `deploy/render.sh`:

```sh
op inject -f -i pilot/config.tpl.js -o pilot/config.js >/dev/null
sed -i.bak "s/__BIND_IP__/$SUCHLAGE_BIND_IP/g" pilot/config.js && rm -f pilot/config.js.bak
```

In `deploy/compose.yaml`:

```yaml
  pilot:
    image: nginx:1.30.6-alpine
    ports: ["${SUCHLAGE_BIND_IP:?}:8080:80"]
    volumes: ["./pilot:/usr/share/nginx/html:ro"]
    restart: unless-stopped
```

In `deploy/smoke.sh` (vor `# --- weitere Prüfungen folgen`):

```sh
dc up -d pilot; sleep 1
curl -sf http://127.0.0.1:8080/ | grep -q "Suchlage"; check $? "Pilot-Seite erreichbar"
curl -sf http://127.0.0.1:8080/config.js | grep -q "tcp://127.0.0.1:1883"; check $? "config.js gerendert"
! grep -q "op://" pilot/config.js; check $? "keine offenen op://-Referenzen"
node --test pilot/ >/dev/null 2>&1; check $? "pilot.js-Unit-Tests"
```

- [ ] **Step 7: Alles laufen lassen**

Run: `deploy/smoke.sh`
Expected: alle Zeilen `ok`, Exit 0.

- [ ] **Step 8: Datenschutz-Check und Commit**

```bash
"$DATENSCHUTZ_CHECK" .   # Datenschutz-Check des dev-projekt-Skills (Exit 1 blockt)
git status --short   # config.js, mediamtx.yml, passwd, .env dürfen NICHT auftauchen
git add deploy/pilot/index.html deploy/pilot/pilot.js deploy/pilot/pilot.test.mjs deploy/pilot/config.tpl.js deploy/compose.yaml deploy/render.sh deploy/smoke.sh
git commit -m "M0: Pilot-Anmeldeseite mit JSBridge-Ablauf"
git push
```

---

### Task 5: vServer und RC Plus ins Tailnet, Deploy

Manuelle Schritte mit Ruben; kein neuer Code. Ergebnis wird im Protokoll (Task 6) festgehalten.

**Interfaces:**
- Consumes: `deploy/` komplett, `render.sh`.
- Produces: laufender Stack unter `<vserver-tailscale-ip>`; RC Plus im Tailnet mit Tag `tag:suchlage-rc`.

- [ ] **Step 0: Aktuelle Tailnet-Policy prüfen – Blocker**

Tailscale-ACLs kennen nur Erlauben, kein Verbieten. Steht in der Policy noch die Default-Regel `{"action": "accept", "src": ["*"], "dst": ["*:*"]}`, erreichen RC Plus und vServer (beide von anderen mitbenutzt) alle privaten Geräte im Tailnet und umgekehrt. Außerdem liefert Port 8080 `config.js` mit App Key, License und Passwörtern an jeden, der ihn erreicht.

Erst weiter, wenn die Allow-all-Regel durch gezielte Regeln für die bestehenden Geräte (inkl. Exit-Node-Nutzung) ersetzt ist. Wie sie ersetzt wird, entscheidet Ruben.

- [ ] **Step 1: Tailnet-ACL ergänzen** (Tailscale-Admin → Access Controls)

```jsonc
"tagOwners": {
  "tag:suchlage-server": ["autogroup:admin"],
  "tag:suchlage-rc":     ["autogroup:admin"]
},
"acls": [
  // RC Plus darf nur den Suchlage-Server erreichen, und nur MQTT, RTMP und die Pilot-Seite
  {"action": "accept", "src": ["tag:suchlage-rc"], "dst": ["tag:suchlage-server:1883,1935,8080"]},
  // eigene Geräte dürfen zuschauen und verwalten
  {"action": "accept", "src": ["autogroup:member"], "dst": ["tag:suchlage-server:*"]}
]
```

Bestehende Regeln nicht entfernen, nur ergänzen. Mit der Prüfung aus Step 0 greift die Isolation der RC Plus tatsächlich.

- [ ] **Step 2: vServer ins Tailnet**

```bash
ssh <vserver> 'curl -fsSL https://tailscale.com/install.sh | sh && sudo tailscale up --advertise-tags=tag:suchlage-server --ssh=false'
ssh <vserver> 'tailscale ip -4'   # → SUCHLAGE_BIND_IP notieren
```

- [ ] **Step 3: Rendern und ausrollen**

```bash
cd ~/dev/personal/drk/suchlage
SUCHLAGE_BIND_IP=<ip> deploy/render.sh
rsync -a --delete --exclude records/ deploy/ <vserver>:/srv/suchlage/deploy/
ssh <vserver> 'mkdir -p /srv/suchlage/records && cd /srv/suchlage/deploy && docker compose up -d --build && docker compose ps'
SUCHLAGE_BIND_IP=127.0.0.1 deploy/render.sh   # lokale Dateien wieder auf Testwerte
```

- [ ] **Step 4: Ports prüfen (Review Focus 5)**

```bash
ssh <vserver> "ss -tulnp | grep -E ':(1883|1935|8080|8888|8889|8189)\b'"
```
Expected: jede Zeile bindet an `<ip>` (100.x), keine an `0.0.0.0`, `[::]` oder die öffentliche IP.

Neustart-Probe: `ssh <vserver> 'sudo reboot'`, danach `docker compose ps` und die `ss`-Zeile erneut. Startet Docker vor `tailscaled`, scheitern die Port-Bindungen mit „cannot assign requested address" und `restart: unless-stopped` hilft nicht. Dann einen Systemd-Override setzen:

```bash
ssh <vserver> 'sudo mkdir -p /etc/systemd/system/docker.service.d && printf "[Unit]\nAfter=tailscaled.service\nWants=tailscaled.service\n" | sudo tee /etc/systemd/system/docker.service.d/after-tailscale.conf && sudo systemctl daemon-reload'
```

und die Probe wiederholen.

- [ ] **Step 5: Gegenprobe aus dem Tailnet (Mac)**

```bash
curl -sf http://<ip>:8080/ | grep -q Suchlage && echo ok
ffmpeg -loglevel error -re -f lavfi -i testsrc=size=640x360:rate=25 -t 15 -c:v libx264 -preset ultrafast -g 25 -f flv \
  "rtmp://<ip>:1935/live/probe?user=pilot&pass=$(op read op://DRK/suchlage-mqtt/rtmp-pilot)" &
sleep 5; open "http://<ip>:8889/live/probe"; wait
```
Expected: Testbild im Browser per WebRTC.

- [ ] **Step 6: RC Plus ins Tailnet**

Tailscale-APK auf der RC Plus installieren (Play Store, falls vorhanden; sonst APK von pkgs.tailscale.com per USB). Gerät im Admin-Panel mit Tag `tag:suchlage-rc` versehen, Key-Ablauf deaktivieren. In Pilot 2: Cloud-Dienst → Open Platform → URL `http://<ip>:8080/`.

---

### Task 6: Testflug und Protokoll

**Files:**
- Create: `docs/m0-spike-protokoll.md`

- [ ] **Step 1: Protokollvorlage anlegen**

`docs/m0-spike-protokoll.md`:

```markdown
# M0 Hardware-Spike – Protokoll

- Datum / Ort:
- Aufbau: M350 RTK (FW …), RC Plus (FW …), Pilot 2 (Version …), Tailscale (Version …)
- Netz: LTE über … / WLAN …

## F1 Lizenzprüfung
Die Lizenzprüfung läuft in Pilot 2 und braucht unseren Server nicht. Für den Offline-Test reicht ein Laptop, der `deploy/pilot/` ausliefert (`cd deploy/pilot && python3 -m http.server 8080`), in einem WLAN ohne Uplink (Hotspot mit abgeschaltetem Mobilfunk). Reihenfolge einhalten, um „braucht Internet" von „merkt sich die erste Online-Prüfung" zu trennen:

- [ ] 1. Mit Internet: „Verbinden" → `platformVerifyLicense: ok`?
- [ ] 2. RC Plus neu starten, ins WLAN ohne Uplink, Seite unter `http://<laptop-ip>:8080/` öffnen, „Verbinden" → Ergebnis?
- [ ] 3. Falls 2 klappt: Pilot-2-Daten löschen bzw. App neu installieren ist **nicht** nötig – Ergebnis „funktioniert nach einmaliger Online-Prüfung" reicht für die Entscheidung.
- Ergebnis:

## F2 Tailscale auf der RC Plus
- [ ] App läuft parallel zu Pilot 2, Verbindung bleibt im Flug stabil
- [ ] `onThingConnect` meldet Verbindung, `sys/product/<rc-sn>/status` im Mitschnitt, Antwort `status_reply`
- Ergebnis:

## F3 Telemetrie
- [ ] Mitschnittdatei: `records/<…>.jsonl`, Dauer, Anzahl Nachrichten je Topic (`jq -r .topic records/X.jsonl | sort | uniq -c`)
- [ ] Felder für Position, Höhen, Geschwindigkeit, Heading, Akku, Flugmodus, Flugzustand gefunden (Pfad im JSON notieren)
- [ ] Frequenz von `osd`
- Ergebnis / Rust-oder-TS-Einschätzung:

## F4 Livestream
- [ ] Manuell aus der Seite („Livestream manuell starten") → Bild unter `http://<ip>:8889/live/m350`
- [ ] Vom Server: `docker compose run --rm topo /tools/live.sh start <rc-sn> <video_id aus live_capacity im state> rtmp://<ip>:1935/live/m350?user=pilot&pass=…`
- [ ] Linsenwechsel: `live.sh lens … thermal` / `zoom` / `wide` – wechselt das Bild? Antwort im Mitschnitt?
- [ ] RTMPS: in Pilot 2 eine `rtmps://`-URL anbieten möglich? (nur prüfen, nicht einrichten)
- Ergebnis:

## F5 Verzögerung
- [ ] Stoppuhr auf Handy vor die Kamera, Browser daneben filmen; 10 Messungen
- Einzelwerte / Median / Maximum:

## Entscheidung
- [ ] Cloud API trägt → M1 planen
- [ ] Rust bleibt / Wechsel auf TS (Begründung aus F3)
- [ ] Rückfall nötig (RTMP ohne Telemetrie / HDMI)
```

- [ ] **Step 2: Commit**

```bash
git add docs/m0-spike-protokoll.md
git commit -m "M0: Protokollvorlage für den Testflug"
git push
```

- [ ] **Step 3: Testflug durchführen** – mit Pilot und Luftbeobachter nach Operating Manual, Protokoll ausfüllen.

- [ ] **Step 4: Mitschnitte sichern** – `rsync <vserver>:/srv/suchlage/records/ ~/dev/personal/drk/suchlage-records/` (außerhalb des Repos). Ein bereinigter Ausschnitt (Positionen verschoben) kommt erst in M1 als Test-Fixture ins Repo.

- [ ] **Step 5: Protokoll committen** – `git add docs/m0-spike-protokoll.md && git commit -m "M0: Ergebnisse Testflug" && git push`
