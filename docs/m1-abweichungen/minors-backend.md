# Abweichungen: Minor-Funde Backend aus dem Abschluss-Review M1

Abweichungen vom Plan `docs/superpowers/plans/2026-10-16-m1-live-lage.md`, entstanden beim Beheben
der Minor-Funde aus dem Abschluss-Review. Jeder Fund wurde zuerst am Code geprüft.

## Cargo.toml: `tokio-stream` und `bytes` entfernt

Beide Crates wurden nirgends benutzt, auch nicht in `tests/` oder `src/bin`. `axum::body::Bytes`
kommt über axum. `cargo check --all-targets` läuft mit und ohne Feature `dev` durch, und
`Cargo.lock` verliert die beiden Einträge.

## `scripts/check.sh`: Modulgrenzen strenger

- **situation:** Neben `use tokio;` und `use ::tokio` fällt jetzt auch ein voll qualifizierter
  Pfad wie `tokio::time::Instant` im Ausdruck auf.
- **dji → runtime/web und dji::wire außerhalb von `src/dji`:** Gruppenimporte wie
  `use crate::{runtime, …}` bzw. `use crate::dji::{topic, wire}` fallen jetzt auf. Für die Prüfung
  werden die Dateien eines Strangs zu einer Zeile zusammengefügt. So erkennt der Check auch die
  mehrzeiligen Gruppen, die rustfmt bei langen Importen erzeugt. Eine Gruppe reicht dabei bis zum
  nächsten `;`, also bis zum Ende der `use`-Anweisung.
- **contract (neu):** In `src/contract` darf `crate::` nur auf `crate::contract` zeigen.
- Die Muster verzichten auf `\b` und nutzen stattdessen `[^[:alnum:]_]`. Damit verhalten sich GNU
  grep 3.11 (Debian-Container), BSD grep 2.6 (`/usr/bin/grep`) und ugrep 7.8 (lokal im `PATH`) gleich.

Nachweis: Der heutige Code bleibt grün. Danach wurde je Muster ein absichtlicher Verstoß
eingebaut, mit allen drei grep geprüft und mit `git checkout` wieder entfernt. Jede Sonde schlug an:

| Sonde | Datei | Ausgabe |
|---|---|---|
| `let _ = tokio::time::Instant::now();` | `src/situation/mod.rs` | `FAIL src/situation importiert Laufzeit-Crates` |
| `use tokio;` | `src/situation/mod.rs` | `FAIL src/situation importiert Laufzeit-Crates` |
| `use crate::{runtime, store};` | `src/dji/mod.rs` | `FAIL src/dji hängt an runtime oder web (Zyklus)` |
| `use crate::{` + Zeilenumbruch + `store::sessions, web::live, };` | `src/dji/mod.rs` | `FAIL src/dji hängt an runtime oder web (Zyklus)` |
| `use crate::store::users::User;` | `src/contract/mod.rs` | `FAIL src/contract hängt an anderen Teilen des Crates` |
| `use crate::dji::{topic, wire};` | `src/web/mod.rs` | `FAIL dji::wire wird außerhalb von src/dji benutzt` |
| `use crate::dji::{` + Zeilenumbruch + `topic, wire::Envelope, };` | `src/web/mod.rs` | `FAIL dji::wire wird außerhalb von src/dji benutzt` |

Die Sonde `use crate::{runtime, store};` lief zusätzlich durch das echte Gate
`CI=1 scripts/check.sh --rust`. Es brach mit Exit 1 ab, noch vor `cargo fmt`.

Bekannte Grenzen: `super::super::…`-Pfade und Umbenennungen über `use … as` prüft das Gate nicht.
Ein Kommentar mit `crate::{`, auf den kein `;` folgt, kann einen Fehlalarm auslösen. Der heutige
Code enthält keinen solchen Kommentar.

## `is_valid_sn` liegt in `src/sn.rs`

