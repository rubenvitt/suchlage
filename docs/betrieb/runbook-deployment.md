# Runbook: Suchlage ausliefern unter suchlage.iuk-ue.de

Von einem leeren Linux-Host bis zur laufenden Suchlage. Die Oberfläche ist öffentlich unter
`https://suchlage.iuk-ue.de` erreichbar, hinter dem vorhandenen Traefik bzw. Cloudflare-Tunnel des
Hosts. Die Anmeldung läuft über Pocket ID. Der Server selbst lauscht nur an `127.0.0.1` und ist nie
direkt aus dem Internet erreichbar.

**Diese Dienste bleiben im Tailnet und dürfen nie öffentlich werden:**

| Port | Dienst | Warum nur im Tailnet |
|---|---|---|
| TCP 1883 | MQTT (`mqtt`) | Die RC Plus spricht MQTT ohne TLS. Zugangsdaten und Telemetrie gingen im Klartext. |
| TCP 1935 | RTMP (`media`) | Das Livebild kommt unverschlüsselt von der RC, mit Stream-Schlüssel in der URL. |
| TCP 3900 | S3 des Fotospeichers (`fotos`, Garage) | Pilot 2 lädt Fotos direkt hoch, ohne TLS, mit einem Schlüssel, der nach einer Stunde abläuft. |
| UDP 8189 | WebRTC-Medien (`media`) | WebRTC mit 1–2 s Verzögerung nur für Geräte im Tailnet. Wer von außen kommt, bekommt HLS (5–10 s) über die Oberfläche. |
| HTTP `/internal/` | Medien-Freigabe des Servers | Über `/internal/media/auth` entscheidet der Server, wer an MediaMTX darf (ADR 0003). Der Pfad ist nur für MediaMTX im Compose-Netz gedacht. |

```
Browser (Internet) ──HTTPS 443──▶ Traefik / cloudflared ──▶ 127.0.0.1:8080  server
Browser (Tailnet)  ──UDP 8189──────────────────────────────▶ <TS-IP>:8189     media (WebRTC-Bild)
RC Plus (Internet) ──HTTPS 443 (/pilot, /map/api/v1, /media/api/v1, /storage/api/v1, WebSocket /pilot/ws)──▶ Traefik / cloudflared
Flugmappe (Browser) ─HTTPS 443 (/api/flugmappe/v1, Zugriffstoken aus Pocket ID)──▶ Traefik / cloudflared
RC Plus (Tailnet)  ──TCP 1883──────────────────────────────▶ <TS-IP>:1883     mqtt
RC Plus (Tailnet)  ──TCP 1935──────────────────────────────▶ <TS-IP>:1935     media (RTMP)
RC Plus (Tailnet)  ──TCP 3900──────────────────────────────▶ <TS-IP>:3900     fotos (S3, Garage)
server ──Compose-Netz──▶ mqtt, media (WHEP 8889, HLS 8888, Playback 9996, nie am Host), fotos (S3 3900, Admin 3903)
```

## Vorher bereitlegen

| Wert | Woher | Beispiel |
|---|---|---|
| `<URL>` | fest | `https://suchlage.iuk-ue.de` |
| DNS-Eintrag | Cloudflare → DNS bzw. Tunnel | `suchlage.iuk-ue.de` |
| Hostname im Tailnet | frei wählen | `suchlage` |
| `<TS-IP>` | Schritt 2 | `100.x.y.z` |
| Pocket ID: Issuer, Client-ID, Client-Secret | Schritt 7 | |
| DJI: App-ID, App-Key, Lizenz | 1Password, Tresor DRK, Item „DJI Cloud API – Suchlage“ | |
| Workspace-ID | beliebige UUID (`uuidgen`), bleibt danach fest | |

Am Host werden gebraucht: Linux (amd64 oder arm64), Docker Engine mit Compose v2, `rsync`, `openssl`,
dazu Traefik oder `cloudflared` mit gültigem Zertifikat für `suchlage.iuk-ue.de`.
Rechne mit Platz für Flugvideos, grob 1–2 GB pro Flugstunde.

## 1. Tailnet vorbereiten (einmalig)

Das Tailnet trägt nur noch MQTT, RTMP und WebRTC. Die Oberfläche und die Pilot-Seite laufen über
das Internet.

