# Suchlage – Design

- **Stand:** 16.10.2026, aus dem Brainstorming zum PRD v0.1 (Arbeitstitel „Fährte")
- **Status:** Entwurf zur Durchsicht

## 1. Zweck

Die I&K-Gruppe kann das Bild ihrer Drohne heute nicht übertragen. Suchlage schließt diese Lücke ohne Lizenzkosten: Livebild, Position und Flugspur der M350 RTK erscheinen im Browser, im FüKw, am Arbeitsplatz und mobil. Suchlage ersetzt damit für uns DJI FlightHub 2.

Personensuche bleibt das Zielbild (Sektoren, Abdeckung, Hinweise, Export für die Polizei). Es baut auf einer funktionierenden Übertragung auf und kommt danach.

**Erfolg:** Ein Flug mit der M350, Bild und Position sind im FüKw und auf dem Handy live zu sehen, der Flug ist danach vollständig nachvollziehbar, ohne laufende Kosten und ohne den Entwickler als Bediener.

## 2. Entscheidungen

| Thema | Entscheidung | Grund |
|---|---|---|
| Übertragungsweg | DJI Cloud API, selbst betrieben | liefert Bild **und** Telemetrie, Basis für Kartenelemente, Fotos und spätere Suche |
| Zielgerät | nur M350 RTK mit RC Plus und DJI Pilot 2 | Einsatzgerät mit Wärmebild (H20N); die M200 kann zu wenig |
| Betriebsort | zuerst I&K-vServer, später Server im FüKw (M4) | öffentliches TLS-Zertifikat, LTE ist im FüKw verbaut |
| Codebasis | schlank neu; `uav-lagebild` nur als Nachschlagewerk | dort fehlt der DJI-Teil, der Rest ist Ballast |
| Backend | Rust (axum, sqlx/SQLite, rumqttc, openidconnect, rust-embed) | gleicher Stack wie Lifeline Hub |
| UI | neu gedacht, Designsystem aus Lifeline Hub (antd 6 + Tokens, MapLibre, `@einsatzzeichen/maplibre`, terra-draw) | Wiederverwendung statt Neuerfindung |
| Hauptansicht | Arbeitsplatz; Großbild und mobil leiten sich ab | dort wird bedient |
| Anmeldung | Pocket ID (wie IDA); Großbild per eigenem Zugangstoken | vorhandene Identität |
| Verhältnis zu Lifeline Hub | eigenständig, später ggf. Schnittstelle | Lifeline Hub ist ein eigenes Produkt |
| Verhältnis zur Flugmappe (`uav-checklists`) | getrennte Systeme, später verknüpft (SUL-1) | ergänzen sich fachlich |

**Ausstiegsbedingung Rust:** Zeigt M0, dass sich das DJI-Datenmodell laufend ändert und Rust die Iteration spürbar bremst, wechselt der Server auf TypeScript (Fastify). Die Bausteingrenzen bleiben gleich.

## 3. Bausteine

```
RC Plus / Pilot 2 ──MQTT(S)──▶ EMQX ──▶ ┌────────────── Suchlage-Server (Rust) ──────────────┐
        │                               │ DJI-Adapter ─▶ Lage-Kern ─▶ WebSocket ─▶ Browser    │
        │                               │                    │                                │
        │                               │                 SQLite                              │
        └───────RTMP────▶ MediaMTX ─────┼──── WebRTC (WHEP) ───────────────────────▶ Browser │
                           (Aufzeichnung)└────────────────────────────────────────────────────┘
```

| Baustein | Verantwortung | kennt DJI? |
|---|---|---|
| **DJI-Adapter** | H5-Anmeldeseite für Pilot 2 (Lizenzprüfung per JSBridge, MQTT-Zugangsdaten je Gerät); DJI-Topics abonnieren und in eigene Ereignisse übersetzen; Livestream starten/stoppen, Linse wechseln; rohe MQTT-Sitzungen mitschneiden | **ja, als einziger** |
| **Lage-Kern** | Flugerkennung (Start/Landung), Spur, Live-Zustand, Verbindungsfrische; verteilt Zustand per WebSocket | nein |
| **Speicher** | SQLite: Geräte, Flüge, Spurpunkte, Mitschnitt-Index | nein |
| **MediaMTX** | RTMP-Ingest, WebRTC/WHEP an Browser, HLS als Rückfall, Aufzeichnung je Flug | nein |
| **Web-UI** | React-SPA, eingebettet in die Server-Binärdatei | nein |
| **Simulator** | spielt Mitschnitte über MQTT ab, für Entwicklung und Tests ohne Drohne | nutzt Adapter-Format |

Betrieb: Docker Compose (Suchlage-Server, EMQX, MediaMTX, Caddy). Identischer Stack auf vServer und FüKw-Server.

## 4. Datenfluss

1. **Anmeldung:** In Pilot 2 wird einmalig `https://<host>/pilot` als Cloud-Plattform eingetragen. Die Seite prüft die DJI-Lizenz, übergibt die MQTT-Zugangsdaten dieses Geräts und lädt die Module für Telemetrie und Livestream.
2. **Telemetrie:** Pilot 2 publiziert OSD/State an EMQX (etwa 1 Hz). Der Adapter übersetzt in eigene Ereignisse (Position, Höhen, Geschwindigkeit, Heading, Akku, Flugmodus, Flugzustand). Der Lage-Kern erkennt Start/Landung, speichert die Spur und pusht den Live-Zustand an alle Browser.
3. **Video:** „Live starten" am Arbeitsplatz (oder manuell in Pilot 2) → Server sendet `live_start_push` mit RTMP-Adresse → MediaMTX → WebRTC an Browser. Während eines Flugs zeichnet MediaMTX auf.
4. **Aufzeichnung:** Jeder Flug erscheint automatisch in der Flugliste: Spur auf der Karte, Kennzahlen, Video zum Abspielen und Herunterladen. Synchroner Replay von Spur und Video ist nicht Teil von M1.
5. **Mitschnitt:** Der Adapter kann rohe MQTT-Sitzungen speichern. Echte Flüge werden so zu Testdaten. Mitschnitte enthalten Positionen und kommen nur bereinigt ins Repo.

## 5. Fehlerfälle

| Fall | Verhalten |
|---|---|
| Telemetrie bleibt aus | Marker bleibt stehen, wird grau, zeigt „letzte Position vor N s"; Spur bekommt eine Lücke (neues Segment) |
| Video fällt aus | Player versucht selbst erneut; Karte und Telemetrie laufen unabhängig weiter |
| Server-Neustart | laufender Flug wird fortgesetzt, nicht doppelt angelegt |
| Unbekannte/ungültige DJI-Felder | werden ignoriert und gezählt, nie Absturz; sichtbar in der Systemansicht |
| LTE-Ausfall (vServer-Phase) | alles steht – bekannte Grenze, behoben erst mit M4 |

## 6. Sicherheit

- EMQX ist im Internet erreichbar: Zugangsdaten je Fernsteuerung, ACL nur auf die eigenen Topics, TLS.
- RTMP läuft in der vServer-Phase unverschlüsselt über LTE, geschützt durch einen geheimen Pfad je Stream. RTMPS wird in M0 geprüft.
- Web-UI nur nach Pocket-ID-Anmeldung; Großbild über widerrufbaren Zugangstoken.
- DJI App-ID, App-Key und License liegen im Passwortmanager und als Secret im Betrieb, nie im Repo.

## 7. Meilensteine

| # | Inhalt | Abnahme |
|---|---|---|
| **M0 Hardware-Spike** | DJI-Entwickler-App (Typ Cloud API) anlegen; auf dem vServer EMQX, MediaMTX und eine minimale Pilot-Anmeldeseite (fast nur Konfiguration); ein Testflug mit Mitschnitt | Fragen F1–F5 beantwortet, Mitschnitte und kurzes Protokoll liegen vor |
| **M1 Live-Lage** | Rust-Server (Adapter, Lage-Kern, WebSocket, SQLite, Pocket ID), UI im Lifeline-Design (Arbeitsplatz, Großbild, mobil), Video, Aufzeichnung, Flugliste, Simulator aus M0-Mitschnitten; Betrieb auf dem vServer | echter Flug: Position und Bild im FüKw und mobil sichtbar, Flug danach vollständig in der Liste; Verzögerung gemessen, Ziel ≤ 3 s über LTE |
| **M2 Kartenelemente** | Punkte und Flächen zwischen Browser und Pilot 2 in beide Richtungen abgleichen | Fläche im FüKw gezeichnet erscheint auf der RC Plus, Punkt des Piloten erscheint im FüKw |
| **M3 Fotos** | Medien-Upload aus Pilot 2 in S3-kompatiblen Speicher (MinIO), Fotos mit Koordinate auf der Karte | Wärmebildfoto liegt nach dem Flug an der richtigen Stelle |
| **M4 Betrieb im FüKw** | gleicher Stack auf dem FüKw-Server, Zertifikatslösung ohne Internet, Aufbaukarte | Flug bei gezogenem LTE; zweite Person baut nach Aufbaukarte selbst auf |
| **Später** | Suche (Sektoren, Abdeckung, Hinweise, Export), Verknüpfung Flugmappe (SUL-1), Schnittstelle Lifeline Hub | je eigenes Mini-PRD |

### Fragen für M0

- **F1** Lizenzprüfung: klappt sie mit LTE, und klappt sie ohne Internet?
- **F2** Vertraut Pilot 2 bei MQTT über TLS einem Let's-Encrypt-Zertifikat? (DJI nennt nur GoDaddy ausdrücklich.)
- **F3** Wie sehen die realen OSD-/State-Payloads der M350 aus (Felder, Einheiten, Frequenz)?
- **F4** Startet der Livestream vom Server aus, funktioniert der Linsenwechsel (`live_lens_change` antwortet laut Forum teils nicht), gibt es RTMPS?
- **F5** Wie groß ist die Ende-zu-Ende-Verzögerung über LTE?

**Wenn F1 oder F2 mit „nein" endet**, wird neu entschieden. Rückfallebenen: RTMP direkt aus Pilot 2 ohne Telemetrie, oder HDMI-Ausgang der RC Plus mit Hardware-Encoder.

## 8. Tests

- Rust-Unit-Tests für den Adapter, gespeist aus echten (bereinigten) M0-Mitschnitten
- ein Replay-Integrationstest: Mitschnitt rein, erwarteter Flug mit Spur raus
- ein Playwright-Smoke-Test für Arbeitsplatz, Großbild und mobil
- Hardware-Gates als kurze Checkliste je Meilenstein, ohne Belegapparat

## 9. Nicht-Ziele

- keine Flugsteuerung aus dem Browser
- kein Ersatz für Fireboard oder Lifeline Hub
- keine KI-Personenerkennung
- kein Betrieb für fremde Organisationen
- keine weiteren Drohnenmodelle vor M4

## 10. Risiken

| Risiko | Umgang |
|---|---|
| Lizenzprüfung braucht Internet | LTE im FüKw; F1 in M0 |
| Pilot 2 vertraut dem Zertifikat nicht | F2 in M0; notfalls GoDaddy-Zertifikat |
| DJI ändert Cloud API / Pilot 2 | Adapter isoliert, getestete Versionen festschreiben, Mitschnitte als Regressionstests |
| DJI pflegt die Demo nicht mehr | gegen die Doku bauen, nicht gegen die Demo |
| Bus-Faktor 1 | schlanker Stack, Aufbaukarte, zweite Person im Betrieb einweisen |
| Scope Creep | neue Wünsche gehen nach „Später" |
| Datenschutz bei Polizeiunterstützung | vor der Suche-Phase mit DRK-Datenschutz klären |
