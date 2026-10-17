# M2 Kartenelemente – Plan

- **Stand:** 17.10.2026, nach der Abnahme von M1 (SUL-9, SUL-22)
- **Spec:** `docs/superpowers/specs/2026-10-16-suchlage-design.md` §7, Zeile M2
- **Referenz:** `docs/reference/dji-cloud-api-m350.md` Kapitel 6.1; Original: DJI Cloud API v1.11.3, `30.feature-set/10.pilot-feature-set/40.pilot-map-elements.md`, `60.api-reference/10.pilot-to-cloud/10.https/10.map-elements/*`, `20.websocket/10.map-elements/10.message-push.md`, `30.jsbridge.md` (Module `api`, `ws`, `map`)
- **Abnahme (Spec):** Eine Fläche, die im FüKw gezeichnet wird, erscheint auf der RC Plus. Ein Punkt, den der Pilot setzt, erscheint im FüKw.

## Ziel und Abgrenzung

Punkte, Linien und Flächen werden zwischen Browser und Pilot 2 in beide Richtungen abgeglichen. Suchlage ist dabei der Server der DJI-Kartenelemente (HTTPS und WebSocket aus der Cloud API) und die Quelle für alle Browser.

Nicht in M2:
- Geometrie im Browser nachträglich verschieben. Im Browser wird umbenannt, umgefärbt und gelöscht; eine falsche Fläche wird neu gezeichnet. Pilot 2 darf Geometrien ändern, der Server übernimmt sie.
- Mehrere Ebenen oder Gruppen. Es gibt genau eine Gruppe vom Typ 2 (App-Shared), in der auch die Elemente aus dem Browser liegen.
- Situational Awareness (Modul `tsa`) und Sperren von Gruppen (`is_lock`).
- Suche, Sektoren und Abdeckung (Später, eigenes Mini-PRD).

## Entscheidungen

| Thema | Entscheidung | Grund |
|---|---|---|
| Ablage | Eigene Tabelle `map_elements`, eine feste Gruppe `SHARED_GROUP_ID` mit `type: 2` | Pilot 2 lädt neue Elemente nur in eine App-Shared-Gruppe hoch (Referenz 6.1). Eine Gruppe reicht für einen Einsatz. |
| Domänenmodell | `contract::MapElement` mit `kind` (point, line, polygon), `points` (`lat`, `lon`, `alt_m`), `color` (`#RRGGBB`), `clamp_to_ground`, `author`, `origin` (pilot, web) | Höhe und `clampToGround` aus Pilot 2 überleben den Rundweg. Das DJI-Format kennt nur `src/dji/map.rs`. |
| Ringe | Flächen speichern den Ring **offen** (erster Punkt nicht wiederholt). Ein geschlossener Ring aus Pilot 2 verliert den letzten Punkt; nach außen geht der Ring offen wie im DJI-Beispiel. | Das Beispiel in `30.obtain.md` ist offen; ein doppelter Punkt würde sonst bei jedem Rundweg einen Eckpunkt mehr erzeugen. |
| Farben | Der Browser bietet genau die sechs Hex-Werte der DJI-Tabelle an, benannt nach ihrem Aussehen (blau, grün, gelb, rot, violett `#B620E0`, schwarz `#212121`). Fremde Farben aus Pilot 2 werden normalisiert gespeichert (`#RRGGBB`, auch aus `0x…`). | Die Doku vertauscht ORANGE/PURPLE (Referenz 6.1). Welche Farbe Pilot 2 für `#B620E0` zeigt, klärt der Feldtest. |
| Token für Pilot 2 | Je Fernsteuerung ein API-Token, beim Koppeln (`/pilot/api/connect`) neu erzeugt; in der DB nur sein Hash (`remote_controllers.api_token_hash`). Gilt für HTTPS (`X-Auth-Token`) und WebSocket (`?x-auth-token=`). | Wie Sitzungen und Großbild-Tokens nur als Hash. Pilot 2 lädt die Module bei jedem Koppeln neu und bekommt dabei den neuen Token. |
| Pfade | DJI-HTTPS unter `/map/api/v1/workspaces/{workspace_id}/…` (Wurzel, von Pilot 2 festgelegt), WebSocket unter `/pilot/ws` | Das Modul `api` hängt `/map/api/v1/…` an den Host. Beide Pfade laufen wie `/pilot` über Traefik bzw. Cloudflare-Tunnel (Runbook, Schritt 6); nur `/internal` bleibt gesperrt. |
| Fehler an Pilot 2 | HTTP-Status nach Konvention, Body im DJI-Format `{code, message, data}` | Pilot 2 zeigt je Status einen Toast; 401 meldet die Plattform ab (`40.https.md`). Das ist für einen ungültigen Token richtig. |
| Verteilung | `map::MapHub` (tokio `broadcast`) verteilt jede Änderung an alle WebSocket-Verbindungen der Pilots und an alle SSE-Ströme. Pushes gehen an **alle** Pilots, auch an den Absender (wie die DJI-Demo). | Ein Ort, der schreibt und verteilt; Browser und Pilot sehen dieselbe Reihenfolge. |
| Browser | REST unter `/api/map/elements` (lesen: Viewer, schreiben: Operator) und drei neue `LiveEvent`-Varianten im bestehenden SSE-Strom (ADR 0001): `map_elements` (vollständige Liste nach dem Snapshot und nach einem Überholen), `map_element` (neu oder geändert), `map_element_deleted`. | Kein zweiter Strom; das Großbild bekommt die Elemente ohne Bedienung. |
| Zeichnen | Eigene kleine Zeichenlogik (Klick setzt Eckpunkt, „Fertig“ schließt ab) statt terra-draw | Drei Formen ohne Bearbeiten brauchen kein Framework. Die Logik ist eine reine Funktion und ohne WebGL testbar. |
| Icons | Icons8 darf verwendet werden (Klärung 17.10.2026). M2 bleibt bei Beschriftungen; die Übernahme der Lifeline-Icons ist eine eigene Aufgabe auf dem Board. | Kein Umbau der Rail im selben Strang. |

