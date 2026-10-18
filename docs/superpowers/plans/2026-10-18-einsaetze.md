# Einsätze in Suchlage – Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

- **Stand:** 18.10.2026. M3 ist gemergt (0.3.0). Dieser Plan setzt Teil A der Spec um; die Schnittstelle für die Flugmappe (SUL-66) baut darauf auf.
- **Spec:** `docs/superpowers/specs/2026-10-18-einsaetze-und-flugmappe-design.md` §4 (Teil A), dazu §2 und §3
- **Board:** SUL-65

**Goal:** Suchlage kennt Einsätze (vorbereitet → laufend → abgeschlossen) mit Abschnitten; Flüge, Kartenelemente und Fotos gehören zu einem Einsatz oder zu „ohne Einsatz“.

**Architecture:** Neue Tabellen `missions` und `mission_sections`, dazu `mission_id` an `flights`, `map_elements`, `photos`. Die Zuordnung beim Anlegen passiert im SQL des Speichers (laufender Einsatz zum Zeitpunkt des Einfügens), damit der reine Lage-Kern unberührt bleibt. Ein `MissionHub` (wie `MapHub`/`PhotoHub`) führt Zustandswechsel aus, meldet sie per SSE und setzt den Sichtbereich der Pilots im `MapHub`. Der Browser wählt den Einsatz über den URL-Parameter `einsatz`.

**Tech Stack:** Rust (axum, sqlx/SQLite, ts-rs), React 19 + antd 6 + TanStack Query, MSW für Vorschau und Tests.

## Global Constraints

- `scripts/check.sh` grün vor jedem Commit; Commits nur mit den Pfaden der Aufgabe (AGENTS.md).
- Bausteingrenzen: `src/situation/` bleibt unverändert und rein; `src/contract/` hängt von nichts im Crate ab; nur `src/dji/` kennt DJI-Typen.
- Vertrag: Typ in `src/contract/` ändern, `cargo test contract` erzeugt `frontend/src/contract/`, beides zusammen committen.
- Statuscodes: 400 Feld, 422 Zusammenhang/Übergang, 409 Konflikt, 401 ohne Sitzung, 403 ohne Rolle.
- Höchstens ein Einsatz `running`, in der Datenbank abgesichert (Teilindex wie `flights_one_open_per_aircraft`).
- Ein Flug gehört zu dem Einsatz und Abschnitt, die beim **Start** laufen bzw. aktiv sind; spätere Wechsel ändern ihn nicht.
- Pilot 2 sieht die Elemente des laufenden Einsatzes, ohne laufenden Einsatz die Elemente „ohne Einsatz“.
- Abgeschlossene Einsätze sind nur lesbar (Kartenelemente ändern → 422). Nachträgliches Zuordnen von „ohne Einsatz“ geht in jeden Einsatz, auch in abgeschlossene.
- Rechte: lesen jede Sitzung (auch Kiosk), ändern nur `operator`/`admin`.
- Neue Bezeichner englisch, Texte und Kommentare deutsch mit echten Umlauten. Keine neue Env-Variable.
- Nicht in diesem Plan: Flugmappe-Felder (Herkunft, Flugmappe-IDs, Sperre für Abschnitte bei verknüpften Einsätzen) – kommen mit SUL-66.

## Review Focus

1. **Bestandsdaten:** Nach der Migration gehören alle vorhandenen Flüge, Elemente und Fotos zu „ohne Einsatz“ und erscheinen dort; nichts verschwindet. → Test in Task 1.
2. **Laufender Flug beim Wechsel:** Startet ein anderer Einsatz oder endet der laufende, während ein Flug läuft, bleibt der Flug bei seinem Einsatz; Abschließen mit laufendem Flug → 422. → Tests in Task 1 und 2.
3. **Pilot sieht nichts Fremdes:** Ein Element, das der Browser in einem vorbereiteten Einsatz zeichnet, geht nicht per WebSocket an Pilot 2. → Test in Task 4.
4. **Neustart:** Nach einem Server-Neustart kennt der `MapHub` den laufenden Einsatz wieder (Sichtbereich der Pilots). → Test in Task 4.
5. **Ungültige Auswahl im Browser:** `?einsatz=999` (unbekannt) oder Müll zeigt den laufenden Einsatz bzw. „ohne Einsatz“, keinen Fehler. → Test in Task 5.

---

### Task 1: Migration, Vertrag und Speicher