1. **Allow-all-Regel prüfen. Das ist ein Blocker.** Tailscale-ACLs kennen nur Erlauben. Steht noch
   `{"action": "accept", "src": ["*"], "dst": ["*:*"]}` in der Policy, erreichen die RC Plus und der
   Host jedes private Gerät im Tailnet. Diese Regel zuerst durch gezielte Regeln für die bestehenden
   Geräte ersetzen.
2. **Policy ergänzen** (Admin → Access Controls). `group:iuk` steht für die Personen, die das Bild
   mit geringer Verzögerung über WebRTC sehen sollen.

   ```jsonc
   "groups": {
     "group:iuk": ["person1@example.org", "person2@example.org"]
   },
   "tagOwners": {
     "tag:suchlage-server": ["autogroup:admin"],
     "tag:suchlage-rc":     ["autogroup:admin"]
   },
   "acls": [
     // Fernsteuerung: MQTT, RTMP und Foto-Upload (M3), sonst nichts
     {"action": "accept", "src": ["tag:suchlage-rc"], "dst": ["tag:suchlage-server:1883,1935,3900"]},
     // WebRTC-Bild im Browser (8189 ist UDP)
     {"action": "accept", "src": ["group:iuk"], "dst": ["tag:suchlage-server:8189"]},
     // Betrieb per SSH
     {"action": "accept", "src": ["autogroup:admin"], "dst": ["tag:suchlage-server:22"]}
   ]
   ```

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

## 3. Image auf den Host bringen

Jeder Push auf `main` ist ein Release (Workflow *Release*, `.github/workflows/release.yml`): Er
vergibt eine Version, baut das Image, legt es in GHCR ab und erstellt dazu Git-Tag und GitHub-Release
`v<Version>` mit den gemergten PRs als Notizen.

- `ghcr.io/rubenvitt/suchlage:<Version>`, etwa `0.2.3`: der Name zum Ausliefern.
- `ghcr.io/rubenvitt/suchlage:m1-<kurzer Commit>` bleibt wie bisher für einen Rückweg liegen.
- `ghcr.io/rubenvitt/suchlage:m1` zeigt auf den neuesten Stand von `main`.

Die Version ist `MAJOR.MINOR` aus `Cargo.toml`, `PATCH` zählt pro Release hoch. Für einen neuen
Meilenstein in `Cargo.toml` die Minor-Version anheben (`0.2.0` → `0.3.0`). Jeder Tag enthält
`linux/amd64` und `linux/arm64`. `docker pull` holt am Host von selbst die passende Architektur.
Von Hand neu bauen: *Actions → Release → Run workflow*. Auf `main` erzeugt das kein zweites Release,
der Commit behält seine Version.

Die neueste Version steht unter *Releases*, lokal auch `git ls-remote --tags --refs --sort=-v:refname origin 'v*' | head -n1`:

```sh
TAG=<Version>
ssh <host> "docker pull ghcr.io/rubenvitt/suchlage:$TAG && docker tag ghcr.io/rubenvitt/suchlage:$TAG suchlage:$TAG && docker tag suchlage:$TAG suchlage:m1"
```

`suchlage:m1` ist der Name, den Compose startet. Der Tag mit Version bleibt für einen Rückweg liegen.
Was läuft, zeigt die Systemseite unter *Server* oder `curl -s https://suchlage.iuk-ue.de/api/health`.

Ist das Paket in GHCR privat (*Package settings → Danger Zone → Change visibility*), meldet sich der
Host vorher einmal an. Dafür reicht ein Token, das nur `read:packages` darf:
`ssh <host> docker login ghcr.io -u <github-user>`, das Token als Passwort eingeben.

## 4. Deploy-Dateien übertragen

```sh
ssh <host> sudo install -d -o "$USER" /srv/suchlage/deploy
rsync -a --delete --exclude records/ --exclude smoke.sh --exclude .env deploy/ <host>:/srv/suchlage/deploy/
```

Die Bootstrap-Werte kommen nach `/srv/suchlage/deploy/.env`. Compose liest die Datei bei jedem Aufruf
selbst, auch bei `logs` und `broker-init.sh`. Geheimnisse stehen darin nicht:

