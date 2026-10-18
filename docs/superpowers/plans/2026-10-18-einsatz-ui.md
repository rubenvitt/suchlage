# Einsatz-Bedienung und Löschen – Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

- **Stand:** 18.10.2026, auf PR #9 (Branch `feat/sul-65-einsaetze`)
- **Spec:** `docs/superpowers/specs/2026-10-18-einsatz-ui-design.md` (baut auf `2026-10-18-einsaetze-und-flugmappe-design.md` §4)
- **Board:** SUL-78

**Goal:** Einsatz-Bedienung mit Startseite, Einsatz in der Kopfleiste, Archiv, 24-h-Aufräumen ohne Einsatz und Löschen von Einsätzen und Medien (nur Admin).

**Architecture:** Server: Löschfunktionen im Speicher, ein reiner Segment-Rechner für Videos, ein `Deleter`, der Dateien (Fotospeicher, Vorschaubilder, Video) vor den Datenbankzeilen entfernt, und ein Aufräumlauf im Stil von `garage::cleanup_loop`. Browser: Route `/` entscheidet zwischen Startseite und Lage; die Kartenwahl (`MissionBar`) weicht einem Einsatz-Element in der Kopfleiste. Nach jedem Löschen meldet der Server `missions_changed` (der Browser lädt Einsätze, Flüge und Fotos neu).

**Tech Stack:** Rust (axum, sqlx/SQLite, reqwest/SigV4), React 19 + antd 6 + TanStack Query, MSW.

## Global Constraints

- Alles aus `AGENTS.md`: `scripts/check.sh` grün vor jedem Commit, Commits nur mit den Pfaden der Aufgabe, Statuscodes (400/422/409/401/403/503), Bausteingrenzen, keine neue Env-Variable, Texte deutsch mit echten Umlauten.
- Löschen nur `admin` (Operator → 403, Kiosk → 403).
- Laufender Einsatz oder laufender Flug → 422.
- Dateien vor Datenbank: Scheitert das Löschen einer Datei mit etwas anderem als „nicht vorhanden“, bleibt die Datenbank unverändert (Fotospeicher nicht erreichbar → 503).
- Videoablage: `SUCHLAGE_DATA_DIR/video`, Struktur `live/<RC-SN>/<YYYY-MM-DD_HH-MM-SS-ffffff>.mp4` (MediaMTX `recordPath`, UTC); fehlt das Verzeichnis und hat der Flug Video → 503 „Videoablage nicht eingehängt“.
- Segment gehört zum Löschen, wenn es einen zu löschenden Flug derselben Fernsteuerung überlappt und keinen verbleibenden; Segmentende = Start des nächsten Segments derselben Fernsteuerung, höchstens Start + 10 min.
- Kartenelemente ohne Einsatz: löschen, wenn `updated_at_ms` älter als 24 h; Lauf alle 10 min; über den `MapHub` (Browser und Pilot 2 erfahren es).
- „Überspringen“ gilt pro Browsersitzung: `sessionStorage['suchlage:ohne-einsatz'] = '1'`.
- Kiosk zeigt immer den laufenden Einsatz, ohne laufenden „Kein Einsatz läuft“.
- `?einsatz=<id>` öffnet einen vorbereiteten (Vorbereitung) oder abgeschlossenen (Archiv, nur lesen) Einsatz; keine Wahlliste mehr auf der Karte.

## Review Focus

1. **Start von anderswo:** Wer „Überspringen“ gewählt hat, landet automatisch in der Lage, sobald irgendwer einen Einsatz startet. → Test in Task 5.
2. **Fotospeicher weg:** Löschen eines Einsatzes, während Garage nicht antwortet, löscht nichts in der Datenbank. → Test in Task 3.
3. **Nachbarflug:** Ein Videosegment, das auch ein verbleibender Flug derselben Fernsteuerung braucht, bleibt. → Test in Task 2.
4. **Gelöschtes in offenen Ansichten:** Ein Flugdetail eines eben gelöschten Flugs führt zurück zur Flugliste statt einen Fehler zu zeigen. → Test in Task 7.
5. **Operator:** sieht keine Lösch-Bedienelemente, und die API antwortet 403. → Tests in Task 3 und 6.

---

### Task 1: Speicher – Löschen, Mengen, Bearbeiten, Aufräumen

**Files:** Modify `src/store/missions.rs`, `src/store/flights.rs`, `src/store/photos.rs`, `src/store/map_elements.rs`, `src/contract/missions.rs`, `src/contract/examples.rs` (+ erzeugte TS)