**Files:**
- Create: `migrations/0004_missions.sql`, `src/contract/missions.rs`, `src/store/missions.rs`
- Modify: `src/contract/mod.rs`, `src/contract/map.rs`, `src/contract/photos.rs`, `src/contract/examples.rs`, `src/store/mod.rs`, `src/store/flights.rs`, `src/store/map_elements.rs`, `src/store/photos.rs`

**Interfaces:**
- Produces (Vertrag):
  - `enum MissionState { Prepared, Running, Closed }` (snake_case)
  - `struct MissionSection { id: i64, label: String, location: Option<String>, created_at_ms: i64 }`
  - `struct Mission { id: i64, name: String, location: Option<String>, state: MissionState, created_by: String, created_at_ms: i64, started_at_ms: Option<i64>, closed_at_ms: Option<i64>, sections: Vec<MissionSection> }` – `sections` älteste zuerst, die letzte ist aktiv
  - `struct MissionCreate { name: String, location: Option<String> }`, `struct SectionCreate { label: String, location: Option<String> }`, `struct MissionAssign { mission_id: i64 }`
  - neues Feld `mission_id: Option<i64>` (`#[ts(type = "number | null")]`) an `MapElement`, `MapElementInput` und `PhotoShot`
- Produces (Speicher, `store::missions`):
  - `create(pool, input: &MissionCreate, created_by: &str, now_ms: i64) -> Result<Mission, sqlx::Error>` – Zustand `prepared`, erster Abschnitt mit Label „Abschnitt 1“ und Ort des Einsatzes
  - `get(pool, id) -> Result<Option<Mission>, _>`, `list(pool) -> Result<Vec<Mission>, _>` (laufend, dann vorbereitet nach Anlage, dann abgeschlossen neueste zuerst)
  - `running_id(pool) -> Result<Option<i64>, _>`
  - `set_state(pool, id, state: MissionState, now_ms) -> Result<(), sqlx::Error>` (setzt `started_at_ms`/`closed_at_ms`; der Teilindex liefert bei zweitem `running` einen Unique-Fehler → `AppError::Conflict` über das vorhandene `From<sqlx::Error>`)
  - `add_section(pool, mission_id, input: &SectionCreate, now_ms) -> Result<(), _>`
  - `has_open_flight(pool, mission_id) -> Result<bool, _>`
  - `assign_flight(pool, flight_id, mission_id) -> Result<bool, _>` – nur wenn `flights.mission_id IS NULL`; setzt Abschnitt = letzter Abschnitt des Ziels und `photos.mission_id` der Fotos des Flugs in derselben Transaktion; `false`, wenn nichts passte
  - `assign_element(pool, element_id, mission_id) -> Result<bool, _>` – nur wenn `mission_id IS NULL`
- Produces (Bereichsfilter): `store::flights::list_in(pool, mission: Option<i64>, limit) -> Result<Vec<FlightSummary>, _>`, `store::map_elements::list_in(pool, mission: Option<i64>)`, `store::photos::in_mission(pool, mission: i64)` und `store::photos::since_unassigned(pool, since_ms)`; alle filtern mit `mission_id IS ?`.

- [ ] **Step 1: Migration schreiben.** `missions(id, name CHECK 1..120, location, state CHECK IN ('prepared','running','closed'), created_by, created_at_ms, started_at_ms, closed_at_ms)`, `CREATE UNIQUE INDEX missions_one_running ON missions (state) WHERE state = 'running'`, `mission_sections(id, mission_id REFERENCES missions ON DELETE CASCADE, label CHECK 1..120, location, created_at_ms)`, `ALTER TABLE flights ADD COLUMN mission_id INTEGER REFERENCES missions(id)` und `section_id INTEGER REFERENCES mission_sections(id)`, `map_elements.mission_id`, `photos.mission_id`, je ein Index auf `mission_id`.
- [ ] **Step 2: Tests in `store::missions` schreiben (rot):**
  - `create_starts_prepared_with_first_section` – `state == Prepared`, `sections.len() == 1`, `sections[0].label == "Abschnitt 1"`
  - `second_running_mission_is_a_conflict` – zweites `set_state(.., Running)` → `AppError::Conflict`
  - `flight_takes_running_mission_and_latest_section_at_insert` – Einsatz laufen lassen, zweiten Abschnitt anlegen, `flights::insert` → `list_in(Some(id))` enthält den Flug; danach Einsatz schließen, nächster Flug landet in `list_in(None)`
  - `photo_takes_mission_of_its_flight_else_running` – Foto mit `flight_id` eines Flugs aus Einsatz A, während B läuft → `mission_id == A`; Foto ohne Flug → `B`
  - `existing_rows_are_unassigned_after_migration` – vor `create` angelegter Flug, Element und Foto erscheinen in `list_in(None)` / `since_unassigned`
  - `assign_flight_moves_its_photos_and_only_from_unassigned` – Zuordnen setzt Flug und Fotos um; zweiter Aufruf → `false`