```sh
ssh <host> 'cat > /srv/suchlage/deploy/.env' <<EOF
SUCHLAGE_BIND_IP=<TS-IP>
SUCHLAGE_PUBLIC_URL=https://suchlage.iuk-ue.de
EOF
```

Läuft auf dem Host noch der M0-Spike, zuerst
`docker compose -f compose.yaml down` in `/srv/suchlage/deploy` ausführen. Beide Stacks nutzen
dieselben Ports.

## 5. Broker einrichten und Stack starten

```sh
ssh <host> /srv/suchlage/deploy/broker-init.sh
```

Das Skript gibt das Passwort für `suchlage-admin` **genau einmal** aus. Notieren und in Schritt 8
eintragen, sonst nirgends speichern. Ein zweiter Aufruf ändert nichts.

Danach den Fotospeicher (M3):

```sh
ssh <host> /srv/suchlage/deploy/fotos-init.sh
```

Das Skript legt das RPC-Geheimnis von Garage unter `/srv/suchlage/garage-secrets` an (neben
`deploy/`, damit `rsync --delete` es nicht löscht), startet `fotos` und legt Bucket
`suchlage-fotos`, den Server-Schlüssel und einen Admin-Token an, der nur Schlüssel verwalten darf.
Schlüssel-ID, geheimen Schlüssel und Admin-Token gibt es **genau einmal** aus. Notieren und in
Schritt 8 eintragen, sonst nirgends speichern. Ein zweiter Aufruf ändert nichts.

```sh
ssh <host> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml up -d && docker compose -f compose.m1.yaml ps'
ssh <host> "ss -tulnp | grep -E ':(1883|1935|3900|8080|8189)\b'"
```

In der Ausgabe von `ss` müssen `127.0.0.1:8080` und `<TS-IP>` für 1883, 1935, 3900 und 8189 stehen.
`0.0.0.0` und die öffentliche IP dürfen nicht vorkommen.

## 6. Oberfläche öffentlich ausliefern

Nach außen geht nur `suchlage.iuk-ue.de` → `127.0.0.1:8080`. `/internal/` bleibt draußen. Die
Antworten werden nicht komprimiert, sonst puffert Cloudflare das Live-Lagebild (SSE, ADR 0001).

**Mit Cloudflare-Tunnel** (`cloudflared` am Host). Die Regeln in `config.yml` stehen vor dem
abschließenden `http_status:404`:

```yaml
ingress:
  - hostname: suchlage.iuk-ue.de
    path: ^/internal(/|$)
    service: http_status:404
  - hostname: suchlage.iuk-ue.de
    service: http://127.0.0.1:8080
  # …
  - service: http_status:404
```

Danach `sudo systemctl restart cloudflared` und
`cloudflared tunnel route dns <tunnel> suchlage.iuk-ue.de`, falls der DNS-Eintrag noch fehlt.

**Mit Traefik** (File-Provider). `/internal/` passt auf keinen Router, Traefik antwortet dort mit 404.
Keine `compress`-Middleware:

```yaml
http:
  routers:
    suchlage:
      rule: Host(`suchlage.iuk-ue.de`) && !PathPrefix(`/internal`)
      entryPoints: [websecure]
      service: suchlage
      tls:
        certResolver: <resolver>
  services:
    suchlage:
      loadBalancer:
        servers:
          - url: http://127.0.0.1:8080
```

Läuft Traefik selbst als Container, erreicht er `127.0.0.1` des Hosts nicht. Dann hängt eine
Override-Datei am Host, die nicht ins Repo kommt, den Server zusätzlich ins Traefik-Netz, und die
Service-URL wird `http://suchlage-m1-server-1:8080`:

```yaml
# /srv/suchlage/deploy/compose.traefik.yaml
services:
  server:
    networks: [default, traefik]
networks:
  traefik:
    external: true
    name: <Netz von Traefik>
```

Alle `docker compose`-Aufrufe brauchen dann `-f compose.m1.yaml -f compose.traefik.yaml`.

Gegenprobe von außen, ohne Tailscale:

```sh
curl -s https://suchlage.iuk-ue.de/api/health                     # {"status":"ok","version":"…","commit":"…"}
curl -s -o /dev/null -w '%{http_code}\n' -X POST https://suchlage.iuk-ue.de/internal/media/auth -d '{}'   # 404
nc -vz -w 3 <öffentliche IP> 1883                                 # muss scheitern
```

Und aus dem Tailnet: `nc -vz <TS-IP> 1883 && nc -vz <TS-IP> 1935`.

## 7. Ersteinrichtung

1. **Pocket ID. Dieser Schritt ist Pflicht.** Einen OIDC-Client „Suchlage“ anlegen.
   - Callback-URL: `https://suchlage.iuk-ue.de/api/auth/oidc/callback`
   - **Allowed User Groups:** nur die I&K-Gruppe. Die Oberfläche ist öffentlich, deshalb ist diese
     Gruppe die einzige Schranke. Suchlage kennt keine eigene Freigabe. Jedes Konto, das sich über
     den Client anmelden darf, wird Operator. Wer aus der Gruppe fliegt, behält eine bestehende
     Sitzung bis zu 7 Tage.
2. **Einrichtungslink** aus dem Log holen. Er ist 24 h gültig, ein Neustart des Servers erzeugt einen
   neuen. Wer den Link hat, richtet die Instanz ein. Ihn deshalb sofort benutzen:

   ```sh
   ssh <host> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml logs server | grep Ersteinrichtung'
   ```

3. Den Link **vollständig** öffnen, der Teil nach `#` gehört dazu. Issuer, Client-ID und
   Client-Secret eintragen. Meldet die Seite „Anmeldedienst nicht erreichbar“, kommt der Server
   nicht an Pocket ID heran. Dann DNS und ausgehende Verbindungen des Hosts prüfen.
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
| Fotospeicher | S3-Adresse (vom Server aus) / Admin-Adresse | `http://fotos:3900` / `http://fotos:3903` |
| | S3-Adresse für die Fernsteuerung (im VPN) | `http://<TS-IP>:3900` |
| | Bucket / Region | `suchlage-fotos` / `garage` |
| | Schlüssel-ID, geheimer Schlüssel, Admin-Token | aus Schritt 5 (`fotos-init.sh`) |

Für die Fernsteuerung immer die Tailscale-IP eintragen, nie `suchlage.iuk-ue.de` und nicht den
MagicDNS-Namen. MQTT, RTMP und S3 gibt es nur im Tailnet und hängen so nicht an der DNS-Auflösung
auf der RC Plus.

Danach unter *System* prüfen: Der Broker steht auf „verbunden“.

## 9. Fernsteuerung koppeln

1. RC Plus: Die Tailscale-App installieren und mit Tag `tag:suchlage-rc` anmelden, über einen
   Auth-Key mit diesem Tag aus Admin → Keys. Die RC braucht zusätzlich Internet (LTE oder WLAN)
   für die Pilot-Seite.
2. Suchlage: *Einstellungen → Fernsteuerungen* → anlegen und den Kopplungslink
   `https://suchlage.iuk-ue.de/pilot#<code>` kopieren. Über den QR-Code geht es schneller.
3. Pilot 2: Den Link unter *Cloud-Dienst* eintragen. Die Seite meldet „Angemeldet als …“, und die
   Fernsteuerung erscheint in Suchlage.

Behält Pilot 2 den Teil nach `#` nicht, steht nach erneutem Öffnen „Kopplungscode fehlt“. Dann den
Link mit `?code=<code>` statt `#<code>` eintragen. Siehe Realflug-Checkliste.

## 10. Großbild im FüKw (optional)

*Einstellungen → Großbild-Zugänge* → anlegen und den Link auf dem Großbild-Rechner öffnen. Ohne
Tailnet zeigt das Großbild das Livebild über HLS. Für WebRTC muss der Rechner im Tailnet sein und
unter `group:iuk` oder einer eigenen Regel auf UDP 8189 zugreifen dürfen.

## 11. Flugmappe verknüpfen (optional)

Die Flugmappe (`uav.iuk-ue.de`) legt über `/api/flugmappe/v1/…` Einsätze in Suchlage an, verknüpft
sie, schickt Abschnitte und Kartenfotos und liest Flüge und Karte. Sie meldet sich dabei mit ihrem
eigenen Zugriffstoken aus **derselben** Pocket ID an, die Suchlage für die Anmeldung nutzt (Schritt 7).

