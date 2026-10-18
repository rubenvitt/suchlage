# Einsatz-Bedienung und Löschen – Design

- **Stand:** 18.10.2026, aus dem Brainstorming nach der ersten Durchsicht von PR #9
- **Status:** Entwurf zur Durchsicht
- **Baut auf:** `2026-10-18-einsaetze-und-flugmappe-design.md` §4 (Einsätze), umgesetzt in PR #9 (SUL-65)
- **Skizzen:** Startseite, Lage mit laufendem Einsatz, Lage ohne Einsatz (im Gespräch gezeigt und freigegeben)

## 1. Zweck

Die erste Einsatz-Oberfläche aus PR #9 bewährt sich nicht. Anlegen und Starten sind umständlich, und der schnelle Wechsel zwischen Einsätzen direkt auf der Karte wirkt fremd. Außerdem fehlt das Löschen: Einsätze und Medien bleiben für immer, Zeichnungen ohne Einsatz ebenso.

Leitgedanke (Ruben): **Ein Einsatz ist der Normalfall.** Läuft keiner, steht die Auswahl eines Einsatzes am Anfang. Fliegen ohne Einsatz ist der zweite Weg.

**Erfolg:** Nach dem Login führt ein einziger Klick in die Lage eines laufenden Einsatzes, ein neuer Einsatz ist mit Name und einem Klick gestartet, und niemand landet aus Versehen „ohne Einsatz“.

## 2. Entscheidungen (Ruben, 18.10.2026)

| Thema | Entscheidung |
|---|---|
| Einstieg | Läuft ein Einsatz, öffnet sich direkt die Lage. Läuft keiner, kommt eine Startseite. |
| Startseite | Neuen Einsatz anlegen und **sofort starten** (Name, Ort optional). Zweitrangig: „Nur vorbereiten“. Daneben die vorbereiteten Einsätze mit „Starten“, ein Link ins Archiv, und „Überspringen“ für das Fliegen ohne Einsatz. |
| Laufender Einsatz | Steht in der Kopfleiste (Name, „Laufend“ mit Dauer, aktiver Abschnitt). Ein Klick öffnet ein Menü: Neuer Abschnitt, Bearbeiten (Name, Ort), Flüge des Einsatzes, Einsatz abschließen. |
| Wechsel auf der Karte | Entfällt. Die Auswahlleiste auf der Lagekarte (`MissionBar` mit Einsatzwahl) verschwindet. |
| Ohne Einsatz | Kopfleiste zeigt deutlich „Ohne Einsatz“, den Löschhinweis und „Einsatz starten“. |
| Aufbewahrung ohne Einsatz | Kartenelemente ohne Einsatz löscht der Server 24 Stunden nach ihrer letzten Änderung. Flüge und Fotos ohne Einsatz bleiben, damit man sie zuordnen kann. |
| Einsatz löschen | Nur vorbereitete und abgeschlossene Einsätze. Löscht alles, was dazugehört: Flüge mit Spur, Videoaufzeichnungen, Fotos (auch im Fotospeicher), Vorschaubilder, Kartenelemente, Abschnitte. Rückfrage nennt die Mengen; bestätigt wird durch Eintippen des Einsatznamens. |
| Medien löschen | Im Flugdetail: eine Auslösung (alle Linsen zusammen), das Video eines Flugs, oder den ganzen Flug samt Spur, Video und Fotos. Jeweils mit Rückfrage. |
| Wer löscht | Nur `admin`. |

## 3. Abläufe

### Einstieg

- Route `/`:
  - läuft ein Einsatz → Lage dieses Einsatzes;
  - sonst → Startseite, außer der Nutzer hat in dieser Browsersitzung „Überspringen“ gewählt → Lage ohne Einsatz.