- [ ] **Step 3: Implementieren.** `flights::insert` setzt `mission_id = (SELECT id FROM missions WHERE state = 'running')` und `section_id = (SELECT MAX(id) FROM mission_sections WHERE mission_id = <dieselbe>)` im `INSERT`. `photos::upsert` setzt `mission_id = COALESCE((SELECT mission_id FROM flights WHERE id = ?flight_id), (SELECT id FROM missions WHERE state = 'running'))`, auch im `ON CONFLICT`-Zweig. `map_elements::insert` speichert `mission_id` aus dem Element. Vertragsbeispiele in `examples.rs` um `mission_id` ergänzen.
- [ ] **Step 4:** `cargo test store` und `cargo test contract` grün; erzeugte TS-Dateien prüfen.
- [ ] **Step 5:** `scripts/check.sh`, Commit `SUL-65: Einsätze in Schema, Vertrag und Speicher`.

### Task 2: MissionHub, Browser-API und SSE

**Files:**
- Create: `src/missions/mod.rs`, `src/missions/rules.rs`, `src/web/missions.rs`
- Modify: `src/lib.rs`, `src/app.rs` (`AppState.missions`, Routen), `src/testing.rs`, `src/contract/live.rs`, `src/web/live.rs`

**Interfaces:**
- Consumes: `store::missions::*` aus Task 1.
- Produces:
  - `missions::rules::transition(from: MissionState, to: MissionState) -> Result<(), String>` – erlaubt `Prepared→Running`, `Prepared→Closed`, `Running→Closed`; sonst Fehlertext
  - `MissionHub::start_up(pool: SqlitePool, map: MapHub) -> Result<MissionHub, AppError>` (setzt den Pilot-Sichtbereich, ab Task 4 wirksam)
  - `MissionHub::{subscribe() -> broadcast::Receiver<()>, list(), get(id), create(MissionCreate, created_by, now_ms), start(id, now_ms), close(id, now_ms), add_section(id, SectionCreate, now_ms), assign_flight(flight_id, MissionAssign)}` → jeweils `Result<Mission, AppError>` bzw. `Result<(), AppError>`; jede Änderung sendet `()` an `subscribe`
  - `LiveEvent::MissionsChanged` (Einheitsvariante, `{"type":"missions_changed"}`)
  - Routen: `GET /api/missions` (Viewer), `POST /api/missions` (Operator), `POST /api/missions/{id}/start`, `POST /api/missions/{id}/close`, `POST /api/missions/{id}/sections` (Operator), `PUT /api/flights/{id}/mission` (Operator, Body `MissionAssign`)

- [ ] **Step 1: Tests schreiben (rot):**
  - `rules::tests::only_forward_transitions` – die drei erlaubten, `Closed→Running` und `Running→Prepared` sind `Err`
  - `web::missions::tests::operator_prepares_starts_and_closes` – POST → 200 `state: "prepared"`; start → `running`; close → `closed`, `closed_at_ms` gesetzt
  - `starting_a_second_mission_is_409`
  - `closing_with_an_open_flight_is_422` – Flug ohne `ended_at_ms` im laufenden Einsatz
  - `closed_mission_takes_no_section_422`
  - `kiosk_reads_but_cannot_change` – GET 200, POST 403; ohne Sitzung 401
  - `empty_name_is_400`
  - `assigning_a_flight_that_already_has_a_mission_is_422`, unbekannter Flug oder Einsatz → 404
  - `web::live::tests::mission_changes_reach_the_browser` – nach `start` erscheint `{"type":"missions_changed"}` im Strom
