# Lifeline Hub als Vorlage für Suchlage

- **Stand:** 16.10.2026, gelesen am Repo `~/dev/personal/lifeline-hub` (Version `1.0.0-alpha.120`, Commit `3eb9115f2`)
- **Zweck:** Klären, was Suchlage aus Lifeline Hub übernimmt, wie man es übernimmt und was man bewusst weglässt.
- Pfade mit `LH:` beginnen im Lifeline-Repo, Pfade ohne Präfix meinen das künftige Suchlage-Repo.

> Kurz gesagt: Das **Designsystem** (Tokens, antd-Theme, Schriften, Bausteine) lässt sich gut kopieren, wenn man die Fachpaletten herausnimmt. Den **Rahmen (Shell)** baut man neu, denn er hängt am Einsatz-/Modulmodell. Beim **Backend** übernimmt man Muster und einzelne kleine Dateien, nicht die Codebasis. Lifeline ist mit 183 Migrationen, rund 70 Fachmodulen und vielen Guards für ein kleines Projekt um ein Vielfaches zu groß.

---

## A) Designsystem und UI

### A.1 Quellen der Wahrheit

| Was | Datei | Bemerkung |
|---|---|---|
| Entwurf „Instrumententafel“ | `LH:docs/design/2026-09-21-neuentwurf/neuentwurf.dc.html` (S1 Systemkarte, S2–S7 Screens), `shell.dc.html` (Rahmen) | Maßgeblich sind die **Inline-Styles**. Das mitgelieferte `_ds/…/tokens/*.css` zeigt den alten Stand und gilt nicht. |
| Umsetzungsgrundlage | `LH:docs/design/2026-09-21-neuentwurf/umsetzung.md` | Palette, Rahmenmaße, Bausteine, Typografie. Lifeline-spezifisch sind die Abschnitte zu Sichtung, ETB, Modulen und Einheitenstatus. |
| Regelkarte Frontend | `LH:frontend/AGENTS.md` (Abschnitt „Gestaltungssprache Neuentwurf“) | Kurzfassung aller UI-Regeln |
| Claude Design | Design System **„Lifeline Hub“** im Claude-Design-Projekt (laut `LH:frontend/AGENTS.md`) | Guter Startpunkt, um Suchlage-Screens zu entwerfen, bevor Code entsteht (`DesignSync`) |

### A.2 Was das Designsystem ist

**Gestaltungsgrundsätze** (aus `umsetzung.md` und `tokens.ts`):

- **Nachtbetrieb ist die Vorgabe.** Ohne gespeicherte Wahl startet die App dunkel. Die helle Palette ist aus der dunklen abgeleitet, mit denselben Rollen.
- **Radius 0 überall.** Flächen werden durch Haarlinien getrennt. Das „Fugenraster“ ist ein Grid mit `gap: 1px` auf der Linienfarbe.
- **Rot bedient nichts.** `bedien` ist Blau (`#4d94d6`), `marke` ist Rot (`#a8071a`, nur Dekoration), `alarm` ist ein eigenes Rot.
- **Zweiter Kanal ist Pflicht.** Ein Status trägt immer auch ein Wort, eine Zahl oder ein Kürzel, nie nur Farbe.
- **Status als getönte Fläche mit getöntem Text** („Ampel als Fläche, Zahl bleibt lesbar“).
- **Typografie:** Archivo 400/500/600, Archivo Narrow 600 für Köpfe, JetBrains Mono für alle Zahlen, Zeiten, Koordinaten und Kennungen (`tabular-nums`). Die Augenbraue ist 10 px, 600, in Versalien, mit Sperrung .14em.
- **Kontraste sind gerechnet.** Text hält am Tag mindestens 7 : 1 und nachts mindestens 5 : 1, Steuerrahmen mindestens 3 : 1. Die Messwerte stehen als Kommentar an den Werten.
- **Dichte-Staffel** `kompakt` 30 px · `komfortabel` 48 px · `handschuh` 72 px Steuerhöhe. Sie wirkt auf antds `controlHeight`, auf Abstände und auf die Grundschrift.

**Technischer Aufbau:**

| Schicht | Datei(en) | Inhalt |
|---|---|---|
| Rollen (TS) | `LH:frontend/src/theme/tokens.ts` (1021 Z.) | `Farbrollen` (≈45 Rollen) als `farbenDunkel` und `farbenHell`, `rahmenFarben` (modusunabhängig dunkel), `dichten`, `abstand`, `flaeche`, `seitenrinne`, `form` (Radius 0, Übergang 120 ms), `schrift`, `schriftskala` sowie die antd-Brücke `antdToken()`, `antdKomponenten()`, `antdAlgorithmus()`, `antdKnopf/Klappkopf/Kaestchen/Optionsknopf()` |
| Rollen (CSS) | `LH:frontend/src/theme/rollen.css` (364 Z.) | dieselben Werte als statische `--lfh-*`-Properties unter `:root` und `[data-theme='dark']` sowie `[data-dichte=…]` und `--lfh-typo-*`. Statisch, damit der erste Frame stimmt (kein weißer Blitz). |
| Drift-Schutz | `LH:frontend/src/theme/rollen.guard.test.ts` | parst `rollen.css` und vergleicht Wert für Wert mit `tokens.ts` |
| Baustein-CSS | `LH:frontend/src/theme/sprache.css` (387 Z.) | nur, was inline nicht geht. Liest ausschließlich `--lfh-*`. |
| Provider | `LH:frontend/src/theme/ThemeModeProvider.tsx` | `ConfigProvider` mit `theme={{ algorithm, token, components }}`, `locale=de_DE`, Modus `system/light/dark` (Vorgabe `dark`), Dichte, Helligkeit (Abdunkelung). Schreibt `data-theme`, `data-dichte` und `--lfh-abdunkelung` an `<html>`. |
| FOUC-Schutz | `LH:frontend/index.html` (Inline-Skript, Z. ≈14–30) | setzt `data-theme` vor dem React-Mount. Spiegelt `MODUS_DEFAULT` und muss mit dem Provider zusammen geändert werden. |
| Schriften | `LH:frontend/src/theme/schriften.ts`, `LH:frontend/src/assets/fonts/*.woff2` (+ `LIESMICH.md`, `lizenzen/`) | lokal über die FontFace-API, `display: swap`, kein CDN. Arimo kommt aus `@einsatzzeichen/core/fonts/text-medium.woff2`. |
| Hilfen | `theme/dichte.ts`, `theme/helligkeit.ts`, `lib/sichererSpeicher.ts` (localStorage mit try/catch), `components/useViewport.ts` | Abhängigkeiten des Providers |
| Bausteine | `LH:frontend/src/components/instrument/` (Import über `index.ts`) | `Augenbraue`, `Paneel`/`PaneelZeile`, `Kennzahl`/`Kennzahlenband`, `Aufgliederung`/`Balken`, `StatusZelle`/`StatusChip`, `Segmentleiste` (statt antd `Segmented`), `Sammelbanner`, `Zeitachseneintrag`, `Schnellerfassungszeile`, `Datenraster`/`Datenfeld` (statt `Descriptions`), `PaneelZustand`, `rollenwerte`/`useRollen` |
| Icons | `LH:frontend/src/icons/` (`IconRahmen.tsx`, `erzeugt.generated.ts`), Quelle `LH:scripts/icons/` (`icons.json`, `quellen/`, `erzeuge-icons.mjs`) | Icons8 „iOS 27 Outlined“, aktive Zustände „Filled“, als generierte React-Komponenten |
| Bildmarke | `LH:frontend/src/marke/` | Lifeline-Marke. **Nicht übernehmen**, Suchlage braucht eine eigene. |