1. **Pocket ID:** Die Flugmappe hat dort ihren eigenen OIDC-Client (öffentlicher Client ohne Secret,
   PKCE). Für Suchlage braucht er nichts Neues. Seine **Client-ID** notieren, nicht die von Suchlage.
2. **Suchlage:** *Einstellungen → Flugmappe*
   - **Client-ID:** die Client-ID der Flugmappe aus Schritt 1. Suchlage nimmt nur Zugriffstokens an,
     die für diesen Client ausgestellt sind.
   - **Adresse der Flugmappe:** ihr Ursprung, nur Schema, Host und Port, ohne Pfad:
     `https://uav.iuk-ue.de`. Nur von dort erlaubt Suchlage Aufrufe aus dem Browser (CORS).
   - *Flugmappe speichern.* Der Zustand springt auf „an“. *Ausschalten* (oder ein leeres Formular)
     schaltet die Schnittstelle wieder aus; jeder Aufruf der Flugmappe bekommt dann 503.
3. **Konten:** Wer aus der Flugmappe mit Suchlage arbeitet, braucht in Suchlage die Rolle Operator
   oder Verwaltung und muss sich **einmal in Suchlage angemeldet** haben. Suchlage legt über die
   Schnittstelle keine Konten an; ein unbekanntes Konto bekommt 403.

Schritt 6 deckt den Pfad schon ab. Gegenprobe von außen:

```sh
curl -s -o /dev/null -w '%{http_code}\n' https://suchlage.iuk-ue.de/api/flugmappe/v1/me   # 401 (an), 503 (aus)
curl -s -o /dev/null -w '%{http_code}\n' -X OPTIONS https://suchlage.iuk-ue.de/api/flugmappe/v1/me \
  -H 'Origin: https://uav.iuk-ue.de' -H 'Access-Control-Request-Method: GET'                # 204
```

In Suchlage stehen solche Einsätze mit „aus der Flugmappe“ bzw. „mit Flugmappe verknüpft“ (Name,
Zeit) in der Einsatzliste und in der Kopfleiste. Abschnitte legt bei ihnen nur die Flugmappe an;
Kartenfotos der Abschnitte zeigt die Einsatzliste, beim laufenden Einsatz das Menü in der Kopfleiste
(*Kartenfotos …*).

## Abnahme

`docs/betrieb/m1-realflug-checkliste.md`. Erst danach gilt die Auslieferung als fertig. Für die
Kartenelemente (M2) zusätzlich `docs/betrieb/m2-realflug-checkliste.md`, für die Fotos (M3)
`docs/betrieb/m3-realflug-checkliste.md`.

Pilot 2 erreicht die Kartenelemente über `/map/api/v1/…` (HTTPS) und `/pilot/ws` (WebSocket), beides
unter `https://suchlage.iuk-ue.de` wie `/pilot`. Die Regeln aus Schritt 6 decken das ab; Traefik und
Cloudflare-Tunnel reichen WebSockets ohne weitere Einstellung durch. Zugang nur mit dem API-Token,
den die Pilot-Seite beim Koppeln bekommt.

Die Fotos (M3) laufen über denselben Token: `/media/api/v1/…` und `/storage/api/v1/…` unter
`https://suchlage.iuk-ue.de`, ebenfalls von den Regeln aus Schritt 6 erfasst. Die Bilder selbst lädt
Pilot 2 direkt nach `http://<TS-IP>:3900`. Der Browser bekommt Vorschaubild und Original nur über
den Server, auch mobil ohne Tailnet.

## Betrieb

**Update.** Schritt 3 mit neuem Stand, danach:

```sh
ssh <host> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml up -d server'
```

Ab 0.4.0 läuft die Migration 0004 (Einsätze) beim Start des Servers von selbst. Alle vorhandenen
Flüge, Kartenelemente und Fotos erscheinen danach unter „ohne Einsatz“; die Migration selbst
verliert nichts (zum Aufräumen ohne Einsatz siehe die Warnung unten). Vor dem Update trotzdem sichern (siehe „Rückweg“).