- „Überspringen“ gilt für die Browsersitzung (`sessionStorage`), damit ein Neuladen nicht wieder die Startseite zeigt. Wird währenddessen ein Einsatz gestartet (auch von anderswo), wechselt die Lage von selbst in diesen Einsatz.
- Großbild (Kiosk): zeigt immer den laufenden Einsatz; läuft keiner, „Kein Einsatz läuft“ über der Karte ohne Elemente *(Entscheidung des Entwurfs)*.
- Mobil: dieselbe Startseite, untereinander angeordnet *(Entscheidung des Entwurfs)*.

### Vorbereiten

- „Nur vorbereiten“ und ein Klick auf einen vorbereiteten Einsatz (außer „Starten“) öffnen die Lage im **Vorbereitungsmodus** dieses Einsatzes: Kopfleiste „Vorbereitung: <Name>“ mit „Einsatz starten“ und „Zurück“. Gezeichnet wird wie sonst; Pilot 2 sieht davon nichts. *(Entscheidung des Entwurfs; wie die Vorbereitung erreichbar ist, war in den Skizzen offen.)*

### Archiv

- Die Rail-Seite „Einsätze“ wird zum Überblick: vorbereitete Einsätze, abgeschlossene Einsätze (Archiv, neueste zuerst) und „Ohne Einsatz“ (Flüge zum Zuordnen).
- Ein abgeschlossener Einsatz öffnet sich in der Lage **nur zum Lesen**, mit Kopfleiste „Archiv: <Name> · abgeschlossen“ und „Zurück“. Hier sitzen auch „Löschen …“ (Admin) und „Flüge“.
- Der URL-Parameter `?einsatz=` bleibt als Adresse für Vorbereitung und Archiv erhalten, aber nicht mehr als Wahlliste auf der Karte.

### Laufender Einsatz

- Menü in der Kopfleiste wie in §2. „Einsatz abschließen“ mit Rückfrage; während eines Flugs abgelehnt (vorhandene 422 mit Text anzeigen).
- Nach dem Abschließen: Startseite.

## 4. Löschen (Server)

| Aufruf | Rolle | Regeln |
|---|---|---|
| `GET /api/missions/{id}/contents` | Admin | Mengen für die Rückfrage: Flüge, Videos, Fotos (Dateien), Kartenelemente |
| `DELETE /api/missions/{id}` | Admin | laufend → 422; löscht alles aus §2 |
| `DELETE /api/flights/{id}` | Admin | laufender Flug → 422; Spur, Video, Fotos des Flugs |
| `DELETE /api/flights/{id}/video` | Admin | nur die Aufzeichnung |
| `DELETE /api/photos/shots/{shot_id}` | Admin | alle Dateien der Auslösung |