So nutzt `antdToken()` die Rollen (gekürzt):

```ts
colorPrimary: farben.bedien, colorError: farben.alarm, colorErrorText: farben.alarmText,
colorBgLayout: farben.grund, colorBgContainer: farben.flaeche, colorBgElevated: farben.flaeche2,
colorText: farben.text, colorTextSecondary: farben.gedaempft, colorBorder: farben.steuerRahmen,
borderRadius: 0, fontFamily: schrift.text, fontFamilyCode: schrift.zahl,
controlHeight: stufe.zeilenhoehe, padding: stufe.abstand.md, motionDurationMid: '120ms',
```

Nachts läuft `[theme.darkAlgorithm, seedTreu]`. `seedTreu` setzt die Signalfarben nach dem antd-Dark-Algorithmus zurück auf ihre Rollenwerte, weil antd Seed-Farben sonst umrechnet.

### A.3 Rahmen (Shell)

Die Maße stehen in `shell.dc.html` und `umsetzung.md` § Rahmen:

- **Kopfleiste 52 px** (`#0c0e11`): Markenzelle 60, Wortmarke Mono 12, Statuspunkt mit Kontext, Suchfeld (⌘K), SYNC-Anzeige, Uhr Mono 14, Initialen.
- **Rail 60 px:** Icon 20 px mit 9-px-Etikett darunter, Zeile 62 px. Aktiv ist die Zeile `flaeche3` mit einer 2-px-Marke links in `marke`.
- **Modulpanel 208 px**, Seitenkopf 44 px, Sprungpalette 640 px breit.
- Code: `LH:frontend/src/components/AppLayout.tsx`, `Kopfleiste.tsx`, `einsatz/IconRail.tsx`, `einsatz/ModulPanel.tsx`, `command-palette/`.

**Nicht kopieren.** `AppLayout` und `Kopfleiste` importieren `api/einsaetze`, `auth/AuthContext`, `live/*`, `offline/queue` und `routing/deeplinks`, hängen also am Einsatz- und Modulmodell. Für Suchlage baut man den Rahmen nach den Maßen neu: Kopf, Rail mit 3–4 Einträgen (Lage, Flüge, System, Einstellungen) und ohne Modulpanel. Die Rollen dafür (`rahmenFarben`, `KOPF_HOEHE`-Logik) kommen aus `tokens.ts`.

### A.4 Karte (MapLibre, Einsatzzeichen, terra-draw)

- **MapLibre 6** (`maplibre-gl ^6.13`). In `LH:frontend/src/pages/lagekarte/Kartenflaeche.tsx` (1717 Z.) gilt:
  - Import als Namensraum: `import * as maplibregl from 'maplibre-gl'`. Ab v6 gibt es keinen Default-Export, und `Map` würde den globalen `Map` verschatten.
  - Die Worker-URL wird explizit gesetzt: `LH:…/lagekarte/maplibreWorker.ts` mit `maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url` und `setWorkerUrl()`, importiert vor der ersten Karte.
  - `zoomLevelsToOverscale: undefined` stellt das v5-Verhalten wieder her. `touchPitch: false` und `maxPitch: 0` halten die Draufsicht.
  - Kein `NavigationControl`, stattdessen ein eigener Knopfblock.
  - `map.setMissingStyleImageResolver(…)` liefert Zeichen als Canvas-Bild (`kartenbildResolver.ts`).
- **Basemap:** `LH:…/lagekarte/basemapStil.ts` baut einen eigenen Vektorstil für das **Shortbread-Schema** mit Hell- und Dunkelpalette (`FARBEN.light/dark`). Offline liefern PMTiles, die das Backend unter `/api/karte/…` ausliefert (Proxy, Offline-Katalog, `karten-service`). Für Suchlage kommt nur der **Dunkelstil** als Vorlage in Frage, die Kartenquelle ist neu zu entscheiden (siehe C).
- **Einsatzzeichen:** `addSymbolImage(map, id, drawing, { size, pixelRatio })` aus `@einsatzzeichen/maplibre`. Die Typen (`Drawing`) kommen aus `@einsatzzeichen/schema`, React-Darstellung aus `@einsatzzeichen/react` (`LH:frontend/src/zeichen/EinsatzZeichen.tsx`).
- **terra-draw** (`terra-draw ^1.37`, `terra-draw-maplibre-gl-adapter ^1.5`) steckt in `LH:…/lagekarte/zeichnen.ts` (Polygon/Linie mit Undo) und `messZeichnung.ts`/`messung.ts` (Messwerkzeug). Das ist eine kompakte, gut übertragbare Vorlage für die späteren **Suchsektoren**:

```ts
const draw = new TerraDraw({
  adapter: new TerraDrawMapLibreGLAdapter({ map, prefixId: praefix }),
  modes: [new TerraDrawPolygonMode({ keyEvents }), new TerraDrawLineStringMode({ keyEvents })],
  undoRedo: { modeLevel: new TerraDrawModeUndoRedo() }, // ohne modeLevel liefert undo() konstant false
});
```

### A.5 Statusanzeigen und Live-Zustand im Frontend

- `StatusZelle`/`StatusChip` (`components/instrument/Status.tsx`) zeigen eine getönte Fläche mit Wort. Die Übersetzung Rolle → Fläche geschieht nur in `statusFlaeche.ts`.
- Live-Verbindung: `LH:frontend/src/live/liveVerbindung.ts` (genau eine `EventSource`, Neuaufbau, `Last-Event-ID`), `liveStatusStore.ts` (Zustand für die SYNC-Anzeige im Kopf), `LiveStatusBanner.tsx`, `liveInvalidierung.ts` (Ereignis → `queryClient.invalidateQueries`, gebündelt).
- Für die Suchlage-Telemetrie mit 1 Hz ist **Invalidieren und Neuladen das falsche Muster**. Der Push trägt dort selbst den Zustand und wird direkt in einen Store geschrieben. Übertragbar sind das Verbindungsmanagement und die SYNC-Anzeige.

### A.6 Übernahme ins zweite Projekt: Dateien kopieren

Ein eigenes npm-Paket für das Designsystem wäre für zwei Projekte Overengineering. Die Dateien werden **kopiert** und im Suchlage-Repo als „Stand aus Lifeline vom 16.10.2026“ vermerkt.

**Kopierliste** (Ziel `frontend/src/…` in Suchlage):

1. `theme/tokens.ts`, **um die Fachpaletten gekürzt**: `sichtungsfarben`, `etbTypFarben*`, `warnstufeFarben*` und `fachebeneFarben*` entfernen. Farbrollen, `rahmenFarben`, `paletteMaske`, Dichte, Form, Schrift, `schriftskala` und alle `antd*()`-Funktionen bleiben.
2. `theme/rollen.css`, gekürzt um dieselben Paletten. `theme/sprache.css` kommt mit.
3. `theme/rollen.guard.test.ts` **mitnehmen**. Ohne ihn driften zwei Wahrheitsquellen auseinander. Die Kontrasttests (`textstufen.test.ts`, `bedienKontrast.test.ts` u. a.) sind optional. Sie lohnen erst, wenn man die Palette ändert.
4. `theme/ThemeModeProvider.tsx`, `theme/dichte.ts`, `theme/helligkeit.ts`, `lib/sichererSpeicher.ts`, `components/useViewport.ts`. Den Speicherschlüssel `lifeline-hub.theme` auf `suchlage.theme` umbenennen, auch im `index.html`-Skript.
5. Das Bootstrap-Skript aus `LH:frontend/index.html` übernehmen, samt `<meta name="theme-color" content="#0c0e11">`.
6. `theme/schriften.ts` und `assets/fonts/` mit `LIESMICH.md` und `lizenzen/` (SIL OFL 1.1). Die Familiennamen `LFH …` dürfen bleiben oder werden einheitlich umbenannt, dann in `tokens.ts`, `rollen.css` und `schriften.ts` gleichzeitig.
7. `theme/statusFarben.ts` **nicht kopieren**. Die Datei importiert `api/fachebenen`, `api/offlineKarten` und `api/types`. Aus ihr braucht man nur `type Statusrolle = 'alarm' | 'achtung' | 'normal' | 'neutral' | 'bedien' | 'marke'` und ggf. `StatusDarstellung`. Beides gehört in eine neue, kleine `theme/status.ts`.
8. `components/instrument/`: zuerst die Dateien ohne API-Kopplung, also `Augenbraue`, `Paneel`, `rollenwerte`, `fugenraster`, `zielEinzug`, `Segmentleiste`, `Status` + `statusFlaeche` (Import auf `theme/status.ts` umbiegen), `Kennzahl` + `Aufgliederung` sowie `Datenraster`/`datenrasterStil`. `Zeitachseneintrag`, `Sammelbanner`, `Formularpaneel` und `PaneelZustand` nur bei Bedarf und nach dem Entkoppeln (`api/types`, `Datenstand`, `icons`). Die zugehörigen `*.test.tsx` kommen mit.
9. Icons: `icons/IconRahmen.tsx` und den Generator `scripts/icons/erzeuge-icons.mjs` übernehmen, das Register `icons.json` auf die Suchlage-Icons reduzieren. **Lizenz:** Die SVGs liegen unter einem bestehenden Icons8-Abo. Ob es ein zweites Projekt deckt, ist vor dem Einchecken zu prüfen.

**Abhängigkeiten** (Versionen wie in `LH:frontend/package.json`): `antd ^6.6.5`, `react ^19.3`, `react-dom`, `react-router ^8.4`, `@tanstack/react-query ^5.104`, `dayjs`, `maplibre-gl ^6.13`, `terra-draw ^1.37`, `terra-draw-maplibre-gl-adapter ^1.5` und `@einsatzzeichen/{core,maplibre,react,schema}`. Dazu die Dev-Abhängigkeiten `vite ^8.3`, `@vitejs/plugin-react ^6`, `typescript ^6`, `vitest ^5`, `@testing-library/*`, `jsdom`, `eslint ^10` + `typescript-eslint`, `prettier`, optional `msw`, `@playwright/test` und `vite-plugin-pwa`.

### A.7 Herkunft und Einbindung von `@einsatzzeichen/*`

- Die Pakete liegen in der **öffentlichen npm-Registry** (registry.npmjs.org). Lizenz MIT, Quelle `github.com/rubenvitt/einsatzzeichen` (eigene Bibliothek). Veröffentlicht wird per npm Trusted Publisher (OIDC). Man braucht **kein `.npmrc`, kein GitHub Packages und keinen Token**.
- Pakete: `@einsatzzeichen/schema` (Typen), `core` (hängt an `schema`, liefert Rendering und Schriften), `maplibre` (`addSymbolImage`), `react` (Komponente).
- **Version:** Lifeline pinnt exakt `5.2.0` (11.10.2026). Seit 13.–14.10.2026 gibt es `6.0.0` bis `6.4.0` (Major). Für Suchlage gibt es zwei Möglichkeiten:
  - **5.2.0:** Der kopierte Lifeline-Code läuft damit sicher.
  - **6.x:** Vorher das Changelog im einsatzzeichen-Repo auf Breaking Changes prüfen (`addSymbolImage`, `Drawing`).
  - Empfehlung: Mit **6.x** starten, weil Suchlage neu ist und kaum Zeichencode übernimmt, und dann exakt pinnen.