## Bausteine

```
Pilot 2 ──HTTPS /map/api/v1/…──▶ web::pilot_map ─┐
        ◀──WS /pilot/ws─────────── map_element_* │
                                                 ▼
Browser ──REST /api/map/elements──▶ web::map ──▶ map::MapHub ──▶ store::map_elements (SQLite)
        ◀──SSE /api/live──────────── LiveEvent::map_*  │
                                                       └─ broadcast an alle WS und SSE
```

| Datei | Inhalt |
|---|---|
| `migrations/0002_map_elements.sql` | Tabelle `map_elements`, Spalte `remote_controllers.api_token_hash` mit eindeutigem Index |
| `src/contract/map.rs` | `MapElement`, `MapElementKind`, `MapPoint`, `MapElementOrigin`, `MapElementInput`, `MapElementUpdate`, `MAP_COLORS` |
| `src/map/mod.rs` | `MapHub`: `list`, `create`, `update`, `delete`, `subscribe`; prüft mit `map::rules` |
| `src/map/rules.rs` | reine Prüfungen: Name, Farbe, Punktzahl je Form, Koordinatenbereich, Ring öffnen |
| `src/store/map_elements.rs` | SQLite-Zugriff |
| `src/dji/map.rs` | DJI-Format: Gruppenliste, `ElementCreateInput`, `ElementUpdateInput`, GeoJSON-Inhalt ↔ Domäne, WebSocket-Hülle `{biz_code, version, timestamp, data}` |
| `src/web/pilot_map.rs` | `/map/api/v1/…` (GET, POST, PUT, DELETE) und `/pilot/ws`, Token-Extractor `PilotApi` |
| `src/web/map.rs` | `/api/map/elements` für den Browser |
| `src/web/live.rs` | SSE führt den `MapHub` neben dem Lage-Strom |
| `src/dji/pilot/pilot.js` | lädt nach `thing` und `liveshare` die Module `api`, `ws` und `map`; ein Fehler dort bricht die Kopplung nicht ab |
| `frontend/src/map/elements.ts` | GeoJSON für die Karte, Zeichen-Zustandsmaschine, Farbpalette, Namensvorschlag |
| `frontend/src/map/SituationMap.tsx` | Ebenen `elements-fill`, `elements-line`, `elements-point`, `draft`; Zeichenwerkzeuge |
| `frontend/src/map/ElementDialog.tsx`, `ElementPanel.tsx` | Name und Farbe beim Anlegen; Auswahl mit Umbenennen, Umfärben, Löschen |