- **Reihenfolge und Fehler:** Erst Dateien (Fotospeicher, Vorschaubilder, Video), dann die Datenbankzeilen in einer Transaktion. Scheitert eine Datei mit etwas anderem als „nicht vorhanden“, bricht der Aufruf ab (503 bzw. 500) und nichts in der Datenbank ist gelöscht; ein zweiter Versuch räumt den Rest. *(Entscheidung des Entwurfs)*
- **Fotos:** `DELETE` auf die Objekte im Fotospeicher mit dem Server-Schlüssel (SigV4 wie `head`/`get`), Vorschaubilder unter `SUCHLAGE_DATA_DIR/thumbs` entfernen.
- **Video:** MediaMTX legt Aufzeichnungen unter `/recordings/live/<RC-SN>/<Startzeit>` in 10-Minuten-Segmenten ab. Der Server bekommt dasselbe Volume eingehängt, unter `SUCHLAGE_DATA_DIR/video` (keine neue Env-Variable): in `deploy/compose.m1.yaml` das benannte Volume `video` beschreibbar am Server-Container (Schreibrechte zwischen MediaMTX und Server im Plan klären); lokal liegt es schon unter `data/dev/video`. `deploy/compose.yaml` (M0-Spike) bleibt unverändert. *(Entscheidung des Entwurfs)*
- **Welche Segmente:** Ein Segment reicht von seiner Startzeit (Dateiname) bis zum Start des nächsten Segments derselben Fernsteuerung, höchstens 10 Minuten. Gelöscht wird ein Segment, wenn es das Zeitfenster eines zu löschenden Flugs überlappt **und** kein verbleibender Flug derselben Fernsteuerung es überlappt. Beim Löschen eines Einsatzes zählen nur die Flüge dieses Einsatzes. So geht kein Video eines Nachbarflugs verloren; ein Segment, das nur noch Randminuten eines gelöschten Flugs enthält und von einem anderen Flug gebraucht wird, bleibt. *(Entscheidung des Entwurfs)*
- **Ablage fehlt:** Ist `SUCHLAGE_DATA_DIR/video` nicht vorhanden, scheitert das Löschen eines Flugs oder Einsatzes mit Video hart (503 „Videoablage nicht eingehängt“), damit keine Aufzeichnung unbemerkt liegen bleibt. *(Entscheidung des Entwurfs)*
- **Fotos eines Flugs:** `DELETE /api/flights/{id}` löscht die Fotos mit dieser `flight_id`. Fotos ohne Flug gehören nur zum Einsatz und gehen mit dem Einsatz oder einzeln. Altlasten „ohne Einsatz“ werden einzeln gelöscht (Flug, Auslösung); ein Sammellöschen gibt es nicht. *(Entscheidung des Entwurfs)*
- **Offene Ansichten:** Wer gerade einen gelöschten Einsatz oder Flug ansieht, wird durch das Neuladen-Ereignis zurück auf die Startseite bzw. die Flugliste geführt, statt einen 404 zu sehen. *(Entscheidung des Entwurfs)*
- **Verteilung:** Gelöschte Kartenelemente gehen wie heute als `map_element_deleted` an Browser und Pilot 2; gelöschte Flüge und Fotos lösen ein Neuladen aus (`missions_changed` bzw. ein neues schlankes Ereignis, Wahl des Plans).
- **Aufräumen ohne Einsatz:** ein Hintergrundlauf (wie `garage::cleanup_loop`) löscht Kartenelemente mit `mission_id IS NULL` und `updated_at_ms` älter als 24 h, über den `MapHub`, damit Browser und Pilot 2 es erfahren. Intervall 10 Minuten *(Entscheidung des Entwurfs)*.

## 5. Nicht-Ziele

- Papierkorb oder Wiederherstellen. Gelöscht ist gelöscht; dafür die Rückfrage mit Eintippen.
- Löschfristen für Flüge und Fotos (Datenschutz-Frage der Suche-Phase, SUL-48).
- Löschen durch `operator`.
- Einsatz-Wechsel auf der Karte.
- Änderungen an der Flugmappe-Schnittstelle (SUL-66).

## 6. Anti-Ziele

- Jemand fliegt „ohne Einsatz“, ohne es zu merken.
- Ein Löschen hinterlässt Dateien ohne Datenbankeintrag, die niemand mehr findet, oder Einträge ohne Datei, die Fehler werfen.
- Ein Klick löscht ohne Rückfrage.

## 7. Tests

- Server: Rollen (Admin/Operator/Kiosk), 422 für laufenden Einsatz/Flug, Mengen in `contents`, Löschen mit Fake-Fotospeicher (`storage/fake.rs`) inkl. Fehlerfall „Speicher nicht erreichbar → nichts gelöscht“, Videodateien im Zeitfenster gelöscht und außerhalb nicht, Aufräumlauf nach 24 h.
- Browser: Einstieg je Lage (laufend → Lage, keiner → Startseite, übersprungen → ohne Einsatz, Start von anderswo → Lage), Starten in einem Schritt, Menü in der Kopfleiste, Archiv nur lesbar, Lösch-Rückfrage mit Eintippen.
- Smoke: Einsatz anlegen und starten, Flug, Einsatz abschließen und löschen; danach Flug, Foto und Video weg.

## 8. Offen für den Plan

- genaue Komponenten und Routen im Frontend
- ob gelöschte Flüge/Fotos ein eigenes SSE-Ereignis bekommen
- Format der Mengen-Anzeige in der Rückfrage