- **pnpm 11 und `minimumReleaseAge`:** Frisch veröffentlichte Versionen weist pnpm ab. Lifeline löst das in `LH:frontend/pnpm-workspace.yaml` mit exakten Ausnahmen:

```yaml
minimumReleaseAgeExclude:
  - '@einsatzzeichen/core@6.4.0'
  - '@einsatzzeichen/maplibre@6.4.0'
  - '@einsatzzeichen/react@6.4.0'
  - '@einsatzzeichen/schema@6.4.0'
```

- Die Schrift Arimo wird direkt aus dem Paket importiert (`import arimo500 from '@einsatzzeichen/core/fonts/text-medium.woff2'`). Sie muss mit der Paketversion mitwandern, weil das Paket an ihr Breiten misst.

---

## B) Rust-Architektur

### B.1 Workspace und Modulstruktur

- `LH:Cargo.toml`: ein Workspace mit dem Wurzelpaket `lifeline-hub` und den Mitgliedern `crates/karten-katalog`, `karten-service` und `src-tauri`. Die Version steht einmal in `[workspace.package]`. Edition 2021.
- **Lib/Bin-Trennung:** `src/lib.rs` exportiert alle Module, `src/main.rs` ist dünn. So testen Integrationstests unter `tests/` gegen `lifeline_hub::app::build_router`.
- **Schnitt je Fachdomäne:** `src/<domain>/mod.rs` (Typen, DTOs, `#[derive(ToSchema)]`) + `src/<domain>/repo.rs` (SQL) + `src/routes/<domain>.rs` (Handler). Querschnitt liegt in eigenen Dateien: `error.rs`, `extract.rs`, `db.rs`, `tx.rs`, `static_files.rs`, `config.rs`, `app.rs`, `live/` und `auth/`.
- **Für Suchlage:** **ein Crate, kein Workspace.** Vorgeschlagener Schnitt nach den Bausteinen der Spec:

```
src/
  main.rs        // Logging, Config, Start, Shutdown
  lib.rs
  app.rs         // AppState + build_router
  config.rs      // nur Bootstrap (siehe B.7)
  error.rs       // AppError → {"error": "..."}
  extract.rs     // JsonBody, PfadParam (aus Lifeline)
  db.rs          // connect, migrate, test_pool
  static_files.rs
  auth/          // session.rs, oidc.rs, einrichtung.rs, zugang.rs (Großbild-Token)
  dji/           // Adapter: MQTT (rumqttc), Pilot-Seite, Übersetzung → eigene Ereignisse
  lage/          // Lage-Kern: Flugerkennung, Live-Zustand, Verbreitung (broadcast)
  flug/          // mod.rs + repo.rs: Flüge, Spurpunkte
  routes/        // health, auth, live, flug, einstellungen, pilot
migrations/
tests/common/mod.rs
```

### B.2 axum-0.8-App

- **State:** `LH:src/app.rs` → `#[derive(Clone)] pub struct AppState { pool: SqlitePool, live: LiveHub, … }`. Teure Clients liegen als Felder darin. Einige prozessweite Schalter hält Lifeline als `OnceLock`, etwa `oidc_konfiguriert` und `cookie_secure`. Für Suchlage gehört **alles in den State**, denn `OnceLock`-Globals erschweren Tests und das Ändern von Einstellungen zur Laufzeit.
- **Router:** `build_router_mit(state, RouterOptionen)` mit `.route(…)` je Pfad (axum-0.8-Syntax `{id}`) und `.fallback(static_files::serve)` für die SPA.
  - Layer von innen nach außen: `DefaultBodyLimit` → Kompression (nur JSON ≥ 1 KiB) → `CatchPanicLayer` → Request-Gate → `TraceLayer` mit Request-ID → `SetRequestId`/`PropagateRequestId` → `X-Content-Type-Options: nosniff`.
  - Für Suchlage reichen BodyLimit, CatchPanic, Trace mit Request-ID und `nosniff`.
- **Fehlertyp:** `LH:src/error.rs`. `enum AppError { Unauthorized, Forbidden, NotFound, Validation(String), Conflict(String), UnprocessableEntity(String), Database(sqlx::Error), Internal(String), ServiceUnavailable(String), … }`, dazu `From<sqlx::Error>`. `IntoResponse` liefert `{"error": "<Meldung>"}`.
  - Interne und Datenbankfehler werden geloggt und nach außen nur als „Interner Serverfehler“ gemeldet.
  - Sicherheitsnetz: UNIQUE/FK-Verletzungen werden zu 409, CHECK-Verletzungen zu 422, `SQLITE_BUSY`/PoolTimeout zu 503.
  - Statuscode-Konvention (`LH:src/AGENTS.md`): **400** für ein einzelnes Feld, **422** für Zusammenhang oder Zustandsübergang, **409** für Nebenläufigkeit oder Lebenszyklus.
- **Extractors:** `LH:src/extract.rs`. `JsonBody<T>` und `PfadParam<T>` ersetzen `Json`/`Path`, damit auch Rejections im `{error}`-Format ankommen. Auth-Extractors stehen in `LH:src/auth/session.rs`: `CurrentUser`, `AdminUser`, `AktuelleSitzung`, jeweils `impl FromRequestParts<AppState>`.
- **OpenAPI-Codegen** (`utoipa 6` → `frontend/src/api/openapi.json` → `openapi-typescript`): Ermessensfrage, siehe C.

### B.3 sqlx/SQLite und Migrationen

`LH:src/db.rs`:

```rust
let options = SqliteConnectOptions::new()
    .filename(db_path).create_if_missing(true)
    .journal_mode(SqliteJournalMode::Wal)
    .busy_timeout(Duration::from_secs(5))
    .foreign_keys(true);
SqlitePoolOptions::new().max_connections(5)
    .acquire_timeout(Duration::from_secs(10))   // schneller 503 statt 30-s-Hänger
    .connect_with(options).await
// migrate: sqlx::migrate!("./migrations").run(pool).await
```