## Aufgaben

Jede Aufgabe endet mit grünem `scripts/check.sh` und einem eigenen Commit.

1. **K1 Vertrag und Speicher:** Migration, `contract::map`, `store::map_elements`, `map::rules` mit Tests (gültige und ungültige Formen, Ring schließen/öffnen, Farbnormalisierung). Vertrag erzeugen.
2. **K2 MapHub und Browser-API:** `MapHub` im `AppState`, `/api/map/elements` mit Rollenprüfung (Viewer liest, Operator schreibt, Kiosk 403), SSE-Varianten. Tests: Anlegen erscheint im SSE-Strom, Löschen ebenso, überholter Strom bekommt die Liste neu.
3. **K3 DJI-Format:** `dji::map` mit Tests gegen die Beispiele der Doku (Liste, Anlegen, Ändern, Push-Hülle), inklusive geschlossener und offener Ringe und `[lon, lat]` ohne Höhe.
4. **K4 Pilot-Schnittstelle:** API-Token beim Koppeln, `PilotApi`-Extractor (Header `X-Auth-Token`, Query `x-auth-token`), HTTPS-Endpunkte, WebSocket mit Push an alle Pilots. Tests: ohne Token 401 im DJI-Format, fremde Workspace-ID 404, Anlegen aus Pilot 2 erscheint im Browser-Strom, Anlegen im Browser erzeugt `map_element_create` am WebSocket.
5. **K5 Pilot-Seite:** Module `api`, `ws`, `map` laden; Node-Tests für Reihenfolge und Fehlertoleranz.
6. **K6 Karte im Browser:** Elemente auf allen Karten (Arbeitsplatz, Großbild, mobil, nicht im Flugdetail); Zeichnen nur am Arbeitsplatz mit Rolle Operator oder Admin; Auswahl mit Umbenennen, Umfärben, Löschen. Vorschau-Modus (`mise run ui`) zeigt Beispielelemente.
7. **K7 Betrieb und Abnahme:** `suchlage-sim` setzt beim Start über `/map/api/v1/…` einen Punkt, wie Pilot 2; Checkliste `docs/betrieb/m2-realflug-checkliste.md`; das Runbook nennt die neuen Pfade und die Fehlerbilder.

## Offene Punkte und Annahmen

1. **Farben in Pilot 2:** Welche Farbe Pilot 2 für `#B620E0` und `#212121` zeigt und welche Werte Pilot 2 selbst schickt, zeigt der Feldtest.
2. **Echo an den Absender:** Pilot 2 bekommt seinen eigenen Punkt per WebSocket zurück (wie in der DJI-Demo). Verdoppelt Pilot 2 ihn, muss der Server den Absender auslassen (Token → Verbindung).
3. **`elementPreName`:** Die Seite setzt `Pilot`, `userName` ist die Bezeichnung der Fernsteuerung. Wie Pilot 2 daraus Namen bildet, zeigt der Feldtest.
4. **Gruppen-ID:** fest im Code. Ein Wechsel der Workspace-ID in den Einstellungen trennt Pilot 2 von den Elementen (404), bis neu gekoppelt wird.
5. **Token-Rotation:** Jedes Koppeln erzeugt einen neuen Token und macht den alten ungültig. Öffnet jemand die Pilot-Seite auf einem zweiten Gerät mit demselben Code, verliert das erste den Kartenabgleich (401 meldet die Plattform dort ab).
6. **Höhe:** `alt_m` wird nur durchgereicht. Der Browser zeichnet ohne Höhe und setzt `clampToGround: true`.
