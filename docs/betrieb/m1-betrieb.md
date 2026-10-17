# M1 Betrieb auf dem vServer

> Auslieferung nur über Tailscale (ohne Traefik/Cloudflare): `docs/betrieb/runbook-deployment.md`.

Suchlage M1 läuft als Compose-Projekt `suchlage-m1` mit drei Diensten: `server` (Image
`suchlage:m1`, ohne Feature `dev`), `mqtt` (Mosquitto mit dynamic-security) und `media`
(MediaMTX, Zugriff entscheidet der Server). Der M0-Spike (`deploy/compose.yaml`, Projekt
`suchlage`) bleibt liegen, bis der M0-Feldtest gelaufen ist. Beide Stacks nutzen dieselben Ports
und laufen nie gleichzeitig.

| Dienst | erreichbar an | wofür |
|---|---|---|
| `server` | `127.0.0.1:8080` | Traefik (Oberfläche, API, `/pilot`) |
| `mqtt` | `<Tailscale-IP>:1883` | Fernsteuerungen im VPN |
| `media` | `<Tailscale-IP>:1935`, `<Tailscale-IP>:8189/udp` | RTMP der Fernsteuerung, WebRTC-Medien im VPN |

WHEP (8889), HLS (8888) und Playback (9996) haben keinen Host-Port. Nur der Server erreicht sie über
das Compose-Netz und reicht sie über seine Video-Proxies weiter.

## Einrichtung

1. **M0-Spike stoppen:**

   ```sh
   ssh <vserver> 'cd /srv/suchlage/deploy && docker compose -f compose.yaml down'
   ```

2. **Image bauen und übertragen.** Das Image muss zur Architektur des vServers passen
   (`ssh <vserver> uname -m`). `mise run image` baut für die eigene Maschine, auf Apple Silicon
   also `arm64`. Für einen `x86_64`-vServer stattdessen
   `docker build --platform linux/amd64 -f deploy/Dockerfile -t suchlage:m1 .` (unter Emulation
   deutlich langsamer). Danach:

   ```sh
   docker save suchlage:m1 | ssh <vserver> docker load
   ```

3. **`deploy/` übertragen,** ohne Mitschnitte, ohne `smoke.sh` (überschreibt Geheimnisse und
   löscht Mitschnitte) und ohne `.env` (die Datei auf dem vServer bleibt so erhalten):

   ```sh
   rsync -a --delete --exclude records/ --exclude smoke.sh --exclude .env deploy/ <vserver>:/srv/suchlage/deploy/
   ```

4. **Bootstrap-Werte eintragen.** Compose liest `deploy/.env` auf dem vServer selbst; jeder Aufruf
   von `compose.m1.yaml` braucht beide Werte, auch `logs`, `restart` und `deploy/broker-init.sh`.
   `SUCHLAGE_BIND_IP` steht dort schon, wenn M0 auf dem vServer lief. Ergänzen:

   ```sh
   ssh <vserver> 'echo "SUCHLAGE_PUBLIC_URL=https://<öffentliche Adresse>" >> /srv/suchlage/deploy/.env'
   ```

   Fehlen Datei oder `SUCHLAGE_BIND_IP`, ebenso eintragen
   (`SUCHLAGE_BIND_IP=<Tailscale-IP>`, aus `ssh <vserver> tailscale ip -4`).

5. **Broker einrichten,** einmalig. Das Skript legt den Admin `suchlage-admin` an und gibt sein
   Passwort genau einmal aus. Notieren und erst in Schritt 8 in Suchlage eintragen, sonst nirgends.
   Ein zweiter Aufruf ändert nichts und meldet „bereits eingerichtet“.

   ```sh
   ssh <vserver> /srv/suchlage/deploy/broker-init.sh
   ```

   Ohne diesen Schritt startet `mqtt` bewusst nicht und meldet im Log „Broker nicht eingerichtet:
   deploy/broker-init.sh ausführen“.

6. **Stack starten:**

   ```sh
   ssh <vserver> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml up -d && docker compose -f compose.m1.yaml ps'
   ```

   Danach prüfen, dass nichts an der öffentlichen Schnittstelle lauscht (nur `127.0.0.1:8080` und
   die Tailscale-IP):

   ```sh
   ssh <vserver> "ss -tulnp | grep -E ':(1883|1935|8080|8189)\b'"
   ```

7. **Ersteinrichtung.** Zuerst, **Pflicht:** Den Pocket-ID-Client auf eine I&K-Gruppe beschränken
   (Client → „Allowed User Groups“). Suchlage kennt keine eigene Kontenfreigabe: Jedes Konto, das
   sich über den Client anmelden darf, wird Operator, darf Livebild starten und stoppen und sieht
   das ganze Flugarchiv mit Video. Ohne Beschränkung gilt das für alle Konten der
   Pocket-ID-Instanz, auch die anderer DRK-Dienste. Wer aus der Gruppe entfernt wird, kann sich
   nicht mehr neu anmelden; eine bestehende Sitzung gilt aber bis zu 7 Tage weiter.

   Danach: Der Server schreibt beim ersten Start einen Einmal-Link ins Log (24 h gültig). Link
   öffnen, die Pocket-ID-Daten eintragen und anmelden. Das erste Konto wird Admin.

   ```sh
   ssh <vserver> 'cd /srv/suchlage/deploy && docker compose -f compose.m1.yaml logs server | grep Ersteinrichtung'
   ```

   Ist der Link abgelaufen, erzeugt ein Neustart des Servers einen neuen.

8. **Einstellungen eintragen:**
   - Broker: Broker-Adresse `mqtt`, Port `1883`, Admin-Benutzer `suchlage-admin` mit dem
     Passwort aus Schritt 5, Adresse für die Fernsteuerung `tcp://<Tailscale-IP>:1883`.
   - Video: WebRTC `http://media:8889`, HLS `http://media:8888`, Playback `http://media:9996`,
     RTMP für die Fernsteuerung `rtmp://<Tailscale-IP>:1935`.
   - DJI: die App-Daten aus dem 1Password-Item `zy32h44zpdllk7vttcyi33ldce` von Hand übertragen.

9. **Traefik:** nur `/` nach `127.0.0.1:8080` routen und `/internal/` mit 404 beantworten. Über
   `/internal/media/auth` entscheidet der Server, wer an MediaMTX darf (ADR 0003); dieser Pfad
   darf von außen nie erreichbar sein.

10. **Fernsteuerung anlegen** (Einstellungen → Fernsteuerungen) und den Kopplungslink in Pilot 2
    unter Cloud-Dienst eintragen.

## Betrieb

- **Neues Image einspielen:** Schritt 2, danach
  `docker compose -f compose.m1.yaml up -d server`. Die Daten liegen im Volume `server-data` und
  bleiben erhalten.
- **Daten:** Datenbank und Rohmitschnitte im Volume `server-data` (`/data`), Flugvideos im Volume
  `video`, die Broker-Zugänge im Volume `mqtt-data`. `docker compose -f compose.m1.yaml down -v`
  löscht alle drei und damit jeden aufgezeichneten Flug.
- **Broker neu aufbauen:** Ist `mqtt-data` verloren, Schritt 5 wiederholen und das neue Passwort
  unter Einstellungen → Broker eintragen. Die Zugänge der Fernsteuerungen legt der Server beim
  nächsten Koppeln selbst wieder an.
- **Abnahme:** `docs/betrieb/m1-realflug-checkliste.md`.