- **sqlx 0.9** mit den Features `runtime-tokio`, `sqlite`, `migrate` und `sqlite-deserialize`. Dazu `libsqlite3-sys` mit `bundled`: SQLite ist eingebacken, ein CVE wird also nur durch einen Crate-Bump geschlossen.
- **`build.rs`** mit `println!("cargo:rerun-if-changed=migrations");` ist Pflicht. Sonst bettet `migrate!` neue Migrationen erst nach der nächsten Rust-Änderung ein.
- Migrationen heißen `migrations/NNNN_name.sql` und sind fortlaufend nummeriert, mit einem Test auf eindeutige Nummern.
- Schreibdisziplin `LH:src/tx.rs`: `write_retry!` nutzt `BEGIN IMMEDIATE` und wiederholt die ganze Transaktion bei BUSY. Für Suchlage ist das zunächst unnötig, siehe C.
- **Test-Pool:** `db::test_pool()` migriert einmal je Testprozess, serialisiert das Ergebnis und spielt es per `deserialize` in eine frische `:memory:`-DB ein (`max_connections(1)`). Das ist schnell und lohnt sich, sobald es mehr als ein Dutzend Migrationen gibt. Vorher reicht ein schlichtes `migrate` je Test.

### B.4 Live-Updates an den Browser

- **Lifeline nutzt SSE, kein WebSocket.** `LH:src/live/mod.rs` (`LiveHub`, je Einsatz ein `tokio::sync::broadcast`-Kanal), `LH:src/routes/live.rs`.
  - Der Server antwortet mit `Sse::new(stream).keep_alive(KeepAlive::default())`.
  - Ein Ringpuffer erlaubt das Nachliefern ab `Last-Event-ID`. Bei `RecvError::Lagged` geht das Ereignis `lagged` raus, worauf der Client voll abgleicht.
  - Die Lebensdauer des Stroms ist begrenzt (`live/strom.rs`), ebenso die Zahl der Ströme je Benutzer.
  - Der Strom endet beim Abmelden (`sitzung_ende`).
  - Ereignisnamen sind ein BE↔FE-Kontrakt (`wire_enum!`, Kontrakttest).
- Die Suchlage-Spec sieht **WebSocket** vor. Wichtig für die Entscheidung:
  - Der Datenfluss für Telemetrie ist **nur Server → Browser** (etwa 1 Hz), und „Live starten“ und „Linse wechseln“ sind Befehle, die per normalem `POST` gehen. SSE reicht also, läuft über reines HTTP (Traefik/Cloudflare ohne Upgrade-Sonderfall) und verbindet sich im Browser selbst neu.
  - **Empfehlung: SSE nach Lifeline-Muster**, aber vereinfacht. Ein `broadcast`-Kanal für den ganzen Lagezustand und beim Verbinden zuerst ein **Snapshot-Ereignis** (aktueller Zustand aller Geräte) statt Ringpuffer und `Last-Event-ID`. Bei `Lagged` wird ebenfalls ein Snapshot gesendet.
  - Die Spec sagt etwas anderes. Die Abweichung muss deshalb in der Spec oder einem ADR festgehalten werden. Für WebSocket bräuchte axum das Feature `ws`, und Neuverbindung und Heartbeat muss man selbst bauen.
- Cloudflare puffert SSE nicht, solange `Content-Type: text/event-stream` gesetzt und die Antwort **nicht komprimiert** ist. Lifeline komprimiert nur JSON (`nur_json_ab`). Dieses Prädikat muss man übernehmen, wenn man einen Kompressions-Layer einsetzt.

### B.5 rust-embed für die SPA

`LH:src/static_files.rs`:

```rust
#[derive(rust_embed::Embed)]
#[folder = "frontend/dist"]
struct Asset;
// Pfad leer → index.html; Datei vorhanden → ausliefern; Pfad mit Endung, aber fehlt → 404;
// sonst SPA-Fallback index.html. Cache: assets/* „public, max-age=31536000, immutable“, Rest „no-cache“.
```

- `frontend/dist/.gitkeep` muss eingecheckt sein. rust-embed braucht den Ordner zur Compile-Zeit, und der Vite-Build legt die Datei per Plugin wieder an (`gitkeepBewahren` in `LH:frontend/vite.config.ts`).
- Im Dev-Betrieb proxyt Vite `/api` auf das Backend (`server.proxy['/api'] = { target: backendUrl, changeOrigin: true }`). Das Frontend nutzt `fetch(…, { credentials: 'same-origin' })` (`LH:frontend/src/api/client.ts`).
- Für Suchlage lassen sich `static_files.rs` (ohne den libheif-Teil) und seine Tests **fast unverändert übernehmen**.

### B.6 OIDC (Pocket ID) und Sitzungen

- **Crate:** `openidconnect = { version = "4", default-features = false }`, der HTTP-Client ist selbst gebaut (`LH:src/auth/oidc/mod.rs`):
  - `reqwest` mit `redirect::Policy::none()` (SSRF-Schutz), Timeouts und `https_only` außer bei Loopback, angebunden über `impl AsyncHttpClient`.
  - **Grund für die abgeschalteten Default-Features:** Sie zögen einen zweiten reqwest/rustls-Stack herein, und rustls könnte den Default-CryptoProvider nicht mehr bestimmen. Dieselbe Falle droht in Suchlage mit `rumqttc`. Dort die TLS-Features prüfen (MQTT läuft laut Spec im VPN ohne TLS, also `rumqttc` ohne TLS-Feature).
