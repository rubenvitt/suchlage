# Runbook: Suchlage ausliefern über Tailscale

Von einem leeren Linux-Host bis zur laufenden Suchlage, ausgeliefert nur im Tailnet. Unabhängig davon,
ob der Host ein vServer, ein Rechner im FüKw oder eine VM ist. Die Oberfläche läuft über
`tailscale serve` mit gültigem HTTPS-Zertifikat. Ein Reverse-Proxy wie Traefik ist nicht nötig,
Cloudflare auch nicht. Nichts lauscht an einer öffentlichen Schnittstelle.

```
Browser (Tailnet) ──HTTPS 443──▶ tailscale serve ──▶ 127.0.0.1:8080  server
Browser (Tailnet) ──UDP 8189──────────────────────▶ <TS-IP>:8189     media (WebRTC-Bild)
RC Plus (Tailnet) ──HTTPS 443 (/pilot)────────────▶ tailscale serve
RC Plus (Tailnet) ──TCP 1883──────────────────────▶ <TS-IP>:1883     mqtt
RC Plus (Tailnet) ──TCP 1935──────────────────────▶ <TS-IP>:1935     media (RTMP)
server ──Compose-Netz──▶ mqtt, media (WHEP 8889, HLS 8888, Playback 9996, nie am Host)
```

## Vorher bereitlegen

| Wert | Woher | Beispiel |
|---|---|---|
| Hostname im Tailnet | frei wählen | `suchlage` |
| Tailnet-Name | Tailscale-Admin → DNS | `tail1234.ts.net` |
| `<URL>` | ergibt sich | `https://suchlage.tail1234.ts.net` |
| `<TS-IP>` | Schritt 2 | `100.x.y.z` |
| Pocket ID: Issuer, Client-ID, Client-Secret | Schritt 7 | |
| DJI: App-ID, App-Key, Lizenz | 1Password, Tresor DRK, Item „DJI Cloud API – Suchlage“ | |
| Workspace-ID | beliebige UUID (`uuidgen`), bleibt danach fest | |

Am Host werden gebraucht: Linux (amd64 oder arm64), Docker Engine mit Compose v2, `rsync`, `openssl`.
Rechne mit Platz für Flugvideos, grob 1–2 GB pro Flugstunde.

## 1. Tailnet vorbereiten (einmalig)

1. **Allow-all-Regel prüfen. Das ist ein Blocker.** Tailscale-ACLs kennen nur Erlauben. Steht noch
   `{"action": "accept", "src": ["*"], "dst": ["*:*"]}` in der Policy, erreichen die RC Plus und der
   Host jedes private Gerät im Tailnet. Diese Regel zuerst durch gezielte Regeln für die bestehenden
   Geräte ersetzen.
2. **MagicDNS und HTTPS-Zertifikate einschalten:** Admin → DNS → *MagicDNS* und *HTTPS Certificates*.
3. **Policy ergänzen** (Admin → Access Controls). `group:iuk` steht für die Personen, die die Lage
   sehen sollen.

   ```jsonc
   "groups": {
     "group:iuk": ["person1@example.org", "person2@example.org"]
   },
   "tagOwners": {
     "tag:suchlage-server": ["autogroup:admin"],
     "tag:suchlage-rc":     ["autogroup:admin"]
   },
   "acls": [
     // Fernsteuerung: Pilot-Seite, MQTT, RTMP – sonst nichts
     {"action": "accept", "src": ["tag:suchlage-rc"], "dst": ["tag:suchlage-server:443,1883,1935"]},
     // Lage im Browser: Oberfläche und WebRTC-Bild (8189 ist UDP)
     {"action": "accept", "src": ["group:iuk"], "dst": ["tag:suchlage-server:443,8189"]},
     // Betrieb per SSH
     {"action": "accept", "src": ["autogroup:admin"], "dst": ["tag:suchlage-server:22"]}
   ]
   ```

   Liegt Pocket ID selbst im Tailnet, braucht der Host zusätzlich eine Regel
   `tag:suchlage-server → <Pocket-ID-Gerät>:443`.

## 2. Host ins Tailnet

```sh
curl -fsSL https://tailscale.com/install.sh | sh
sudo tailscale up --hostname=suchlage --advertise-tags=tag:suchlage-server
tailscale ip -4                      # → <TS-IP>
```

Damit Docker nach einem Neustart auch dann an `<TS-IP>` binden kann, wenn Tailscale noch nicht
verbunden ist, erlaubt der Host das Binden an noch nicht vorhandene Adressen. Ohne diese Einstellung
scheitern die Ports nach einem Reboot mit „cannot assign requested address“, und
`restart: unless-stopped` hilft dagegen nicht.

```sh
echo 'net.ipv4.ip_nonlocal_bind = 1' | sudo tee /etc/sysctl.d/90-suchlage.conf
sudo sysctl --system
```

## 3. Image bauen und auf den Host bringen