- [ ] **Step 2: Implementieren.** `MissionHub` prüft Übergänge mit `rules::transition` (→ 422), Name/Label mit `map::rules::name`-Art (1–120 Zeichen, getrimmt → 400), schließt nicht bei `has_open_flight` (→ 422). `start`/`close` setzen zusätzlich `map.set_pilot_scope(...)` (Task 4; bis dahin als leere Methode im `MapHub` anlegen). `web::live` nimmt `missions.subscribe()` als dritten Strom neben Karte und Fotos (`stream::select`).
- [ ] **Step 3:** `cargo test missions` und `cargo test web::live` grün.
- [ ] **Step 4:** `scripts/check.sh`, Commit `SUL-65: Einsätze anlegen, starten, abschließen`.

### Task 3: Bereich in Flug-, Foto- und Karten-API

**Files:**
- Modify: `src/web/flights.rs`, `src/web/photos.rs`, `src/web/map.rs`, `src/map/mod.rs`, `src/photos/mod.rs`, `src/web/missions.rs`

**Interfaces:**
- Consumes: `list_in`, `in_mission`, `since_unassigned`, `assign_element` (Task 1).
- Produces:
  - `web::missions::parse_scope(raw: Option<&str>) -> Result<Option<Option<i64>>, AppError>` – fehlt → `None` (alles wie bisher), `"none"` → `Some(None)`, Zahl → `Some(Some(n))`, sonst 400
  - `GET /api/flights?mission=…`, `GET /api/photos?mission=…` (bei Zahl alle Fotos des Einsatzes ohne Zeitgrenze; bei `none` und ohne Parameter wie bisher `since_ms`), `GET /api/map/elements?mission=…`
  - `MapHub::list_in(mission: Option<i64>)`, `MapHub::assign(id, mission_id) -> Result<MapElement, AppError>` (veröffentlicht `MapChange::Updated`)
  - `PUT /api/map/elements/{id}/mission` (Operator, Body `MissionAssign`)

- [ ] **Step 1: Tests schreiben (rot):**
  - `flights_are_filtered_by_mission` – je ein Flug in A und ohne Einsatz; `?mission=<A>` liefert nur A, `?mission=none` nur den anderen, ohne Parameter beide; `?mission=abc` → 400
  - `browser_draws_into_the_selected_mission` – POST mit `mission_id: A` (vorbereitet) → Element mit `mission_id == A`
  - `elements_of_a_closed_mission_are_read_only` – PUT und DELETE → 422; POST mit `mission_id` eines abgeschlossenen → 422
  - `unassigned_element_can_be_assigned_once` – PUT `/mission` → 200, zweites Mal → 422
  - `photos_of_a_mission_ignore_the_24h_window` – Foto älter als 24 h in A erscheint bei `?mission=<A>`
- [ ] **Step 2: Implementieren.** Der Abschluss-Check sitzt im `MapHub` (`create` mit Ziel-Einsatz, `apply`, `delete`): Ist der Einsatz des Elements `closed` → `AppError::Unprocessable("Einsatz ist abgeschlossen")`. Unbekannter Ziel-Einsatz → 422.
- [ ] **Step 3:** `cargo test web` grün.
- [ ] **Step 4:** `scripts/check.sh`, Commit `SUL-65: Flüge, Fotos und Kartenelemente je Einsatz`.

### Task 4: Pilot 2 sieht den laufenden Einsatz

**Files:**
- Modify: `src/map/mod.rs`, `src/web/pilot_map.rs`, `src/app.rs`

**Interfaces:**
- Consumes: `MissionHub::start_up` ruft `map.set_pilot_scope(running_id)`; `start`/`close` rufen es erneut.
- Produces:
  - `MapHub::set_pilot_scope(&self, mission: Option<i64>)` – speichert in einem `watch::Sender<Option<i64>>` und veröffentlicht `MapChange::Refresh`
  - `MapHub::pilot_scope(&self) -> Option<i64>`
  - neue Variante `MapChange::Refresh`; `web::live` ignoriert sie, `push_to_pilot` sendet `dji::push_group_refresh`

- [ ] **Step 1: Tests schreiben (rot), in `web::pilot_map::tests`:**
  - `groups_show_the_running_mission_or_the_unassigned` – Elemente in A, B, ohne; ohne laufenden Einsatz liefert `groups` nur „ohne“; nach Start von A nur A
  - `pilot_draws_into_the_running_mission` – Pilot-POST, während A läuft → `mission_id == A`
  - `prepared_mission_elements_are_not_pushed_to_pilot` – Browser legt Element in vorbereitetem B an, A läuft → WebSocket bekommt keine `map_element_create`; Element in A → wird gepusht
  - `starting_a_mission_sends_group_refresh` – WebSocket bekommt `map_group_refresh` nach `start`
  - `pilot_scope_survives_restart` – Einsatz laufen lassen, neuen `MapHub` + `MissionHub::start_up` auf demselben Pool → `pilot_scope() == Some(A)`