- **Ablauf** (`LH:src/routes/auth.rs`, `oidc_start`/`oidc_callback`):
  1. `/api/auth/oidc/start` baut PKCE (`PkceCodeChallenge::new_random_sha256`), CSRF-State und Nonce und fordert die Scopes `openid` und `profile` an.
  2. State, Nonce und PKCE-Verifier liegen **im Speicher** (`auth/oidc/state.rs`, begrenzt). Ein **Binding-Cookie `oidc_state`** schützt gegen Session-Fixation.
  3. Der Callback prüft den Cookie gegen den Query-State, tauscht den Code (`exchange_code` + `set_pkce_verifier`) und prüft das ID-Token (`claims(&id_token_verifier(), &nonce)`).
  4. Die Provisionierung (`auth/oidc/provisioning.rs`) findet das Konto über `(issuer, subject)` oder legt es an. Die Spalten `benutzer.oidc_issuer`/`oidc_subject` haben einen UNIQUE-Index (`LH:migrations/0083_auth_provider.sql`).
  5. Zum Schluss wird eine Sitzung angelegt und per Redirect auf den Zielpfad zurückgeleitet.
  - Discovery geschieht lazy beim ersten Login und wird mit Rückfall zwischengespeichert. So startet der Server auch ohne erreichbaren IdP.
- **Sitzungen** (`LH:src/auth/session.rs`):
  - Das Token besteht aus 32 Byte Zufall (`OsRng`) als Hex. In der DB steht nur `sha256(token)`.
  - Die Sitzung lebt 7 Tage ohne Verlängerung, Cookie-`Max-Age` gleich der Lebensdauer.
  - Cookie `lifeline_sid` mit HttpOnly und `SameSite=Lax` (`session_cookie` in `LH:src/routes/auth.rs`). `Secure` wird gesetzt, wenn es per Konfiguration erzwungen ist oder der TLS-Proxy `X-Forwarded-Proto: https` mitschickt (`weitergeleitet_als_https`). Ein direkter http-Aufruf im LAN oder in Dev bekommt kein `Secure`.
  - Die Tabelle `session` hat die Spalten `token_hash`, `benutzer_id`, `expires_at`, `kennung` und `zuletzt_gesehen_at`. Abgelaufene Sitzungen räumt ein Purge-Lauf.
  - Der Extractor `CurrentUser` liest das Cookie, sucht die Sitzung und gibt sonst 401 zurück.
- **Großbild-Zugang:** Am nächsten kommt die **Gerätekopplung** in Lifeline (`LH:src/geraet/`, `routes::geraet::koppeln`, `session::anlegen_geraet`). Ein Gerät (z. B. „Lagemonitor“) erhält mit einem Kopplungscode eine eigene Gerätesitzung ohne Personenkonto. `CurrentUser` prüft sie gegen eine Routen-Positivliste `darf_route(ansicht, methode, matched_path)`, alles Übrige ist 403. Für Suchlage reicht eine schlanke Fassung:
  - Der Admin erzeugt einen widerrufbaren Zugangstoken, gespeichert als Hash.
  - Der Großbild-Browser löst ihn einmal ein und bekommt eine Sitzung mit der Rolle `anzeige`.
  - Diese Rolle darf nur `GET` auf den Lagezustand und den Live-Strom.

### B.7 Konfiguration: Env oder App-Einstellungen

Der Grundsatz des Nutzers lautet: **keine Env-Konfiguration, Settings und Tokens in der App erzeugen, Env nur zum Bootstrap.**

**Lifeline ist hier keine Vorlage.** `LH:src/config.rs` ist eine `clap`-Struktur mit `env = "LIFELINE_*"` für rund 40 Werte. Darunter sind ausdrücklich **die OIDC-Daten** (`LIFELINE_OIDC_ISSUER`, `…_CLIENT_ID`, `…_CLIENT_SECRET`, `…_REDIRECT_URL`) sowie WebAuthn, TLS, Backup und ClamAV. In der DB steht für OIDC nur der An/Aus-Schalter (`auth_provider(id, aktiviert)`) und die Kontobindung, keine Issuer- oder Client-Daten.

Zwei Muster aus Lifeline passen trotzdem:

1. **Env als Vorgabe, gespeicherter Wert gewinnt:** `karte_auto_aktualisierung`. Der Wert aus Env/CLI gilt, „solange in der Verwaltung nichts gespeichert ist“ (`LH:src/main.rs` um `auto_vorgabe`). Ebenso `org_einstellungen` und `auth_provider` als DB-Overrides.
2. **Ersteinrichtung:** `LH:src/auth/bootstrap.rs` → `bootstrap_admin`. Ist die Tabelle `benutzer` leer, wird ein Admin angelegt. Ohne vorgegebenes Passwort wird eines erzeugt und **einmal ins Log** geschrieben.

**Vorschlag für Suchlage** (löst das Henne-Ei-Problem, dass ein reiner Pocket-ID-Login ohne eingetragene Pocket-ID-Daten niemanden hereinlässt):

- Env und CLI tragen nur Bootstrap-Werte: `SUCHLAGE_DB_PATH` und `SUCHLAGE_BIND`, optional `SUCHLAGE_PUBLIC_URL`.
- Beim Start mit leerer Einstellungstabelle erzeugt der Server einen **Einrichtungstoken** (Zufall, Hash in der DB, nur einmal gültig, z. B. 24 h). Er gibt ihn einmal im Log aus, als Link `https://<host>/einrichtung#<token>`.
- Die Seite `/einrichtung` fragt Issuer, Client-ID und Client-Secret ab. Die Redirect-URL wird aus der öffentlichen URL abgeleitet und angezeigt, damit man sie in Pocket ID einträgt. Ein Knopf „Verbindung testen“ führt die Discovery aus. Danach folgt der erste Login per Pocket ID, und dieses Konto wird Admin. Der Token ist damit verbraucht.
- Alles Weitere wird in der App erzeugt und gepflegt: MQTT-Zugangsdaten je Fernsteuerung, Großbild-Tokens, die MediaMTX-Adresse und das DJI-App-Profil.
  - Ausnahme sind die DJI-App-ID, der App-Key und die Lizenz. Laut Spec sind sie ein Betriebs-Secret. Konsequent wäre auch hier: einmal im Admin-Bereich eintragen und verschlüsselt oder nur für den Admin lesbar in der DB ablegen.
- Der Client-Secret liegt in der DB. Den Pfad zur DB schützen (Volume-Rechte, Backup verschlüsselt). Ob eine Verschlüsselung at rest nötig ist, ist im Sicherheitskonzept zu entscheiden.

### B.8 Tests