Die SN-Prüfung ist von `dji/topic.rs` in das neue Modul `src/sn.rs` gezogen. `dji::topic`
re-exportiert sie (`pub use crate::sn::is_valid_sn`). Deshalb bleiben `dji/translate.rs` und
`sim/fake_pilot.rs` unverändert. `media/hook.rs`, `web/media.rs` und `web/pilot.rs` importieren
jetzt `crate::sn`. Die Baustein-Tabelle nennt `sn` nicht. Das Modul ist wie `time` oder `token`
ein neutrales Hilfsmodul ohne Abhängigkeiten im Crate.

## Zyklus app ↔ auth behoben

Die Extractors `Viewer`, `Operator` und `Admin` sind jetzt generisch über den Zustand `S`, mit
`SqlitePool: FromRef<S>`. `auth::dev::login` holt sich `State<SqlitePool>` und
`State<Arc<BootConfig>>`. `app.rs` stellt beide Teilzustände per `impl FromRef<AppState>` bereit,
von Hand geschrieben, weil axum hier ohne das Feature `macros` läuft. `src/auth` importiert
`crate::app` nicht mehr. Der Rest der Baustein-Tabelle für `auth` bleibt, wie er war: `config`,
`contract`, `error`, `time` und `token` nutzt `auth` weiter. Sie gelten als Querschnitt, und die
Tabelle nennt sie nicht.

## Flugmodi 12 und 16 deutsch

- `12 => "Landung mit drei Rotoren"`, wie im Kommentar in `frontend/src/telemetry/readouts.ts`.
- `16 => "Steuerung per App"` statt „Virtual Stick“.
- Kein Test prüfte die beiden Texte. `readouts.ts` blieb unverändert: Die Datei nennt Code 12 nur
  im Kommentar, schon mit dem neuen Wort, und Code 16 kommt dort nicht vor.

## Rohmitschnitt ohne Gerätegeheimnisse

- **Bereits behoben:** `$CONTROL/dynamic-security/#` übersprang `Recorder::write` schon vorher,
  denn er schreibt nur Topics unter `thing/` und `sys/`. Das prüft der Test
  `writes_m0_format_and_skips_broker_control`. Neuer Code war dafür nicht nötig.
- **Neu:** Auf `sys/…/status` entfernt `Recorder::write` die Felder `device_secret` und `nonce` aus
  `data` und aus jedem Eintrag in `data.sub_devices[]`. Erkannt wird das Topic an seiner Form
  (`sys/` am Anfang, `/status` am Ende), nicht über `topic::parse`. `parse` liefert bei einer
  ungültigen SN `Other`, und dann wären die Geheimnisse ungefiltert geschrieben worden.
- Neu serialisiert wird nur, wenn etwas entfernt wurde. Dabei sortiert `serde_json` die Schlüssel,
  und `payloadlen` wird aus dem neuen Payload berechnet. Ein Status ohne Geheimnisse bleibt
  bytegleich.
- **Entscheidung:** Ein Status-Payload, der kein gültiges JSON ist, fällt ganz aus dem Mitschnitt.
  Er wird nicht ungeprüft geschrieben, denn ob er Geheimnisse trägt, lässt sich nicht feststellen.
- Test: `status_loses_device_secrets_and_keeps_everything_else` in `src/dji/recorder.rs`.

## Pocket ID: jedes Konto wird Operator

Rollenlogik ist bewusst nicht gebaut. `docs/betrieb/m1-betrieb.md` schreibt jetzt in Schritt 7
vor: Bevor die Pocket-ID-Daten eingetragen werden, wird der Client auf eine I&K-Gruppe beschränkt
(„Allowed User Groups“). Dazu steht dort, dass eine bestehende Sitzung nach dem Entfernen aus der
Gruppe bis zu 7 Tage weiter gilt. Den Hinweis auf der Einrichtungsseite im Frontend ergänzt der
Frontend-Strang. Offen für später bleiben eine Prüfung des Gruppen-Claims in `oidc::finish` und
ein Endpunkt zum Sperren von Konten. **Erledigt:** Gruppen-Claim seit 27.10.2026 (Rolle aus den
Gruppen), Sperren seit 29.10.2026 (SUL-52, `PUT /api/accounts/{id}/block`).