Ab 0.5.0 läuft die Migration 0005 (Flugmappe) ebenso von selbst. Vorhandene Einsätze gelten danach
als in Suchlage angelegt und sind mit keiner Flugmappe verknüpft. Die Schnittstelle bleibt aus, bis
sie unter *Einstellungen → Flugmappe* gesetzt ist (Schritt 11).

**Achtung ab dem Stand mit „Aufräumen ohne Einsatz“ (SUL-78):** Kurz nach dem Start löscht der Server
alle Kartenelemente ohne Einsatz, die seit mehr als 24 h nicht geändert wurden, auch die aus 0.4.0
übernommenen. Wer solche Zeichnungen behalten will, ordnet sie vor dem Update einem Einsatz zu (Lage,
„Einsatz zuordnen“).

Haben sich Dateien in `deploy/` geändert, zuerst das `rsync` aus Schritt 4 laufen lassen und dann
`up -d` für alle Dienste. Die Daten bleiben in den Volumes.

**Rückweg.**

```sh
ssh <host> 'docker image ls suchlage'                     # vorhandene Tags
ssh <host> 'docker tag suchlage:<alte Version> suchlage:m1 && cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml up -d server'
```

Fehlt der alte Tag am Host, holt ihn Schritt 3 aus GHCR. Hat ein neuerer Stand die Datenbank
migriert, hilft der Rückweg nur mit der Sicherung von davor.

**Sicherung.** Server und Fotospeicher werden kurz gestoppt, damit beide SQLite-Datenbanken
(Suchlage und die Metadaten von Garage) konsistent sind. Nicht während eines Flugs:

```sh
ssh <host> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml stop server fotos &&
  for v in server-data fotos-meta fotos-data; do
    docker run --rm -v suchlage-m1_$v:/d:ro -v /srv/suchlage/backup:/b busybox:1.37.0-uclibc tar czf /b/$v-$(date +%F).tgz -C /d .
  done &&
  docker run --rm -v /srv/suchlage/garage-secrets:/d:ro -v /srv/suchlage/backup:/b busybox:1.37.0-uclibc tar czf /b/garage-secrets-$(date +%F).tgz -C /d . &&
  docker compose -f compose.m1.yaml start fotos server'
```

Die Flugvideos (`video`) sind groß und fehlen deshalb in der Schleife. Wer sie mitsichern will,
nimmt `video` dazu und stoppt zusätzlich `media`.

Was in welchem Volume liegt:
- `suchlage-m1_server-data`: Datenbank mit Einstellungen, Konten, Einsätzen, Flügen und
  Kartenelementen, dazu Rohmitschnitte.
- `suchlage-m1_fotos-meta`, `suchlage-m1_fotos-data`: Fotospeicher (Garage), nur zusammen
  brauchbar. Das RPC-Geheimnis liegt außerhalb unter `/srv/suchlage/garage-secrets`.
- `suchlage-m1_video`: Flugvideos.
- `suchlage-m1_mqtt-data`: Broker-Zugänge, lassen sich neu aufbauen (siehe unten).

`docker compose … down -v` löscht alle fünf.

**Videoablage gemeinsam.** Der Server hängt `suchlage-m1_video` unter `/data/video` ein und löscht
dort die Aufzeichnungen gelöschter Flüge. Dafür läuft MediaMTX als derselbe Nutzer wie der Server
(UID 65532, `user:` in `compose.m1.yaml`). Vor jedem Start übergibt der Einmal-Dienst `video-init`
die Ablage diesem Nutzer, auch ältere Aufzeichnungen, die MediaMTX noch als root geschrieben hat.
Von Hand ist dafür nichts zu tun. Fehlt die Ablage am Server, scheitert das Löschen eines Flugs, seines
Videos oder eines Einsatzes mit Video mit 503 „Videoablage nicht eingehängt“; gelöscht ist dann nichts,
ein zweiter Versuch nach dem Einhängen räumt alles. Dasselbe gilt für Fotos, wenn der Fotospeicher
(Garage) nicht erreichbar ist.