- **Integrationstests** `LH:tests/*.rs` (≈175 Dateien) mit dem gemeinsamen Helfer `LH:tests/common/mod.rs`:
  - `setup()` erzeugt Router und Pool aus `db::test_pool()`, `test_state(&pool, &LiveHub::new())`.
  - Dazu `login_cookie(app, user, pw)`, `anfrage(…)`, `anfrage_json(…)`, `live_oeffnen(…)` und `recv_until_tag(…)` zum frameweisen Lesen eines offenen SSE-Bodys mit `http-body-util`.
- Muster: `app.oneshot(Request::builder().uri("/api/health").body(Body::empty()))`, dann `to_bytes` und `serde_json` (siehe `LH:tests/health.rs`).
- Dev-Dependencies: `tokio` mit `test-util` (`start_paused = true` für Fristen), `http-body-util` und `tower` (`ServiceExt::oneshot`).
- OIDC gegen ein echtes Pocket ID ist ein `#[ignore]`-Smoke mit Anleitung (`LH:tests/oidc_smoke.rs`). Für Suchlage wäre ein Mock-IdP im Test (Discovery, JWKS, Token) besser, mindestens für den Callback-Pfad.
- Frontend: Vitest mit jsdom und Testing Library, MSW für API-Mocks, Playwright für E2E.
- Qualitäts-Gate als ein Skript: `LH:scripts/check-all.sh`, von der CI unverändert aufgerufen.

### B.9 Logging und Graceful Shutdown

- **Logging** (`LH:src/main.rs`): `tracing_subscriber::fmt()` mit `EnvFilter`, Vorgabe `info`. `RUST_LOG` ist hier akzeptable Env-Nutzung, weil es Betrieb betrifft und nicht Fachkonfiguration. Geschrieben wird nicht blockierend über `tracing_appender::non_blocking` (`lossy`, 16k-Zeilen-Puffer). Pro Request gibt es einen Span mit Request-ID über `TraceLayer` und `MakeSpanMitRequestId`. Häufige Fehler werden zu Sammelzeilen zusammengefasst (`log_drossel.rs`).
- **Shutdown:** `shutdown_signal()` wartet auf `ctrl_c` und unter Unix auf `SIGTERM` und löst dann `axum_server::Handle::graceful_shutdown(Some(10 s))` aus. Hintergrund-Tasks bekommen das Stopp-Signal über `tokio::sync::watch`. Zum Schluss wird der Pool mit Timeout geschlossen. Lifeline nutzt `axum-server` wegen TLS. Suchlage läuft hinter Traefik ohne eigenes TLS, dort reicht `axum::serve(listener, app).with_graceful_shutdown(shutdown_signal())`.
- Für Suchlage wichtig: Der MQTT-Client und der Lage-Kern müssen am Stopp-Signal hängen. Der laufende Flug wird beim Herunterfahren als „unterbrochen“ gespeichert, damit er laut Spec nach einem Neustart fortgesetzt wird und nicht doppelt entsteht.

### B.10 Dockerfile, mise und Toolchain

- `LH:Dockerfile` **baut nicht aus der Quelle.** Es kopiert die Binaries `dist/linux-{amd64,arm64}/lifeline-hub` aus dem CI-Matrix-Job (`.github/workflows/artefakte.yml`) in `gcr.io/distroless/cc-debian13:nonroot`. Das Volume `/data` ist für den nonroot-Nutzer vorbereitet, über eine busybox-Stufe. Env: `LIFELINE_DB_PATH=/data/lifeline.db` und `LIFELINE_BIND=0.0.0.0:8080`.
- Suchlage hat zunächst keine solche CI. Deshalb dort ein **mehrstufiges Dockerfile**: `node` (pnpm build) → `rust` (cargo build --release, mit `frontend/dist` aus Stufe 1) → `distroless/cc-debian13:nonroot`. Bei glibc auf die Debian-Version achten. Lifeline brauchte debian13 wegen GLIBC 2.38+.
- **mise:** `LH:mise.toml` pinnt nur `node = "26.7.0"` und `pnpm = "11.10.0"` und trägt einige Aufräum-Tasks. **Rust kommt als `stable` über rustup**, es gibt **kein `rust-toolchain.toml`**. Für Suchlage empfiehlt sich, Rust zusätzlich zu pinnen (`rust-toolchain.toml` oder `rust = "<version>"` in mise), weil das Projekt seltener angefasst wird.
- Weitere Dateien: `bacon.toml` (Watch für cargo), `.devcontainer/` und `compose.yml` (nur der lokale Object-Store für Karten).

---

## C) Wiederverwendung für Suchlage

### C.1 Direkt übernehmen (kopieren, wenig anpassen)

| Was | Lifeline-Pfad | Anpassung |
|---|---|---|
| Tokens + antd-Theme | `frontend/src/theme/tokens.ts` | Fachpaletten entfernen (A.6) |
| CSS-Rollen + Guard | `frontend/src/theme/rollen.css`, `sprache.css`, `rollen.guard.test.ts` | gleich kürzen |
| Theme-Provider | `frontend/src/theme/ThemeModeProvider.tsx`, `dichte.ts`, `helligkeit.ts`, `lib/sichererSpeicher.ts`, `components/useViewport.ts` | Speicherschlüssel umbenennen |
| FOUC-Skript | `frontend/index.html` | Schlüssel umbenennen |
| Schriften | `frontend/src/theme/schriften.ts`, `frontend/src/assets/fonts/` | – |
| Bausteine | `frontend/src/components/instrument/` (Auswahl A.6 Nr. 8) | `statusFarben`-Import lösen |
| Icon-Generator | `frontend/src/icons/IconRahmen.tsx`, `scripts/icons/erzeuge-icons.mjs` | eigenes Register, Lizenz prüfen |
| terra-draw-Muster | `frontend/src/pages/lagekarte/zeichnen.ts`, `messZeichnung.ts` | für Sektoren und Messen |
| MapLibre-Setup | `…/lagekarte/maplibreWorker.ts`, Map-Optionen aus `Kartenflaeche.tsx` (Z. 610–640) | – |
| Dunkler Kartenstil | `…/lagekarte/basemapStil.ts` (`FARBEN.dark`, Layer-Aufbau Shortbread) | an die gewählte Kartenquelle anpassen |
| SPA-Einbettung | `src/static_files.rs` | libheif-Teil streichen |
| Fehlertyp | `src/error.rs` | Varianten auf die genutzten reduzieren |
| Extractors | `src/extract.rs` (`JsonBody`, `PfadParam`) | – |
| DB-Anschluss | `src/db.rs` (`connect`, `migrate`, `test_pool`), `build.rs` | Pragmas wie oben |
| Sitzungen | `src/auth/session.rs` (Token, Hash, Cookie, `CurrentUser`) | ohne Gerät, Offline-Queue und Passwortweg |
| OIDC | `src/auth/oidc/mod.rs`, `oidc/state.rs`, `oidc/provisioning.rs`, Handler in `src/routes/auth.rs` | Settings aus der DB statt `OnceLock`/Env |
| Ersteinrichtung | Idee aus `src/auth/bootstrap.rs` | als Einrichtungstoken (B.7) |
| SSE-Grundgerüst | `src/routes/live.rs` (`Sse`, `KeepAlive`, `Lagged`), `src/live/mod.rs` (broadcast) | Snapshot statt Replay, ein Kanal |
| Shutdown/Logging | `src/main.rs` (`shutdown_signal`, Tracing-Setup) | `axum::serve` statt `axum-server` |
| Testhelfer | `tests/common/mod.rs` (`setup`, `anfrage_json`, SSE frameweise lesen) | stark gekürzt |
| Statuscode-Konvention | `src/AGENTS.md` § Statuscode-Konvention | als Regel in Suchlages AGENTS.md |

