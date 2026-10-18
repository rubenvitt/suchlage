# Schnittstelle für die Flugmappe – Plan (SUL-66)

- **Stand:** 18.10.2026
- **Spec:** `docs/superpowers/specs/2026-10-18-einsaetze-und-flugmappe-design.md` §6, §7, §8
- **Baut auf:** SUL-65 (Einsätze, gemergt), SUL-78 (Einsatz-Bedienung, gemergt)
- **Gegenstück:** SUL-69 (Flugmappe, Repo `uav-checklists`) übernimmt die erzeugten Vertragstypen als Kopie mit Herkunftskopf.

## 0. Geprüft: Pocket-ID-Zugriffstokens (erster Plan-Schritt)

Quelle: `pocket-id/pocket-id` `main` @ `acba4e7` (16.10.2026, 2.18.0), Ausgabe über den Fork `pocket-id/fosite` v1.4.2.

- Zugriffstokens sind **JWTs nach RFC 9068**, signiert mit demselben Schlüssel wie die ID-Tokens (JWKS unter `/.well-known/jwks.json`, Standard RS256, ES256/EdDSA möglich). Header `typ: at+jwt`, `kid` gesetzt.
- Claims: `iss` (Basis-URL), `sub` (Benutzer-ID, dieselbe wie im ID-Token), `aud` **immer Array**: bei normaler Anmeldung `["<client_id>", "<issuer>"]`, `exp`, `iat`, `jti`, `scope`, `scp`, `client_id`. Kein `nbf`, keine Benutzer-Claims (Name, Gruppen).
- ID-Tokens haben ebenfalls `aud` = Client-ID, aber `typ: JWT`, kein `client_id` und `type: "id-token"`.
- Lebensdauer Standard 60 min (je Client einstellbar); die Flugmappe erneuert still (oidc-client-ts `automaticSilentRenew`).
- Das Signatur-Backend der Flugmappe (`uav-checklists/server/src/auth.ts`) prüft dieselben Tokens bereits per JWKS mit `iss`/`aud`/`exp`.

**Folge:** Kein Rückfall über Userinfo nötig. Suchlage prüft lokal:

1. Header: `alg` aus der Liste des JWKS-Schlüssels (kein `none`, kein HS*), `typ` ist `at+jwt` oder `application/at+jwt` (Groß/klein egal).
2. Signatur gegen den JWKS des **in den OIDC-Einstellungen** stehenden Ausstellers (Schlüssel über `kid`; unbekannter `kid` → JWKS einmal neu laden, höchstens alle 60 s). Geprüft wird in zwei Schritten: erst alles ohne Schlüssel (Form, Kopf, Claims aus 3.–5.), dann die Signatur – nur ein sonst gültiges Token löst einen Abruf aus. Jeder Abruf (auch ein gescheiterter, auch die Discovery) sperrt weitere für 60 s; scheitert er, gelten die alten Schlüssel weiter, 503 nur ohne je geladene Schlüssel. Nach Ablauf (1 h) kommt der JWKS direkt von `jwks_uri`.
3. `iss` == eingestellter Aussteller (ohne Schrägstrich am Ende verglichen).
4. `exp` in der Zukunft (60 s Spielraum), `iat` nicht mehr als 60 s in der Zukunft.
5. `aud` (String oder Array) enthält die **Client-ID der Flugmappe** aus den Einstellungen „Flugmappe“; zusätzlich `client_id`-Claim, falls vorhanden, gleich dieser Client-ID.
6. Konto über (`iss`, `sub`) in `users`. Kein Konto → 403. Konten werden hier nie angelegt.

Keine neue Krypto-Abhängigkeit: `openidconnect` bringt `CoreJsonWebKeySet` und `JsonWebKey::verify_signature` mit; der JWKS kommt über die vorhandene Discovery (`OidcState::metadata` → `jwks_uri`) und denselben HTTP-Client (keine Redirects, Zeitgrenzen).

## 1. Entscheidungen dieses Plans