**Interfaces – Produces:**
- Vertrag: `MissionContents { flights: u32, photos: u32, map_elements: u32, video_segments: u32 }`, `MissionUpdate { name: String, location: Option<String> }`
- `store::missions::update(pool, id, &MissionUpdate) -> Result<bool, sqlx::Error>`
- `store::missions::counts(pool, id) -> Result<(u32, u32, u32), sqlx::Error>` (Flüge, Fotodateien, Kartenelemente)
- `store::missions::delete_rows(pool, id) -> Result<bool, sqlx::Error>` – in einer Transaktion: Fotos, Flüge (Spurpunkte kaskadieren), Kartenelemente, Abschnitte, Einsatz
- `store::flights::of_mission(pool, id) -> Result<Vec<FlightSummary>, _>`, `store::flights::delete_rows(pool, id) -> Result<bool, _>` (inkl. Fotos mit dieser `flight_id`)
- `store::photos::of_mission_all(pool, id) -> Result<Vec<PhotoRow>, _>` (mit und ohne Flug), `store::photos::delete_shot_rows(pool, shot_key) -> Result<u64, _>`
- `store::map_elements::unassigned_older_than(pool, cutoff_ms) -> Result<Vec<String>, _>`

- [ ] **Step 1: Tests (rot):** `delete_rows_removes_everything_of_the_mission_and_nothing_else` (zwei Einsätze, je Flug, Spur, Foto, Element, Abschnitt; nach Löschen von A ist B vollständig da); `counts_match_contents`; `flight_delete_takes_its_photos_only` (Foto ohne Flug im selben Einsatz bleibt); `unassigned_older_than_ignores_assigned_and_fresh`; `update_changes_name_and_location`.
- [ ] **Step 2:** implementieren; `cargo test store` grün; `cargo test contract` erzeugt TS.
- [ ] **Step 3:** `scripts/check.sh`, Commit `SUL-78: Speicher für Löschen und Bearbeiten`.

### Task 2: Videoablage und Fotospeicher-Löschen

**Files:** Create `src/media/recordings.rs`; Modify `src/media/mod.rs`, `src/storage/s3.rs`, `src/storage/fake.rs`, `src/config.rs` (`video_dir()` = `data_dir/video`), `deploy/compose.m1.yaml`, `deploy/compose.dev.yaml`, `scripts/dev-smoke.sh`, `scripts/e2e.sh`

**Interfaces – Produces:**
- `media::recordings::Segment { rc: String, start_ms: i64, path: PathBuf }`
- `media::recordings::scan(video_dir: &Path, rc: &str) -> std::io::Result<Vec<Segment>>` – leere Liste, wenn `live/<rc>` fehlt; `Err(NotFound)`, wenn `video_dir` selbst fehlt
- `media::recordings::to_delete(segments: &[Segment], delete: &[(i64, i64)], keep: &[(i64, i64)]) -> Vec<PathBuf>` – rein, Regel aus den Global Constraints
- `storage::s3::delete(http, cfg, object_key) -> Result<(), AppError>` – 204/404 = Ok, sonst Fehler; der Fake-Speicher kann „nicht erreichbar“ simulieren

- [ ] **Step 1: Tests (rot):** `parses_mediamtx_names_as_utc` (`2026-10-18_12-30-00-123456.mp4` → ms); `deletes_overlapping_segments`; `keeps_segment_shared_with_remaining_flight`; `segment_end_is_next_start_or_ten_minutes`; `missing_rc_dir_is_empty`; S3: `delete_signs_and_treats_404_as_done` gegen den Fake.
- [ ] **Step 2:** implementieren. In `compose.dev.yaml` das Aufzeichnungsverzeichnis von MediaMTX über `${SL_VIDEO_DIR:-../data/dev/video}` einhängen und in `scripts/dev-smoke.sh`/`scripts/e2e.sh` `SL_VIDEO_DIR="$SUCHLAGE_DATA_DIR/video"` setzen, damit Server und MediaMTX dieselbe Ablage sehen. In `compose.m1.yaml` das Volume `video` zusätzlich am Server unter `/data/video` einhängen (beschreibbar); prüfen, ob der Server-Prozess dort löschen darf (MediaMTX schreibt als root) – sonst die kleinste Lösung (z. B. gleiche UID oder `user:` im Compose) wählen und im Runbook vermerken.
- [ ] **Step 3:** `scripts/check.sh`, Commit `SUL-78: Videosegmente und Fotospeicher löschen`.