### C.2 Bewusst nicht übernehmen

Das wäre für ein kleines Projekt Overengineering.

- **Shell-Code** (`AppLayout`, `Kopfleiste`, `IconRail`, `ModulPanel`, Sprungpalette): Er ist an das Einsatz- und Modulmodell gebunden. Neu bauen nach den Maßen in `shell.dc.html`.
- **Fachpaletten** (Sichtung, ETB, Warnstufe, Fachebenen), `statusFarben.ts`, Bildmarke „Lebenslinie“ und das PWA-Manifest.
- **Kartenbackend:** `karten-service`, `crates/karten-katalog`, Offline-Katalog, PMTiles-Download, Style-/Tile-Proxy, KRITIS-Import (`osmpbf`, `rayon`) und Fachebenen (NINA, DWD, Pegel …). Suchlage braucht anfangs eine Basemap und keine Kartenverwaltung. Ob das eine Online-Quelle ist oder eine einzelne mitgelieferte PMTiles-Datei für den FüKw (M4), ist eine eigene Entscheidung.
- **Lastschutz und Transfer:** `zulassung.rs` (Request-Gate), `verbindung.rs` (Semaphor-Acceptor, TCP-Keepalive), `transfer.rs` (Upload/Download-Fristen), `http_begrenzt.rs` und `log_drossel.rs`.
- **Sicherheitsausbau ohne Bedarf:** WebAuthn/Passkeys (inkl. `openssl vendored`), TOTP, Passwortlogin, Rate-Limits je Anmeldeweg, Admin- und Anmelde-Audit, ClamAV-Scan, `age`-verschlüsselte Backups samt Scheduler, Datenträgerprüfung, Aufbewahrung und Schwärzung. Später je nach Bedarf ergänzen, z. B. ein einfaches `VACUUM INTO`-Backup.
- **Tauri-Desktop-Hülle** (`src-tauri/`), eigenes TLS (`axum-server`, `rcgen`, mkcert), Trusted-Proxy-Liste: Traefik und Cloudflare übernehmen das.
- **`write_retry!`/`BEGIN IMMEDIATE`** (`tx.rs`): Bei einem Schreiber (Lage-Kern) und wenigen Nutzern genügt `busy_timeout`. Wenn Telemetrie mit 1 Hz und Nutzeraktionen kollidieren, die Spurpunkte gebündelt schreiben.
- **Ringpuffer und `Last-Event-ID`-Replay**, Stromgrenzen je Benutzer, modulgefilterte Ereignisse: Für Suchlage genügt ein Snapshot beim Verbinden.
- **Gerätekopplung mit Funktionsansichten und Routen-Positivliste** in voller Breite: Nur die Idee wird für den Großbild-Token übernommen.
- **pnpm-Audit-Overrides** (`frontend/pnpm-workspace.yaml`): Sie gehören zu Lifelines Abhängigkeitsbaum. Übernommen wird nur `minimumReleaseAgeExclude` für einsatzzeichen.
- **Die meisten Guard-Tests** (`*.guard.test.ts`, z. B. Gate 3 Trefffläche, Gate 5 keine Hex-Literale, `dichteQuelle`) und Kontrasttests: Übernommen wird nur `rollen.guard.test.ts`. Gate 5 („keine Hex-Literale außerhalb `theme/`“) lohnt sich als einfache ESLint-Regel oder als grep im Check-Skript.
- **semantic-release mit KI-Changelog**, OpenSpec-Prozess und die Zunder-Konventionen der Lifeline-AGENTS.md.

### C.3 Ermessensfragen (mit Empfehlung)

| Frage | Empfehlung | Grund |
|---|---|---|
| OpenAPI-Codegen (`utoipa` → `openapi-typescript`) | **ja, aber erst ab M1-Mitte** | Die Live-Ereignisse und Flug-DTOs sind ein BE↔FE-Kontrakt. Der Codegen verhindert stille Drift. Am Anfang reichen handgeschriebene Typen. |
| Dichte-Staffel mit drei Stufen | **kompakt + komfortabel, `handschuh` weglassen** | Suchlage wird am Arbeitsplatz, am Großbild und mobil genutzt, nicht mit Einsatzhandschuh. Die Staffel in `tokens.ts` bleibt erhalten, nur der Umschalter entfällt. |
| Hellmodus | **übernehmen** (ist in den Tokens enthalten) | Kostet nichts und hilft mobil bei Tageslicht |
| PWA (`vite-plugin-pwa`) | **später** | Erst beim Offline-Ziel (M4) sinnvoll |
| `@einsatzzeichen` 5.2 oder 6.x | **6.x, exakt gepinnt** | neues Projekt, wenig kopierter Zeichencode (A.7) |
| SSE oder WebSocket | **SSE + POST-Befehle** | B.4. Die Spec ist entsprechend anzupassen. |