| Thema | Entscheidung |
|---|---|
| Pfad | `/api/flugmappe/v1/…`, eigener Router `src/web/flugmappe.rs`, eigener Extractor `FlugmappeUser` (Bearer), keine Sitzungscookies |
| Aus | Einstellungen „Flugmappe“ fehlen → jeder Aufruf 503 (auch die Probe), CORS-Preflight ohne Erlaubnis |
| Rechte | Rolle `operator` oder `admin` (alle Konten in `users` haben eine davon). Kiosk kann kein Bearer-Token haben. Kontosperre (SUL-52) hängt sich später in `flugmappe_auth::account` ein – das ist die einzige Stelle, an der das Konto nachgeschlagen wird. |
| Bindung an eine Flugmappe | Einsatzbezogene Aufrufe tragen den Header `X-Flugmappe-Einsatz: <uuid>` (die Einsatz-ID der Flugmappe). Passt er nicht zur gespeicherten Verknüpfung → 409 „Der Einsatz ist mit einer anderen Flugmappe verknüpft“. So bekommt eine abgelöste Flugmappe nach „Übernehmen“ nichts mehr. |
| Adressierung | Einsätze über die Suchlage-ID (`/missions/{id}`), Abschnitte über die **Abschnitts-ID der Flugmappe** (`/missions/{id}/sections/{fm_section_id}`), weil die Flugmappe nur ihre eigenen kennt. |
| Idempotenz | Anlegen eines Abschnitts mit bekannter `flugmappe_section_id` liefert den vorhandenen (200). „Kartenelemente übernehmen“ geht einmal je Einsatz; ein zweiter Aufruf ist ein No-op (200, `imported: 0`). Abschließen eines abgeschlossenen Einsatzes, der mit dieser Flugmappe verknüpft ist → 200 (Nachholen ist dann schon erledigt). |
| Kartenfoto | Datei im Datenverzeichnis `kartenfotos/<section_id>.jpg` (nicht im Garage-Bucket: unabhängig vom Fotospeicher, klein). Nur JPEG (Magic Bytes `FF D8 FF`), höchstens 8 MiB, ersetzt ein vorhandenes Foto. Löschen eines Einsatzes löscht die Dateien mit. |
| Karte | GeoJSON `FeatureCollection`: alle Kartenelemente des Einsatzes (Kartenelemente haben keinen Abschnitt) und die Flugspuren der Flüge – mit `?section=<fm_section_id>` nur die Flüge dieses Abschnitts. Properties: `kind` (`map_element`/`flight_track`), Name, Farbe, Flug-ID. |
| Kartenelemente übernehmen | Polygone, Linien und Punkte (Kreise kommen aus der Flugmappe bereits als Polygone) werden als `origin = web` mit `author = <Name des Kontos> (Flugmappe)` im Einsatz angelegt; Farbe aus `MAP_COLORS` (Fläche blau, Standort rot). Der Standort setzt zusätzlich den Ort des aktiven Abschnitts, falls leer. Höchstens 200 Features, Punktzahlen wie in `map::rules`. |
| Abschnitte in Suchlage | Bei verknüpften Einsätzen lehnt die Browser-API `POST /api/missions/{id}/sections` mit 422 ab (Spec §4); die Oberfläche bietet es dort nicht an. |
| Live | Jede Änderung über die Schnittstelle ruft `MissionHub::changed()` → Browser laden neu. |

## 2. Datenmodell (Migration `0005_flugmappe.sql`)

```sql
ALTER TABLE missions ADD COLUMN origin TEXT NOT NULL DEFAULT 'web' CHECK (origin IN ('web', 'flugmappe'));
ALTER TABLE missions ADD COLUMN flugmappe_mission_id TEXT;          -- UUID der Flugmappe
ALTER TABLE missions ADD COLUMN flugmappe_linked_by TEXT;           -- Anzeigename des Kontos
ALTER TABLE missions ADD COLUMN flugmappe_linked_at_ms INTEGER;
ALTER TABLE missions ADD COLUMN flugmappe_map_imported_at_ms INTEGER;
CREATE UNIQUE INDEX missions_by_flugmappe ON missions (flugmappe_mission_id) WHERE flugmappe_mission_id IS NOT NULL;
ALTER TABLE mission_sections ADD COLUMN flugmappe_section_id TEXT;
ALTER TABLE mission_sections ADD COLUMN map_photo_at_ms INTEGER;    -- gesetzt, wenn ein Kartenfoto liegt
CREATE UNIQUE INDEX mission_sections_by_flugmappe ON mission_sections (mission_id, flugmappe_section_id) WHERE flugmappe_section_id IS NOT NULL;
```