### Task 3: Löschen über die API

**Files:** Create `src/deletion.rs`; Modify `src/lib.rs`, `src/app.rs`, `src/testing.rs`, `src/web/missions.rs`, `src/web/flights.rs`, `src/web/photos.rs`, `src/missions/mod.rs` (`notify` öffentlich als `changed()`)

**Interfaces – Consumes:** Task 1 und 2. **Produces:**
- `deletion::Deleter { pool, photos: PhotoHub, map: MapHub, missions: MissionHub, video_dir: PathBuf }` in `AppState.deleter`
- `Deleter::contents(mission_id) -> Result<MissionContents, AppError>`, `delete_mission(id)`, `delete_flight(id)`, `delete_flight_video(id)`, `delete_shot(shot_key)` → `Result<(), AppError>`; jede Löschung ruft am Ende `missions.changed()`; gelöschte Kartenelemente gehen als `MapChange::Deleted` hinaus
- Routen: `GET /api/missions/{id}/contents`, `DELETE /api/missions/{id}`, `PUT /api/missions/{id}` (Operator, `MissionUpdate`; abgeschlossen → 422), `DELETE /api/flights/{id}`, `DELETE /api/flights/{id}/video`, `DELETE /api/photos/shots/{shot_id}` (`shot_id` URL-kodiert, enthält `/`)

- [ ] **Step 1: Tests (rot):** `operator_and_kiosk_cannot_delete` (403); `running_mission_or_flight_is_422`; `deleting_a_mission_removes_rows_photos_thumbs_and_videos` (Fake-Speicher + temporäres `video_dir` mit Segmenten); `storage_down_deletes_nothing` (503, Datenbank unverändert); `missing_video_dir_is_503_only_when_flight_has_video`; `contents_counts`; `update_renames_running_mission`; SSE: nach Löschen `missions_changed`.
- [ ] **Step 2:** implementieren (Reihenfolge: Dateien sammeln → Fotospeicher → Vorschaubilder → Video → Datenbank → Ereignisse).
- [ ] **Step 3:** `scripts/check.sh`, Commit `SUL-78: Einsätze, Flüge und Medien löschen`.

### Task 4: Aufräumen ohne Einsatz

**Files:** Modify `src/map/mod.rs` (oder `src/map/cleanup.rs`), `src/app.rs`

**Interfaces – Produces:** `map::cleanup_loop(map: MapHub, mut stop: watch::Receiver<bool>)`; `MapHub::delete_unassigned_older_than(cutoff_ms) -> Result<usize, AppError>`; Konstanten `UNASSIGNED_TTL_MS = 24 * 3_600_000`, `CLEANUP_EVERY = 10 min`.

- [ ] **Step 1: Tests (rot):** `old_unassigned_elements_are_deleted_and_announced` (Element 25 h alt, ohne Einsatz → `MapChange::Deleted`; 23 h alt und Element im Einsatz bleiben).
- [ ] **Step 2:** implementieren, in `app.rs` starten.
- [ ] **Step 3:** `scripts/check.sh`, Commit `SUL-78: Kartenelemente ohne Einsatz nach 24 h löschen`.

### Task 5: Browser – Einstieg und Startseite

**Files:** Create `frontend/src/missions/WelcomePage.tsx` (+ Test), `frontend/src/missions/entry.ts` (+ Test); Modify `frontend/src/app/router.tsx`, `frontend/src/missions/scope.ts`, `frontend/src/missions/useMissionScope.ts`, `frontend/src/test/handlers.ts`, `frontend/src/api/queries.ts` (Mutationen)

**Interfaces – Produces:**
- `entryFor({ role, missions, param, skipped }): 'lage' | 'welcome'` – rein: Kiosk → lage; laufender Einsatz → lage; `param` gesetzt → lage; `skipped` → lage; sonst welcome
- `SKIP_KEY = 'suchlage:ohne-einsatz'`; `useSkip(): { skipped: boolean; skip(): void; reset(): void }` (sessionStorage, in try/catch)
- `resolveScope` ohne `'ohne'`-Wert: `param` id → dieser Einsatz; sonst laufender; sonst none
- Route `/`: `EntryPage` rendert `WelcomePage` oder `LagePage`