- [ ] **Step 2: Implementieren.** `groups` und Pilot-`create` nutzen `pilot_scope()`. `push_to_pilot` filtert `Created`/`Updated` mit `element.mission_id != pilot_scope()` heraus; ein `Updated`, das ein bisher sichtbares Element aus dem Bereich nimmt (Zuordnen), löst `push_group_refresh` statt des Updates aus. `Deleted` geht immer raus.
- [ ] **Step 3:** `cargo test pilot_map` grün.
- [ ] **Step 4:** `scripts/check.sh`, Commit `SUL-65: Pilot 2 sieht die Elemente des laufenden Einsatzes`.

### Task 5: Browser – Einsätze verwalten und auswählen

**Files:**
- Create: `frontend/src/missions/scope.ts`, `frontend/src/missions/scope.test.ts`, `frontend/src/missions/useMissionScope.ts`, `frontend/src/missions/MissionsPage.tsx`, `frontend/src/missions/MissionsPage.test.tsx`, `frontend/src/missions/MissionBar.tsx`, `frontend/src/missions/missions.css`
- Modify: `frontend/src/api/queries.ts`, `frontend/src/live/LiveProvider.tsx`, `frontend/src/app/router.tsx`, `frontend/src/shell/Rail.tsx`, `frontend/src/test/handlers.ts`

**Interfaces:**
- Consumes: Vertragstypen aus Task 1, Routen aus Task 2.
- Produces:
  - `type MissionScope = { kind: 'mission'; mission: Mission } | { kind: 'none' }`
  - `resolveScope(param: string | null, missions: Mission[]): MissionScope` – Zahl eines bekannten Einsatzes → dieser; `'ohne'` → none; sonst laufender Einsatz, ohne laufenden none
  - `scopeQuery(scope: MissionScope): string` → `'<id>'` oder `'none'`
  - `inScope(missionId: number | null, scope: MissionScope): boolean`
  - `useMissionScope(): { scope: MissionScope; setScope(param: string): void }` – liest/schreibt `?einsatz=`; im Kiosk immer der laufende
  - `useMissions()` mit `queryKeys.missions = ['missions']`; `LiveProvider` invalidiert bei `missions_changed`
  - Route `/einsaetze` (admin, operator), Rail-Eintrag „Einsätze“ zwischen „Lage“ und „Flüge“

- [ ] **Step 1: Tests schreiben (rot):**
  - `scope.test.ts`: `resolveScope('7', [m7])` → mission 7; `resolveScope('999', [running])` → running; `resolveScope('ohne', [running])` → none; `resolveScope(null, [])` → none; `resolveScope('abc', [running])` → running
  - `MissionsPage.test.tsx`: legt „Vermisste Person Ebstorf“ an → erscheint unter „Vorbereitet“; „Starten“ → unter „Laufend“; „Abschließen“ fragt nach und verschiebt nach „Abgeschlossen“; 409 beim Starten zeigt „Es läuft schon ein Einsatz“
- [ ] **Step 2: Implementieren.** `MissionsPage`: drei Gruppen, Formular Name/Ort, Aktionen Starten, Abschließen (mit Bestätigung), Abschnitt anlegen, „Auf der Lage öffnen“ (`/?einsatz=<id>`). `MissionBar`: Name, Zustand als Tag, aktiver Abschnitt, Auswahl (laufend, vorbereitete, „ohne Einsatz“, abgeschlossene). MSW-Handler für `/api/missions` mit einem laufenden und einem vorbereiteten Beispiel.
- [ ] **Step 3:** `pnpm -C frontend test missions` grün.
- [ ] **Step 4:** `scripts/check.sh --frontend`, Commit `SUL-65: Einsätze im Browser verwalten`.

### Task 6: Browser – Lage, Flüge und Großbild je Einsatz