Bestandsdaten: `origin = 'web'`, keine Verknüpfung.

## 3. Vertrag (`src/contract/flugmappe.rs`, dazu Erweiterungen in `missions.rs`)

`Mission` bekommt `origin: MissionOrigin` (`web`/`flugmappe`) und `flugmappe: Option<FlugmappeLink>` (`linked_by`, `linked_at_ms`; die UUID bleibt im Browser-Vertrag weg, sie braucht dort niemand). `MissionSection` bekommt `has_map_photo: bool` (und `map_photo_at_ms`).

Schnittstelle (alle Felder `snake_case`, Zeiten ms seit Epoch):

| Methode, Pfad | Eingang | Ausgang | Fehler |
|---|---|---|---|
| `GET /me` | – | `FmMe { name, role, api_version: 1 }` | 401, 403, 503 |
| `GET /missions` | – | `Vec<FmMission>` (vorbereitet + laufend) | |
| `POST /missions` | `FmMissionCreate { name, location, section_label, section_location, flugmappe_mission_id, flugmappe_section_id }` | `FmMission` (laufend) | 400, 409 (anderer läuft / UUID schon verknüpft) |
| `POST /missions/{id}/link` | `FmMissionLink { flugmappe_mission_id, flugmappe_section_id, section_label, section_location, take_over: bool }` | `FmMission` (laufend) | 409 (anderer läuft; mit anderer Flugmappe verknüpft und `take_over = false`), 422 (abgeschlossen) |
| `POST /missions/{id}/sections` | `FmSectionCreate { flugmappe_section_id, label, location }` | `FmMission` | 409 (Bindung), 422 (abgeschlossen) |
| `GET /missions/{id}/flights` | – | `Vec<FmFlight>` | 409 (Bindung) |
| `GET /missions/{id}/map?section=` | – | GeoJSON (`serde_json::Value`, im Vertrag als `FmFeatureCollection` typisiert) | 409 |
| `POST /missions/{id}/map/import` | `FmMapImport { features: Vec<FmImportFeature>, standort: Option<GeoPoint> }` | `FmMapImportResult { imported }` | 400, 409, 422 |
| `PUT /missions/{id}/sections/{fm_section_id}/photo` | `image/jpeg` (Body bis 8 MiB) | 204 | 400, 404, 409, 413, 422 |
| `POST /missions/{id}/close` | – | `FmMission` | 409, 422 (Flug läuft) |

`FmMission { id, name, location, state, started_at_ms, created_at_ms, created_by, sections: Vec<FmSection>, link: Option<FmLinkInfo { flugmappe_mission_id, linked_by, linked_at_ms }> }` – die Flugmappe braucht die UUID, um „mit mir verknüpft“ von „mit einer anderen“ zu unterscheiden.
`FmSection { id, flugmappe_section_id, label, location, created_at_ms, has_map_photo }`.
`FmFlight { id, flugmappe_section_id, started_at_ms, ended_at_ms, end_reason, aircraft_sn, aircraft_label, max_elevation_m, distance_m, photo_count }` (`aircraft_label` aus den Bezeichnungen der Fernsteuerungen, sonst `null`).

Header `X-Flugmappe-Einsatz` ist Pflicht bei allen `/missions/{id}/…`; bei `POST /missions` und `/link` steht die UUID im Body.

## 4. CORS

Nur für `/api/flugmappe/v1/*`: Ursprung genau gleich dem eingestellten (`https://uav.iuk-ue.de`), Methoden `GET, POST, PUT, OPTIONS`, Header `Authorization, Content-Type, X-Flugmappe-Einsatz`, keine Credentials, `Vary: Origin`, `max-age` 600. Die Einstellung kann sich zur Laufzeit ändern: eigene kleine Middleware, die den aktuellen Wert aus `SettingsHandle` liest (oder `tower_http::cors` mit Prädikat; `tower-http` braucht dafür das Feature `cors`). Preflight braucht kein Token. Das Body-Limit des Routers (64 KiB) gilt nicht für das Kartenfoto: eigenes `DefaultBodyLimit` auf dieser Route.