Auf dem eigenen Rechner im Repo, Stand `main`:

```sh
git switch main && git pull
ssh <host> uname -m                  # x86_64 → amd64, aarch64 → arm64
TAG=m1-$(git rev-parse --short HEAD)
docker build --platform linux/amd64 -f deploy/Dockerfile -t suchlage:$TAG .
docker save suchlage:$TAG | ssh <host> docker load
ssh <host> docker tag suchlage:$TAG suchlage:m1
```

`suchlage:m1` ist der Name, den Compose startet. Der Tag mit Commit bleibt für einen Rückweg liegen.
Auf Apple Silicon dauert `linux/amd64` unter Emulation deutlich länger. Alternativ das Repo auf dem
Host klonen und dort bauen. Das braucht dort Lesezugriff auf GitHub.

## 4. Deploy-Dateien übertragen

```sh
ssh <host> sudo install -d -o "$USER" /srv/suchlage/deploy
rsync -a --delete --exclude records/ --exclude smoke.sh --exclude .env deploy/ <host>:/srv/suchlage/deploy/
```

Bootstrap-Werte in `/srv/suchlage/deploy/.env` eintragen. Compose liest die Datei bei jedem Aufruf
selbst, auch bei `logs` und `broker-init.sh`. Die Datei enthält keine Geheimnisse:

```sh
ssh <host> 'cat > /srv/suchlage/deploy/.env' <<EOF
SUCHLAGE_BIND_IP=<TS-IP>
SUCHLAGE_PUBLIC_URL=<URL>
EOF
```

Läuft auf dem Host noch der M0-Spike, zuerst
`docker compose -f compose.yaml down` in `/srv/suchlage/deploy`. Beide Stacks nutzen dieselben Ports.

## 5. Broker einrichten und Stack starten

```sh
ssh <host> /srv/suchlage/deploy/broker-init.sh
```

Das Skript gibt das Passwort für `suchlage-admin` **genau einmal** aus. Notieren und in Schritt 8
eintragen, sonst nirgends speichern. Ein zweiter Aufruf ändert nichts.

```sh
ssh <host> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml up -d && docker compose -f compose.m1.yaml ps'
ssh <host> "ss -tulnp | grep -E ':(1883|1935|8080|8189)\b'"
```

In der Ausgabe von `ss` müssen `127.0.0.1:8080` und `<TS-IP>` für 1883, 1935 und 8189 stehen.
`0.0.0.0` und die öffentliche IP dürfen nicht vorkommen.

## 6. Oberfläche über Tailscale ausliefern

```sh
ssh <host> sudo tailscale serve --bg 8080
ssh <host> sudo tailscale serve --bg --set-path /internal 'text:Nicht gefunden'
ssh <host> tailscale serve status
```

Die zweite Zeile ist Pflicht. Über `/internal/media/auth` entscheidet der Server, wer an MediaMTX
darf (ADR 0003). Der Pfad ist nur für MediaMTX im Compose-Netz gedacht und darf aus dem Tailnet
nicht erreichbar sein. Die Konfiguration von `serve` übersteht Neustarts.

Gegenprobe von einem Gerät im Tailnet:

```sh
curl -s <URL>/api/health                                # {"status":"ok","version":"…"}
curl -s -X POST <URL>/internal/media/auth -d '{}'       # „Nicht gefunden“, keine Antwort des Servers
nc -vz <TS-IP> 1883 && nc -vz <TS-IP> 1935
```

Das erste Laden des Zertifikats kann einige Sekunden dauern.

## 7. Ersteinrichtung

1. **Pocket ID. Dieser Schritt ist Pflicht.** Einen OIDC-Client „Suchlage“ anlegen.
   - Callback-URL: `<URL>/api/auth/oidc/callback`
   - **Allowed User Groups:** nur die I&K-Gruppe. Suchlage kennt keine eigene Freigabe. Jedes Konto,
     das sich über den Client anmelden darf, wird Operator. Wer aus der Gruppe fliegt, behält eine
     bestehende Sitzung bis zu 7 Tage.
2. **Einrichtungslink** aus dem Log holen. Er ist 24 h gültig, ein Neustart des Servers erzeugt einen
   neuen:

   ```sh
   ssh <host> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml logs server | grep Ersteinrichtung'
   ```

3. Den Link **vollständig** öffnen, der Teil nach `#` gehört dazu. Issuer, Client-ID und
   Client-Secret eintragen. Meldet die Seite „Anmeldedienst nicht erreichbar“, kommt der Server
   nicht an Pocket ID heran. Dann DNS und ACL des Hosts prüfen. Liegt Pocket ID im Tailnet, die Regel
   aus Schritt 1 ergänzen.
4. Anmelden. Das erste Konto wird Admin.

## 8. Einstellungen in der App

Unter *Einstellungen*:

| Bereich | Feld | Wert |
|---|---|---|
| Broker | Broker-Adresse (vom Server aus) / Port | `mqtt` / `1883` |
| | Admin-Benutzer / Admin-Passwort | `suchlage-admin` / aus Schritt 5 |
| | Adresse für die Fernsteuerung (im VPN) | `tcp://<TS-IP>:1883` |
| Video | WebRTC / HLS / Playback (intern) | `http://media:8889` / `http://media:8888` / `http://media:9996` |
| | RTMP für die Fernsteuerung (im VPN) | `rtmp://<TS-IP>:1935` |
| DJI | App-ID, App-Key, Lizenz | aus 1Password |
| | Workspace-ID | die UUID aus der Vorbereitung |

Für die Fernsteuerung immer die IP eintragen, nicht den MagicDNS-Namen. MQTT und RTMP hängen dann
nicht an der DNS-Auflösung auf der RC Plus.

Danach unter *System* prüfen: Der Broker steht auf „verbunden“.

## 9. Fernsteuerung koppeln

1. RC Plus: Die Tailscale-App installieren und mit Tag `tag:suchlage-rc` anmelden, über einen
   Auth-Key mit diesem Tag aus Admin → Keys. Anschließend in der App „Use Tailscale DNS“ einschalten.
2. Suchlage: *Einstellungen → Fernsteuerungen* → anlegen und den Kopplungslink `<URL>/pilot#<code>`
   kopieren. Über den QR-Code geht es schneller.
3. Pilot 2: Den Link unter *Cloud-Dienst* eintragen. Die Seite meldet „Angemeldet als …“, und die
   Fernsteuerung erscheint in Suchlage.

Behält Pilot 2 den Teil nach `#` nicht, steht nach erneutem Öffnen „Kopplungscode fehlt“. Dann den
Link mit `?code=<code>` statt `#<code>` eintragen. Siehe Realflug-Checkliste.

## 10. Großbild im FüKw (optional)

*Einstellungen → Großbild-Zugänge* → anlegen und den Link auf dem Großbild-Rechner öffnen. Der
Rechner muss im Tailnet sein und unter `group:iuk` oder einer eigenen Regel auf 443 und 8189
zugreifen dürfen.

## Abnahme

`docs/betrieb/m1-realflug-checkliste.md` – erst danach gilt die Auslieferung als fertig.

## Betrieb

**Update.** Schritt 3 mit neuem Stand, danach:

```sh
ssh <host> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml up -d server'
```

Haben sich Dateien in `deploy/` geändert, zuerst das `rsync` aus Schritt 4 laufen lassen und dann
`up -d` für alle Dienste. Die Daten bleiben in den Volumes.

**Rückweg.**

```sh
ssh <host> 'docker image ls suchlage'                     # vorhandene Tags
ssh <host> 'docker tag suchlage:m1-<alter-commit> suchlage:m1 && cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml up -d server'
```

Hat ein neuerer Stand die Datenbank migriert, hilft der Rückweg nur mit der Sicherung von davor.

**Sicherung.** Der Server wird kurz gestoppt, damit SQLite konsistent ist:

```sh
ssh <host> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml stop server &&
  docker run --rm -v suchlage-m1_server-data:/d:ro -v /srv/suchlage/backup:/b busybox:1.37.0-uclibc tar czf /b/server-data-$(date +%F).tgz -C /d . &&
  docker compose -f compose.m1.yaml start server'
```

Was in welchem Volume liegt:
- `suchlage-m1_server-data`: Datenbank mit Einstellungen, Konten und Flügen, dazu Rohmitschnitte.
- `suchlage-m1_video`: Flugvideos.
- `suchlage-m1_mqtt-data`: Broker-Zugänge, lassen sich neu aufbauen (siehe unten).

`docker compose … down -v` löscht alle drei.

**Broker neu aufbauen.** Ist `mqtt-data` verloren, Schritt 5 wiederholen und das neue Passwort
unter *Einstellungen → Broker* eintragen. Die Zugänge der Fernsteuerungen legt der Server beim
nächsten Koppeln wieder an.

**Fehlersuche.**

| Symptom | Ursache / Prüfung |
|---|---|
| `mqtt` startet nicht, Log „Broker nicht eingerichtet“ | Schritt 5 fehlt |
| Ports fehlen nach einem Reboot | `sysctl net.ipv4.ip_nonlocal_bind` muss `1` sein (Schritt 2) |
| Oberfläche lädt, Lage bleibt leer bzw. schraffiert | *System*: Ist der Broker verbunden? Ist die RC gekoppelt? `docker compose … logs server` |
| Livebild nur „verzögert (HLS)“ | Der Browser erreicht `<TS-IP>:8189/udp` nicht. ACL prüfen |
| Kein Bild, RC meldet Push-Fehler | Die RC erreicht `<TS-IP>:1935` nicht. ACL und RTMP-Adresse unter *Einstellungen → Video* prüfen |
| Anmeldung scheitert | Callback-URL in Pocket ID muss genau `<URL>/api/auth/oidc/callback` sein |