**Files:**
- Modify: `frontend/src/views/useLiveMapData.ts`, `frontend/src/views/LagePage.tsx`, `frontend/src/views/WorkstationView.tsx`, `frontend/src/views/MobileView.tsx`, `frontend/src/views/KioskView.tsx`, `frontend/src/map/useMapEditor.tsx`, `frontend/src/map/ElementForm.tsx`, `frontend/src/flights/FlightsPage.tsx`, `frontend/src/flights/FlightTable.tsx`, `frontend/src/api/queries.ts`, zugehörige Tests

**Interfaces:**
- Consumes: `useMissionScope`, `inScope`, `scopeQuery`, `MissionBar` (Task 5).
- Produces: `useFlights(scope: MissionScope)` (Key `['flights', scopeQuery(scope)]`), `useMapPhotos(scope)` statt `useRecentPhotos` (Key `['photos', 'map', scopeQuery(scope)]`), `useLiveMapData(state, focus, scope)`.

- [ ] **Step 1: Tests schreiben (rot):**
  - `WorkstationView.test.tsx`: zeigt nur Elemente des gewählten Einsatzes; Zeichnen schickt `mission_id` des gewählten; bei abgeschlossenem Einsatz sind Zeichenwerkzeuge aus
  - `KioskView.test.tsx`: zeigt die Elemente des laufenden Einsatzes und dessen Namen, ignoriert `?einsatz=`
  - `FlightsPage.test.tsx`: lädt `/api/flights?mission=<id>`; unter „ohne Einsatz“ hat jede Zeile „Einsatz zuordnen“, das `PUT /api/flights/{id}/mission` sendet
  - `ElementForm`: bei Element ohne Einsatz Auswahl „Einsatz zuordnen“
- [ ] **Step 2: Implementieren.** Live-Flugzeug und laufende Spur bleiben in jeder Auswahl sichtbar (sie sind die aktuelle Lage). `MissionBar` über Lagekarte (Arbeitsplatz, mobil) und Flugliste; das Großbild zeigt nur den Namen des laufenden Einsatzes.
- [ ] **Step 3:** `pnpm -C frontend test` grün, Vorschau-Modus (`?rolle=operator`) einmal ansehen.
- [ ] **Step 4:** `scripts/check.sh --frontend`, Commit `SUL-65: Lage, Flüge und Großbild je Einsatz`.

### Task 7: Abschluss

**Files:**
- Modify: `Cargo.toml` (Version 0.4.0), `docs/betrieb/runbook-deployment.md` (Hinweis Migration 0004), `frontend/e2e/smoke.spec.ts`, `scripts/dev-smoke.sh`
- Create: `docs/betrieb/einsaetze-checkliste.md`

- [ ] **Step 1:** Smoke-Test um „Einsatz anlegen, starten, Simulatorflug erscheint in seiner Flugliste“ ergänzen; `scripts/dev-smoke.sh` gegen den Simulator laufen lassen (erwartet: Flug mit `mission_id` des gestarteten Einsatzes in `GET /api/flights?mission=<id>`).
- [ ] **Step 2:** Checkliste für die Feldprobe: Einsatz vorbereiten mit Suchgebiet, starten, Pilot 2 zeigt das Suchgebiet (ggf. Karte neu öffnen), Punkt aus Pilot 2 landet im Einsatz, Flug erscheint nur in dessen Flugliste, Abschließen, Pilot 2 zeigt „ohne Einsatz“.
- [ ] **Step 3:** `scripts/check.sh`, Commit `SUL-65: Version 0.4.0, Smoke und Checkliste`; PR gegen `main`.

## Offene Punkte und Annahmen

1. **Neuladen in Pilot 2:** `map_group_refresh` ist der vorhandene Weg (M2). Ob Pilot 2 daraufhin die Gruppe wirklich neu lädt, zeigt erst die Feldprobe; Rückfall laut Spec: Hinweis an den Piloten.
2. **Bereich des Lage-Kerns:** Der Lage-Kern kennt keine Einsätze; `FlightStarted` im SSE-Strom trägt keinen Einsatz. Die Flugliste lädt ohnehin per REST mit Filter neu. Statt den laufenden Einsatz in den `SituationSnapshot` zu legen (Spec §4, letzter Absatz), meldet `missions_changed` jede Änderung und die Ansichten laden `/api/missions` neu; das hält `src/situation/` rein und erreicht dasselbe Umschalten.
3. **Löschen von Einsätzen** gibt es nicht; ein vorbereiteter Einsatz wird abgeschlossen (verworfen).
4. **Kartenfoto (Papierkarte)** und alle Flugmappe-Felder kommen mit SUL-66.