## 5. Einstellungen „Flugmappe“

`settings::FlugmappeSettings { client_id, allowed_origin }`, Schlüssel `flugmappe`; `SettingsView` zeigt beides (kein Geheimnis). Eingabe validiert: Client-ID nicht leer, Ursprung `https://host[:port]` ohne Pfad (http nur für Loopback, wie `issuer_allowed`). Leeres Formular entfernt die Einstellung (Schnittstelle aus). Oberfläche: neuer Block in `frontend/src/admin/SettingsForms.tsx` mit Erklärung („Client-ID der Flugmappe aus Pocket ID“, „Adresse der Flugmappe“).

## 6. Oberfläche Suchlage

- Einsatzliste/Kopfleiste: Hinweis „aus der Flugmappe“ bzw. „mit Flugmappe verknüpft (Name, Zeit)“.
- Abschnitt anlegen bei verknüpften Einsätzen ausgeblendet (Server: 422).
- Kartenfoto am Abschnitt: Vorschaubild mit Öffnen in voller Größe, `GET /api/missions/{id}/sections/{section_id}/map-photo` (Sitzung, jede Rolle mit Lesezugriff).

## 7. Aufgaben

1. **Migration + Store:** `0005_flugmappe.sql`; `store::missions` liest/schreibt die neuen Spalten (`link`, `find_by_flugmappe`, `add_section_from_flugmappe`, `set_map_photo`, `mark_map_imported`). Tests: Migration mit Bestandsdaten, Eindeutigkeit.
2. **Vertrag:** `src/contract/flugmappe.rs` + Erweiterungen in `missions.rs`; Beispiele in `examples.rs`/Fixtures; `cargo test contract` erzeugt TS.
3. **Bearer-Prüfung:** `src/auth/bearer.rs` – JWKS-Cache je Aussteller in `OidcState`, reine Prüffunktion `verify_access_token(jwt, jwks, expected) -> Result<Claims, Reason>` mit Tests (gültig, abgelaufen, falsches `iss`, falsche `aud`, `typ: JWT`, ID-Token mit `type: id-token`, `alg: none`, unbekannter `kid`, kaputte Signatur). Test-IdP (`oidc::test_idp`) bekommt `access_token_for(sub, aud, ttl)` (RS256, `typ: at+jwt`).
4. **Einstellungen:** `FlugmappeSettings` + Admin-API (`PUT /api/admin/settings/flugmappe`, `DELETE` zum Ausschalten) + Formular.
5. **MissionHub:** `create_from_flugmappe`, `link`, `add_flugmappe_section`, `import_map`, `close_from_flugmappe`, `set_map_photo`; Regeln (Bindung, Übernehmen, 409/422) als reine Funktionen in `missions::rules` mit Tests. Alles unter der Schreibsperre.
6. **Router `/api/flugmappe/v1`:** Extractor, CORS, Handler, GeoJSON-Aufbau als reine Funktion (`missions::geojson` oder `map::geojson`) mit Test.
7. **Kartenfoto:** Speichern (atomar: temp + rename), Ausliefern für Browser, Löschen in `deletion.rs`.
8. **Browser-API:** 422 für Abschnitt anlegen bei verknüpften Einsätzen; Oberfläche (Hinweise, Kartenfoto, Einstellungen).
9. **Integrations-Tests** (`src/web/flugmappe.rs`): ohne Token 401, kaputtes Token 401, Token ohne Konto 403, Schnittstelle aus 503, Test-IdP-Token legt Einsatz an und liest Flüge, zweiter laufender Einsatz 409, fremde Bindung 409, Übernehmen, abgeschlossen 422, CORS-Preflight erlaubt/abgelehnt, Kartenfoto (falscher Typ 400, zu groß 413).
10. **Doku:** README/Betrieb: Pocket-ID-Client der Flugmappe muss für Suchlage nichts Neues können; in Suchlage Einstellungen „Flugmappe“ setzen. Version anheben (minor).

## 8. Abnahme

- `scripts/check.sh` grün.
- Ohne Token 401, Konto unbekannt 403, zweiter laufender Einsatz 409.
- Ein Test-IdP-Token legt einen Einsatz an und liest seine Flüge.