**Aufräumen ohne Einsatz.** Alle 10 Minuten löscht der Server Kartenelemente ohne Einsatz, die seit
mehr als 24 h nicht geändert wurden. Browser und Pilot 2 erfahren es sofort. Flüge und Fotos ohne
Einsatz bleiben, damit man sie später einem Einsatz zuordnen kann. Löschen nur `admin`: Flüge einzeln
im Flugdetail, Fotos dort oder auf der Lage unter „ohne Einsatz“ (Foto auf der Karte wählen,
„Foto löschen …“) – auch Fotos ohne Flug. Einsätze, Flüge, Videos und Fotos löscht nur ein Mensch, nie der Server
von selbst.

**Broker neu aufbauen.** Ist `mqtt-data` verloren, Schritt 5 wiederholen und das neue Passwort
unter *Einstellungen → Broker* eintragen. Die Zugänge der Fernsteuerungen legt der Server beim
nächsten Koppeln wieder an.

**Fehlersuche.**

| Symptom | Ursache / Prüfung |
|---|---|
| `mqtt` startet nicht, Log „Broker nicht eingerichtet“ | Schritt 5 fehlt |
| Ports fehlen nach einem Reboot | `sysctl net.ipv4.ip_nonlocal_bind` muss `1` sein (Schritt 2) |
| `suchlage.iuk-ue.de` meldet 502/504 | Traefik bzw. `cloudflared` erreicht `127.0.0.1:8080` nicht. Läuft Traefik als Container, siehe Schritt 6 |
| Oberfläche lädt, Lage bleibt leer bzw. schraffiert | *System*: Ist der Broker verbunden? Ist die RC gekoppelt? `docker compose … logs server`. Hängt nur das Live-Lagebild, Kompression im Proxy abschalten |
| Livebild nur „verzögert (HLS)“ | Außerhalb des Tailnets erwartet. Im Tailnet: Der Browser erreicht `<TS-IP>:8189/udp` nicht, ACL prüfen |
| Kein Bild, RC meldet Push-Fehler | Die RC erreicht `<TS-IP>:1935` nicht. Tailscale auf der RC, ACL und RTMP-Adresse unter *Einstellungen → Video* prüfen |
| Pilot-Seite lädt nicht auf der RC | Die RC hat kein Internet. `/pilot` läuft über `suchlage.iuk-ue.de`, nicht über das Tailnet |
| Kartenelemente fehlen in Pilot 2, Protokoll der Pilot-Seite ohne „Kartenabgleich: verbunden“ | Cloud-Seite in Pilot 2 neu öffnen (lädt `api`, `ws`, `map`). Meldet Pilot 2 „Anmeldung abgelaufen“, wurde dieselbe Fernsteuerung woanders neu gekoppelt: jedes Koppeln erzeugt einen neuen Token |
| Fotos kommen nicht an, Pilot-Seite zeigt bei „Fotos“ „Server ohne Fotospeicher“ | *Einstellungen → Fotospeicher* fehlt. Danach die Cloud-Seite in Pilot 2 neu öffnen (lädt `media`) |
| Pilot 2 lädt Fotos nicht hoch, Funkprotokoll „Foto-Upload gescheitert“ | Die RC erreicht `<TS-IP>:3900` nicht: ACL (Schritt 1) und *S3-Adresse für die Fernsteuerung* prüfen. `docker compose … logs fotos` |
| `sts` antwortet 503 „Fotospeicher lehnt ab“ | Admin-Token oder Admin-Adresse falsch, oder der Bucket heißt anders. `docker compose … logs server` nennt den HTTP-Status von Garage |
| Foto ohne Vorschaubild | Der Server-Schlüssel darf nicht lesen (`garage bucket info suchlage-fotos` zeigt `RW` für `suchlage-server`) oder das Foto ist kein JPEG |
| `fotos` startet nicht | `/srv/suchlage/garage-secrets/rpc_secret` fehlt oder ist für andere lesbar: `fotos-init.sh` ausführen bzw. `chmod 600` |
| Pilot 2 meldet „Ressource existiert nicht“ beim Zeichnen | Die Workspace-ID unter *Einstellungen → DJI* wurde geändert. Zurückändern oder in Pilot 2 neu koppeln |
| Anmeldung scheitert | Callback-URL in Pocket ID muss genau `https://suchlage.iuk-ue.de/api/auth/oidc/callback` sein |
