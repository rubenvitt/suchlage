# ADR 0004: Geheimnisse in der DB bleiben unverschlüsselt, die Sicherung wird verschlüsselt

- Status: angenommen (29.10.2026, SUL-53)
- Kontext: ADR 0002 legt die fachlichen Einstellungen in die Datenbank und lässt offen, ob deren Geheimnisse at rest verschlüsselt werden (Plan M1, offener Punkt 11). Im Klartext stehen heute:
  - `settings`: OIDC-Client-Secret, DJI-App-Key und Lizenz, Passwort von `suchlage-admin` am Broker, Schlüssel und Admin-Token des Fotospeichers, das interne Geheimnis für MediaMTX (ADR 0003).
  - `remote_controllers`: MQTT-Passwort und Stream-Schlüssel je Fernsteuerung.
  - Nur als Hash: Sitzungen, Großbild-Tokens, Kopplungscodes, API-Tokens der Fernsteuerungen (M2).
- Prüfung gegen das I&K-Sicherheitskonzept (vServer-Konzept der Fachgruppe): Kein Server ist direkt aus dem Internet erreichbar, nur über VPN oder Cloudflare. Je Dienst wird entschieden, was nur im VPN liegt. Jeder Dienst ist zentral dokumentiert, samt Admin-Zugang. Eine Verschlüsselung gespeicherter Daten verlangt das Konzept nicht. Suchlage hält die Regeln ein: Broker, RTMP, WebRTC und S3 lauschen nur an der Tailscale-IP, der Server nur an `127.0.0.1` für Traefik (`deploy/compose.m1.yaml`), die Verwaltung liefert Geheimnisse nie aus (`has_*` in `SettingsView`).
- Entscheidung: Keine Verschlüsselung einzelner Felder in der Anwendung. Geschützt wird die Datenbank als Ganzes:
  1. Auf dem Host liest sie nur, wer root oder in der Gruppe `docker` ist (Docker-Volume, Server als nonroot).
  2. Eine Sicherung verlässt den Host nur verschlüsselt (SUL-107).
  3. Was Suchlage nur prüft, steht weiter nur als Hash in der DB. Klartext bleibt, was Suchlage einem Dritten vorzeigen muss (Pocket ID, DJI, Broker, Garage, MediaMTX).
- Begründung:
  - Der Schlüssel müsste auf demselben Host liegen, als Datei neben der DB oder in der Env (die ADR 0002 nur für Bootstrap zulässt). Wer die DB auf dem Host lesen kann, liest auch den Schlüssel oder den Speicher des Prozesses. Gegen diesen Angreifer hilft die Verschlüsselung nicht.
  - Schützen würde sie nur eine Kopie der DB ohne den Schlüssel, also eine Sicherung. Die ist mit Punkt 2 als Ganzes geschützt.
  - Die DB enthält Wertvolleres als die Geheimnisse: Konten, Einsätze, Flugspuren mit Positionen. Feldverschlüsselung ließe genau das offen.
  - Fast alle Geheimnisse wirken nur im VPN (Broker, RTMP, S3, MediaMTX). Außerhalb nutzbar sind nur das OIDC-Client-Secret (zusammen mit der eingetragenen Redirect-URL) und der DJI-App-Key.
- Verworfen:
  - Feldverschlüsselung mit Schlüssel in Env oder Datei: Aufwand für Migration, Rotation und Wiederherstellung (ohne Schlüssel ist die Sicherung wertlos), kaum Schutzgewinn.
  - SQLCipher für die ganze DB: dasselbe Schlüsselproblem, dazu SQLCipher statt des gebündelten SQLite bauen und Werkzeuge wie `sqlite3` am Host verlieren den Zugriff.
- Folgen:
  - Die Sicherung (`deploy/backup.sh`, Runbook `docs/betrieb/runbook-deployment.md`, Abschnitt „Sicherung“) verschlüsselt jedes Archiv mit `age` an die öffentlichen Schlüssel in `/srv/suchlage/backup-empfaenger.txt`, bevor es auf die Platte kommt (SUL-107, 30.10.2026). Der private Schlüssel liegt nur in 1Password; entschlüsselt wird auf dem eigenen Rechner, nie auf dem Host. Ohne ihn ist jede Sicherung wertlos, deshalb darf die Datei mehrere Empfänger tragen. Ältere, unverschlüsselte Archive meldet das Skript, bis sie gelöscht sind. Außer Haus geht sie ins externe Ziel der IUK-Suite-Sicherung (entschieden 30.10.2026); bis das eingerichtet ist (SUL-118), bleibt sie auf dem Host.
  - Wird die DB oder eine Sicherung bekannt, sind alle Geheimnisse oben zu erneuern. Einen geprüften Weg je Geheimnis gibt es nur für die Fernsteuerungen (löschen und neu koppeln gibt neue MQTT-Passwörter und Stream-Schlüssel). `broker-init.sh` und `fotos-init.sh` brechen beim zweiten Lauf ab. Für den Rest fehlt ein geprüfter Weg, auch im Runbook; SUL-108 trägt ihn nach.
- Neu entscheiden, wenn: der Stack auf dem FüKw-Server läuft (M4, SUL-47; ein Gerät im Fahrzeug kann gestohlen werden, dann Festplattenverschlüsselung des Hosts, nicht der Felder), eine Sicherung außer Haus geht, oder das Sicherheitskonzept eine Regel für gespeicherte Daten bekommt.
