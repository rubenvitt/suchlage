# Schnittstelle zu Lifeline Hub – Mini-PRD

- **Stand:** 24.11.2026, aus der Machbarkeitsprüfung LLH-1202 in Lifeline Hub
- **Status:** Entwurf zur Durchsicht, kein Code
- **Aufgabe:** SUL-49 (`https://zunder.rubeen.dev/#/aufgabe/vn345vmghvs6iizdpl8kmcc8`)
- **Gegenstück:** `rubenvitt/lifeline-hub`, `docs/konzepte/2026-11-24-drohnenbild-ueber-suchlage.md`

## 1. Zweck

Lifeline Hub soll Position, Livebild und Fotos der Drohne in seiner Lagekarte und auf seinem
Lagemonitor zeigen. Heute hängen Drohnenbild und Einsatzführung an getrennten Bildschirmen. SUL-49
fragte, was Lifeline Hub von Suchlage braucht. Die Antwort fällt klein aus, denn fast alles gibt
es schon.

| Bedarf von Lifeline Hub | Heute in Suchlage | Neu |
|---|---|---|
| Position, Heading, Akku, Spur, Suchgebiete | `GET /api/lagemonitor/v1/lage` (Bearer) | – |
| Livebild | `/api/lagemonitor/v1/video/{sn}/hls/…` und `/whep` (Bearer) | – |
| Fotos mit Aufnahmeposition | nur mit Sitzung (`/api/photos`, `/photos/{id}/thumb`) | **Foto-Endpunkt per Bearer** |

Lifeline Hub ruft wie der Lagemonitor der IUK-Suite **von Server zu Server** ab. Der Browser
sieht Suchlage nie, also braucht es kein CORS.

## 2. Entscheidungen

| Thema | Entscheidung | Grund |
|---|---|---|
| Anmeldung | Großbild-Token als `Authorization: Bearer`, wie beim Lagemonitor | vorhanden, widerrufbar, wirkt sofort |
| Umfang des Tokens | neuer Schalter `photos` am Großbild-Zugang, Standard aus | Fotos zeigen Personen. Ein reiner Anzeige-Zugang (FüKw-Monitor) bekommt sie nicht ungefragt dazu; Vorbild ist `einsatztagebuch` (IDA-359). |
| Einsatzbezug | immer der laufende Einsatz, keine fremde Einsatz-ID | Lifeline Hub schaltet „Drohnenbild aus Suchlage“ im eigenen Einsatz zu; Suchlage hat höchstens einen laufenden Einsatz |
| Was herausgeht | nur Vorschauen (640 px, neu kodiert, ohne EXIF), nie Originale | Lifeline Hub speichert keine Kopie; Originale bleiben dem `Operator` vorbehalten |
| Aufbewahrung | bleibt in Suchlage | Lifeline Hub hält nichts fest |

## 3. Endpunkte (neu)

Unter `/api/lagemonitor/v1`, mit demselben Extraktor `LagemonitorClient`, der zusätzlich den
Schalter `photos` verlangt (sonst 403):

- `GET /photos?since_ms=` gibt die Auslösungen des **laufenden** Einsatzes zurück, ohne laufenden
  Einsatz eine leere Liste. Jede Auslösung ist ein `LmPhotoShot` mit `id`, `taken_at_ms`, `position`
  (nur Auslösungen mit Position), `relative_altitude_m`, `gimbal_yaw_deg` und `photos[]` (`lens`
  und relative `thumb_url`). Die Seriennummern von Fluggerät und Fernsteuerung fehlen, weil
  Lifeline Hub sie nicht braucht. Antworten sind gzip-komprimiert, wie bei `lage`.
- `GET /photos/{id}/thumb` liefert die Vorschau-JPEG mit `Cache-Control: private,
  max-age=…`. Sie liefert nur, wenn das Foto zum laufenden Einsatz gehört, sonst 404.
- `LmLage` bekommt `photos_latest_ms`, die Zeit der jüngsten Auslösung. So muss Lifeline Hub die
  Liste nur abrufen, wenn sich etwas geändert hat. `LAGEMONITOR_API_VERSION` bleibt 1, weil das
  Feld nur hinzukommt.

Vertrag wie üblich: Typen in `src/contract/lagemonitor.rs`, `cargo test contract` erzeugt
`frontend/src/contract/`.

## 4. Einstellungen

- Migration: `ALTER TABLE kiosk_tokens ADD COLUMN photos INTEGER NOT NULL DEFAULT 0;`
- Unter *Einstellungen → Großbild-Zugänge* ein Häkchen „Fotos für Lifeline Hub“ beim Anlegen.
  Ein Zugang mit Häkchen ist in der Liste als solcher erkennbar.
- Runbook (`docs/betrieb/runbook-deployment.md`): Zugang für Lifeline Hub anlegen, Token in
  Lifeline Hub unter *Anbindungen → Suchlage* eintragen.

## 5. Nicht-Ziele

- Kein CORS und kein Zugriff aus dem Browser von Lifeline Hub.
- Keine Verknüpfung mit Einsatz-IDs von Lifeline Hub, kein Schreiben aus Lifeline Hub.
- Keine Originale und keine Videoaufzeichnungen nach außen.
- Keine Änderung an Video oder `lage`, abgesehen von `photos_latest_ms`.

## 6. Folgeaufgabe Datenschutz

Suchlage kennt bisher keine Löschfristen (Nicht-Ziel im Design vom 18.10.2026). Mit der
Weitergabe an Lifeline Hub und mit der Personensuche werden Aufnahmen von Personen häufiger. Fristen
für Fotos und Aufzeichnungen gehören daher als eigene Aufgabe auf das Board. Sie hängen nicht an
dieser Schnittstelle, sollten ihr aber bald folgen.

## 7. Tests

- Ein Token ohne `photos` bekommt 403 auf `/photos` und `/photos/{id}/thumb`, `lage` bleibt 200.
- Ein widerrufener Token bekommt sofort 401.
- Ohne laufenden Einsatz liefert `/photos` eine leere Liste. Ein Foto eines anderen Einsatzes ergibt
  404 auf `thumb`.
- Die Vorschau trägt kein EXIF: Ein Test mit einem synthetischen JPEG samt GPS-EXIF prüft das.

## 8. Abnahme

Nach einem Übungsflug mit Fotos erscheinen die Bildpunkte in der Lagekarte von Lifeline Hub an der
richtigen Stelle, und die Vorschau öffnet sich. Nach dem Widerruf des Zugangs verschwinden sie beim
nächsten Abruf.