- [ ] **Step 1: Tests (rot):** `entry.test.ts` alle fünf Fälle; `WelcomePage.test.tsx`: „Einsatz starten“ legt an und startet in einem Schritt und zeigt danach die Lage; „Nur vorbereiten“ öffnet `/?einsatz=<id>`; vorbereiteter Einsatz „Starten“; „Überspringen“ zeigt die Lage ohne Einsatz; nach `missions_changed` mit laufendem Einsatz wechselt die übersprungene Lage in diesen Einsatz (Review Focus 1).
- [ ] **Step 2:** implementieren nach Skizze 1 (Name, Ort optional, „Einsatz starten“ primär, „Nur vorbereiten“ sekundär, rechts Vorbereitet mit „Starten“, Archiv-Link `/einsaetze`, „Überspringen“ mit Hinweis „Zeichnungen ohne Einsatz werden nach 24 Stunden gelöscht.“); mobil untereinander.
- [ ] **Step 3:** `scripts/check.sh --frontend`, Commit `SUL-78: Startseite ohne laufenden Einsatz`.

### Task 6: Browser – Einsatz in der Kopfleiste, Archiv, Löschen

**Files:** Create `frontend/src/missions/MissionHeader.tsx` (+ Test), `frontend/src/missions/DeleteMissionDialog.tsx`; Modify `frontend/src/views/LagePage.tsx`, `frontend/src/flights/FlightsPage.tsx`, `frontend/src/views/WorkstationView.tsx`, `frontend/src/views/MobileView.tsx`, `frontend/src/views/KioskView.tsx`, `frontend/src/missions/MissionsPage.tsx`, `views.css`/`missions.css`; Delete `frontend/src/missions/MissionBar.tsx` (+ Test)

**Interfaces – Consumes:** Task 3 Routen, Task 5 `useMissionScope`/`useSkip`. **Produces:** `MissionHeader` im Status-Bereich von `LagePage` und `FlightsPage`.

- [ ] **Step 1: Tests (rot):** laufend: Name, „Laufend“ mit Dauer, aktiver Abschnitt, Menü mit „Neuer Abschnitt …“, „Einsatz bearbeiten“, „Flüge dieses Einsatzes“, „Einsatz abschließen …“ (Rückfrage; danach Startseite); ohne: „Ohne Einsatz“, Löschhinweis, „Einsatz starten“ (→ Startseite, Skip zurückgesetzt); Vorbereitung: „Vorbereitung: <Name>“, „Einsatz starten“, „Zurück“; Archiv: „Archiv: <Name> · abgeschlossen“, nur lesen, „Löschen …“ nur für Admin; `DeleteMissionDialog` zeigt die Mengen aus `contents` und aktiviert „Endgültig löschen“ erst nach Eintippen des Namens; Kiosk ohne laufenden Einsatz zeigt „Kein Einsatz läuft“; Einsätze-Seite: Vorbereitet, Archiv (neueste zuerst), Ohne Einsatz.
- [ ] **Step 2:** implementieren nach Skizzen 2 und 3; `MissionBar` und die Einsatzwahl auf der Karte entfernen.
- [ ] **Step 3:** `scripts/check.sh --frontend`, Commit `SUL-78: Einsatz in der Kopfleiste, Archiv und Löschen`.

### Task 7: Browser – Medien löschen, Smoke, Doku

**Files:** Modify `frontend/src/flights/FlightDetailPage.tsx`, `frontend/src/photos/PhotoPanel.tsx`, `scripts/dev-smoke.sh`, `docs/betrieb/einsaetze-checkliste.md`, `docs/betrieb/runbook-deployment.md`

- [ ] **Step 1: Tests (rot):** Flugdetail (Admin): „Flug löschen …“, „Video löschen …“, je Auslösung „Foto löschen …“ mit Rückfrage; nach Löschen des Flugs zurück zur Flugliste; ein Flugdetail, dessen Flug inzwischen gelöscht ist, leitet zur Flugliste (Review Focus 4); Operator sieht keine Löschknöpfe.
- [ ] **Step 2:** implementieren; Smoke: nach dem Flug Einsatz abschließen, `contents` prüfen, Einsatz löschen, danach `GET /api/flights/<id>` 404, Foto-Original 404 und Videosegmente des Flugs im `video`-Verzeichnis weg; Runbook: Video-Volume am Server, Aufräumen ohne Einsatz; Checkliste: Startseite, Kopfleiste, Löschen.
- [ ] **Step 3:** `scripts/check.sh`, `scripts/dev-smoke.sh`, `scripts/e2e.sh` grün; Commit `SUL-78: Medien löschen, Smoke und Doku`.
