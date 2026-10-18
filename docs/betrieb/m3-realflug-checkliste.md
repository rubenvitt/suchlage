# M3 Fotos – Abnahme im Feld

Abnahme laut Spec §7: Ein Wärmebildfoto liegt nach dem Flug an der richtigen Stelle auf der Karte.
Abweichungen als Kommentar an die Aufgabe „Feldabnahme M3“ auf dem Board (SUL-64). Kann im selben
Termin wie die Feldabnahme M2 (SUL-45) laufen.

**Vorher:**

1. Neuen Stand ausliefern (Runbook, „Betrieb → Update“), dazu einmalig `fotos-init.sh` (Runbook,
   Schritt 5), die Tailnet-ACL um Port 3900 erweitern (Schritt 1) und *Einstellungen →
   Fotospeicher* ausfüllen (Schritt 8). Danach `docker compose -f compose.m1.yaml up -d`.
2. In Pilot 2 die Cloud-Seite von Suchlage einmal neu öffnen. Erst dieses Koppeln lädt das Modul
   `media`. Im Protokoll der Seite stehen danach `platformLoadComponent: ok` für `media` und
   „Foto-Upload: automatisch (Original)“, in der Zeile „Fotos“ steht „automatischer Upload“.

**Abnahme:**

- [ ] Im Flug mit der H20N mindestens drei Fotos auslösen, eines davon mit Blick auf eine bekannte
      Stelle (Fahrzeug, Wegkreuzung). Zeitpunkt und Stelle notieren.
- [ ] Nach der Landung, spätestens wenige Minuten danach: Die Fotos erscheinen auf der Lagekarte
      als Ring, am Arbeitsplatz, auf dem Großbild und mobil. Zeit bis zur Anzeige notieren.
- [ ] Ein Klick auf den Ring zeigt zuerst das **Wärmebild**, der Umschalter wechselt zu Weit und
      Zoom. Liegt der Ring an der notierten Stelle (Abweichung in Metern notieren)?
- [ ] Flugdetail: Die Fotos hängen am Flug (Leiste „Fotos“) und liegen auf der Spur.
- [ ] Original öffnen und herunterladen funktioniert am Arbeitsplatz und mobil ohne Tailnet.

**Feldfragen (Plan M3, „Offene Punkte“):**

- [ ] Lädt Pilot 2 schon **während** des Flugs hoch oder erst nach der Landung?
- [ ] Wie oft fragt Pilot 2 `sts` (je Foto oder je Stapel)? Unter *System → Funkprotokoll* bzw.
      im Server-Log: Ein Upload-Schlüssel je Fernsteuerung und Stunde ist das Ziel.
- [ ] Akzeptiert Pilot 2 das leere `security_token` und `provider: minio` mit `http://`-Endpunkt?
      Falls nicht: Fehlermeldung in Pilot 2 und im Protokoll notieren. Rückfälle stehen im Plan
      (beliebiger Token-Wert, Garage ignoriert ihn; `provider: aws`).
- [ ] Welche Dateien kommen je Auslösung an (Endungen `_T`, `_W`, `_Z`, weitere?) und wie heißen sie?
- [ ] Was steht in `created_time` (Format, Zeitzone)? Stimmt die Uhrzeit im Vorschaubild?
- [ ] Ist `shoot_position` eine Kommazahl? Die Abweichung oben beantwortet das mit.

Ergebnisse, die den Plan ändern (Upload-Zeitpunkt, Zeitformat, Rückfall beim Token), gehören in
`docs/superpowers/plans/2026-10-17-m3-fotos.md` unter „Offene Punkte“.
