# DJI Cloud API – Referenz für Matrice 350 RTK + DJI RC Plus + DJI Pilot 2

Entwickler-Referenz für **Pilot-to-Cloud** (kein Dock). Feldnamen, Methoden und Enum-Werte stehen im Original.

- **Quelle:** offizielle DJI-Cloud-API-Doku, Stand **v1.11.3** (Release 2024-11-07, Git-Commit `4ec6b0c`). Alle Pfade beziehen sich auf `docs/en/` im geklonten Repo.
- **Mindest-Firmware laut Release Notes** (`00.index.md`, Abschnitt v1.11.3): DJI Pilot 2 `10.1.0.30`, Matrice 350 RTK `10.01.00.09`, DJI RC Plus `05.02.11.06`.
- **Kennzeichnung:** **„Doku-Widerspruch“** markiert Stellen, an denen sich die Doku selbst widerspricht. **„nicht dokumentiert“** markiert Stellen, an denen die Doku schweigt. Mit **„Interpretation“** oder **„Erwartung“** sind eigene Ableitungen gekennzeichnet, die im Feldtest zu prüfen sind.
- Seriennummern, Tokens und Keys sind Platzhalter (`<AIRCRAFT_SN>`, `<RC_SN>` …).

---

## 0. Welcher Doku-Ordner gilt für M350 + RC Plus?

*Quelle: `10.overview/30.product-support.md`, `60.api-reference/10.pilot-to-cloud/00.mqtt/30.others/`, `30.feature-set/10.pilot-feature-set/30.pilot-livestream.md`*

| Gerät | `domain` | `type` | `sub_type` | `device_model_key` (domain-type-subtype) |
|---|---|---|---|---|
| Matrice 350 RTK (Fluggerät, Sub-Device) | `0` | **89** | **0** | `0-89-0` |
| DJI RC Plus (Gateway) | `3` (laut Beispiel) | **119** | **0** | `3-119-0` (siehe Hinweis) |

- **Ordner für M350 + RC Plus:** `60.api-reference/10.pilot-to-cloud/00.mqtt/30.others/`
  - `10.aircraft/00.properties.md` enthält die Eigenschaften des Fluggeräts.
  - `20.rc/00.properties.md` enthält die Eigenschaften der Fernsteuerung, `20.rc/10.device.md` die Topologie und `20.rc/20.live.md` den Livestream.
  - Belege für diese Zuordnung:
    - Die Produkttabelle führt „M350 RTK + DJI RC Plus + DJI Pilot 2“.
    - Das `update_topo`-Beispiel in `20.rc/10.device.md` nutzt den Gateway-`type: 119` (RC Plus).
    - Die Feature-Doku verlinkt `.../mqtt/rc-plus/live.html`, und das entspricht dem Ordner `30.others/20.rc`.
- **Andere Ordner gelten nicht:**
  - `10.m3-series` ist für Mavic 3E/3T gedacht.
  - `20.rc-pro` ist für DJI RC Pro Enterprise gedacht (type 144).
  - Die Fluggeräte-Datei unter `30.others` ist inhaltlich gleich der von `m3-series`, hat aber **zusätzlich** das Payload-Struct `{type-subtype-gimbalindex}` (siehe Kapitel 3.9).
- **Was für M350 + RC Plus fehlt:**
  - Im Ordner `30.others` sind **keine `events`- und keine `requests`-Methoden** dokumentiert.
  - Es gibt dort auch **keine DRC- und keine Remote-Control-Dateien**. DRC und Remote Control existieren nur unter `20.rc-pro` (M3E). Für M350 + RC Plus gibt es also nur `update_topo`, Properties (osd/state) und Livestream-Services.
- **`domain`:**
  - **nicht dokumentiert** als Enum für RCs. Die einzigen gefundenen Enums (`device_model_domain`, `module`) lauten `{"0":"Aircraft","3":"Dock"}`.
  - Das RC-Plus-Beispiel sendet `"domain": "3"` für die RC als **String**.
  - Empfehlung: Geräte über `type`/`sub_type` (119/0 bzw. 89/0) erkennen, nicht über `domain`.
- **Kamera-Indizes für M350** (`type-subtype-gimbalindex`):

  | Kamera | Index |
  |---|---|
  | FPV | `39-0-7` |
  | Zenmuse H20 | `42-0-{g}` |
  | Zenmuse H20T | `43-0-{g}` |
  | Zenmuse H20N | `61-0-{g}` |
  | Zenmuse H30 | `82-0-{g}` |
  | Zenmuse H30T | `83-0-{g}` |

  Für `g` gilt: `0` = Portside (links), `1` = Starboard (rechts), `2` = Upside (oben).
- **Payloads:** Laut Doku werden nur H20/H20T/H20N/H30/H30T unterstützt. Drittanbieter-Payloads werden nicht unterstützt.

---

## 1. Topics, Richtung und Nachrichtenhülle

*Quelle: `60.api-reference/10.pilot-to-cloud/00.mqtt/00.topic-definition.md`, `10.overview/40.basic-concept/20.mqtt.md`, `10.overview/40.basic-concept/30.thing-model.md`*

Bei Pilot-to-Cloud ist das **Gateway die RC Plus** (`{gateway_sn}` = `<RC_SN>`). `{device_sn}` ist das Gerät, zu dem das Thing Model gehört. Das ist entweder das Fluggerät (`<AIRCRAFT_SN>`) oder die RC selbst. Die Verbindung läuft über MQTT 5.0.

| Topic | Richtung | Zweck |
|---|---|---|
| `thing/product/{device_sn}/osd` | Gerät → Cloud | Properties mit `pushMode = 0`, periodisch, **dokumentiert 0,5 Hz** (alle 2 s) |
| `thing/product/{device_sn}/state` | Gerät → Cloud | Properties mit `pushMode = 1`, nur bei Änderung |
| `sys/product/{gateway_sn}/status` | Gerät → Cloud | Online/Offline/Topologie (`update_topo`) |
| `sys/product/{gateway_sn}/status_reply` | Cloud → Gerät | Antwort auf `update_topo` |
| `thing/product/{gateway_sn}/services` | Cloud → Gerät | Kommandos (z. B. `live_start_push`) |
| `thing/product/{gateway_sn}/services_reply` | Gerät → Cloud | Ergebnis der Kommandos |
| `thing/product/{gateway_sn}/events` | Gerät → Cloud | Ereignisse (für M350/RC Plus **keine** dokumentiert) |
| `thing/product/{gateway_sn}/events_reply` | Cloud → Gerät | Quittung, wenn `need_reply = 1` |
| `thing/product/{gateway_sn}/requests` | Gerät → Cloud | Anfragen des Geräts (für M350/RC Plus **keine** dokumentiert) |
| `thing/product/{gateway_sn}/requests_reply` | Cloud → Gerät | Antwort darauf |
| `thing/product/{gateway_sn}/property/set` (+ `_reply`) | Cloud → Gerät (zurück) | Properties mit accessMode `rw` setzen |

Server-seitig abonnieren: `thing/product/+/osd`, `thing/product/+/state`, `sys/product/+/status`, `thing/product/+/services_reply` und, falls künftig nötig, auch `events` und `requests`.

### Nachrichtenhülle

```json
{
  "tid": "<uuid>",          // Transaction-UUID: ordnet Request und Reply zu
  "bid": "<uuid>",          // Business-UUID: klammert mehrere Transaktionen eines Vorgangs
  "timestamp": 1654070968655, // ms seit Epoch, Sendezeitpunkt (13-stellig)
  "method": "live_start_push", // nur bei services/events/requests/status (+ *_reply)
  "need_reply": 0,          // nur bei events: 1 = Cloud muss auf events_reply antworten
  "gateway": "<RC_SN>",     // SN des Gateways (bei osd/state/services/events/requests)
  "data": { }               // Nutzlast
}
```

- In `*_reply`-Nachrichten trägt `data` mindestens das Feld `result` (int, `0` = OK, ungleich 0 = Fehler). Bei `services_reply` und `requests_reply` kommt optional `output` (struct) dazu.
- `tid` und `bid` in der Antwort von der Anfrage übernehmen.
- `status`/`update_topo` hat in den Beispielen **kein** Feld `gateway`. Die RC-SN kommt dort aus dem Topic-Pfad.
- `property/set_reply`: Das Ergebnis steht pro Property-Key: `data.<property>.result` mit `0` = Erfolg, `1` = Fehler, `2` = Timeout.
- **Wichtig: osd und state sind Teil-Updates.** Die osd-Beispiele in `00.topic-definition.md` zeigen drei aufeinanderfolgende Nachrichten desselben Geräts mit jeweils **anderen** Feldern (die Beispiele stammen vom Dock; für Pilot ist das eine **Interpretation**). Jede osd- oder state-Nachricht als **Patch** in den letzten bekannten Gerätezustand mergen und den Zustand nie komplett ersetzen.
- **pushMode bestimmt das Topic:** Laut `30.thing-model.md` gilt `pushMode 0` → osd (0,5 Hz), `pushMode 1` → state (bei Änderung) und `pushMode 2` → nicht gepusht. Kapitel 3 nennt den pushMode je Feld.

---

## 2. `update_topo` – Topologie, Seriennummern, Antwort

*Quelle: `60.api-reference/10.pilot-to-cloud/00.mqtt/30.others/20.rc/10.device.md`, `.../00.topic-definition.md` (status struct)*

**Topic:** `sys/product/<RC_SN>/status`, Richtung Gerät → Cloud, Methode `update_topo`.

> **Doku-Widerspruch:** Die Kopfzeile von `20.rc/10.device.md` nennt `thing/product/{gateway_sn}/status`. `00.topic-definition.md` und die Reply in derselben Datei nennen dagegen `sys/product/...`. Maßgeblich ist **`sys/product/{gateway_sn}/status`**, und dieses Topic abonnieren.

| Feld | Typ | Bedeutung |
|---|---|---|
| `domain` | text | Gateway-Domain, im Beispiel `"3"` |
| `type` / `sub_type` | int | Gateway-Produkttyp, für RC Plus `119` / `0` |
| `device_secret`, `nonce` | text | Gateway-Schlüssel und Nonce (Verwendung **nicht dokumentiert**) |
| `thing_version` | text | Thing-Model-Version des Gateways, z. B. `"1.1.2"` |
| `sub_devices[]` | array (size 1) | verbundenes Fluggerät, **leer, wenn kein Fluggerät verbunden ist** |
| `»sn` | text | **SN des Fluggeräts** |
| `»domain` | text | `"0"` (Fluggerät) |
| `»type` / `»sub_type` | int | für M350 `89` / `0` |
| `»index` | text | z. B. `"A"`. Bedeutung **nicht dokumentiert**, vermutlich der Steuerungskanal A/B (vgl. `control_source`) |
| `»device_secret`, `»nonce`, `»thing_version` | text | wie beim Gateway |

> **Doku-Widerspruch:** Das generische Beispiel in `00.topic-definition.md` nutzt `"version": 1` (int) und `type: 98`/`116`, die zu keinem Produkt passen. Die RC-Plus-Datei nutzt `"thing_version": "1.1.2"` (String). Maßgeblich ist die RC-Plus-Datei. Ihr Beispiel zeigt als Sub-Device `type: 60` (M300). Ein **M350 meldet `type: 89`**.

**So kommt man an die Seriennummern:**
- **RC-SN** steht im Topic-Pfad: `sys/product/{RC_SN}/status`. In anderen Nachrichten steht sie zusätzlich im Hüllenfeld `gateway`.
- **Fluggeräte-SN** steht in `data.sub_devices[0].sn`.
- Danach gilt:
  - osd/state des Fluggeräts kommen auf `thing/product/<AIRCRAFT_SN>/osd|state`.
  - osd/state der RC kommen auf `thing/product/<RC_SN>/osd|state`.
  - Services gehen immer an `thing/product/<RC_SN>/services`.

**Beispiel: Fluggerät online**
```json
{
  "tid": "<uuid>", "bid": "<uuid>", "method": "update_topo", "timestamp": 1234567890123,
  "data": {
    "domain": "3", "type": 119, "sub_type": 0,
    "device_secret": "<secret>", "nonce": "<nonce>", "thing_version": "1.1.2",
    "sub_devices": [
      { "sn": "<AIRCRAFT_SN>", "domain": "0", "type": 89, "sub_type": 0, "index": "A",
        "device_secret": "<secret>", "nonce": "<nonce>", "thing_version": "1.1.2" }
    ]
  }
}
```
**Fluggerät offline:** gleiche Nachricht mit `"sub_devices": []`.

**Erwartete Antwort** auf `sys/product/<RC_SN>/status_reply`, mit `tid` und `bid` aus der Anfrage:
```json
{ "tid": "<uuid aus Anfrage>", "bid": "<uuid aus Anfrage>", "timestamp": 1654070968655,
  "method": "update_topo", "data": { "result": 0 } }
```

---

## 3. OSD und State des Fluggeräts (M350)

*Quelle: `60.api-reference/10.pilot-to-cloud/00.mqtt/30.others/10.aircraft/00.properties.md`*

Topic: `thing/product/<AIRCRAFT_SN>/osd` für pushMode 0 bzw. `.../state` für pushMode 1. Fast alle Felder sind read-only (`r`). Wo die Doku keine Einheit nennt, steht in der Tabelle „—“.

### 3.1 Position und Höhen

| Feld | Typ | Einheit | pushMode | Bedeutung |
|---|---|---|---|---|
| `latitude` | float | Grad (WGS84) | 0 | aktuelle Breite |
| `longitude` | float | Grad (WGS84) | 0 | aktuelle Länge |
| `height` | float | — (de facto m) | 0 | **„Absolute height“: ellipsoidische Höhe** (über dem WGS84-Ellipsoid), berechnet als Höhe relativ zum Startpunkt + Ellipsoidhöhe des Startpunkts |
| `elevation` | float | — (de facto m) | 0 | **„Relative takeoff point altitude“: Höhe relativ zum Startpunkt** |
| `home_distance` | float | — (de facto m) | 0 | Distanz zum Home-Point |

- **`height` ist nicht MSL:**
  - Für die Höhe über NN muss eine Geoid-Korrektur abgezogen werden. In Deutschland liegt die Geoid-Undulation grob bei 45–50 m; dieser Wert ist eigenes Wissen und steht nicht in der Doku.
  - `elevation` ist **keine Höhe über Grund (AGL)**, sondern bezieht sich auf den Startpunkt. In hügeligem Gelände weicht sie von AGL ab.
- Die Einheiten von `height`, `elevation` und `home_distance` sind **nicht dokumentiert**. Meter ist naheliegend, aber im Feldtest zu prüfen.
- Bei `latitude`/`longitude` stehen unter „constraint“ Werte wie `max 3.4028235E38` und `min -1.4E-45`. Das sind float-Typgrenzen und **keine** Gradbereiche.
- Kartenelemente (Kapitel 6) sind ausdrücklich WGS84. Für die Fluggeräte-OSD steht das Bezugssystem nicht ausdrücklich in der Doku; WGS84 ist anzunehmen.

### 3.2 Geschwindigkeit, Kurs und Lage

| Feld | Typ | Einheit | pushMode | Bedeutung |
|---|---|---|---|---|
| `horizontal_speed` | float | — (de facto m/s, **nicht dokumentiert**) | 0 | Horizontalgeschwindigkeit |
| `vertical_speed` | float | m/s | 0 | Vertikalgeschwindigkeit (Vorzeichen **nicht dokumentiert**) |
| `attitude_head` | **int** | Grad | 0 | Gierwinkel zu geografisch Nord. Positiv von 0 bis „6 Uhr“ (über Ost), negativ von 6 bis 12 Uhr (über West), also −180…180. Kompasskurs = `(attitude_head + 360) % 360` |
| `attitude_pitch` | float | — (Grad) | 0 | Nickwinkel |
| `attitude_roll` | float | — (Grad) | 0 | Rollwinkel |
| `wind_speed` | float | — | 0 | geschätzte Windgeschwindigkeit, aus der Fluglage berechnet und nur zur Orientierung |
| `wind_direction` | enum_int | — | 0 | 1 N, 2 NO, 3 O, 4 SO, 5 S, 6 SW, 7 W, 8 NW |

### 3.3 Akku

| Feld | Typ | Einheit | pushMode |
|---|---|---|---|
| `battery.capacity_percent` | int 0–100 | % | 0 |
| `battery.remain_flight_time` | int | s | 0 |
| `battery.return_home_power` | int 0–100 | % für Rückflug nötig | 0 |
| `battery.landing_power` | int 0–100 | % für Zwangslandung | 0 |
| `battery.batteries[]` | array | — | 0 |
| `»capacity_percent` | int | % | |
| `»index` | int | — | |
| `»sn` | text | — | |
| `»type`, `»sub_type` | enum_int | — (leere Enums) | |
| `»firmware_version` | text | — | |
| `»loop_times` | int | Ladezyklen | |
| `»voltage` | int | mV | |
| `»temperature` | float | °C, eine Nachkommastelle | |
| `»high_voltage_storage_days` | int | Tage | |
| `low_battery_warning_threshold` | int | % (vom Nutzer gesetzt) | **1 (state)** |
| `serious_low_battery_warning_threshold` | int | % (vom Nutzer gesetzt) | **1 (state)** |

**Erwartung:** Ein M350 hat zwei Akkus (TB65), daher sollte `batteries[]` **zwei Einträge** enthalten. Das ist **nicht dokumentiert**.

### 3.4 `mode_code` – Flugzustand (osd, pushMode 0)

| Wert | Originaltext | Deutsch | Phase (**Interpretation**) |
|---|---|---|---|
| 0 | Standby | Bereitschaft | Boden |
| 1 | Takeoff preparation | Startvorbereitung | Boden |
| 2 | Takeoff preparation completed | Startvorbereitung abgeschlossen | Boden |
| 3 | Manual flight | manueller Flug | Luft |
| 4 | Automatic takeoff | automatischer Start | Luft (Übergang) |
| 5 | Wayline flight | Wegpunktflug | Luft |
| 6 | Panoramic photography | Panoramaaufnahme | Luft |
| 7 | Intelligent tracking | intelligente Verfolgung | Luft |
| 8 | ADS-B avoidance | ADS-B-Ausweichen | Luft |
| 9 | Auto returning to home | automatische Rückkehr (RTH) | Luft |
| 10 | Automatic landing | automatische Landung | Luft (Übergang) |
| 11 | Forced landing | Zwangslandung | Luft (Übergang) |
| 12 | Three-blade landing | „Three-blade landing“ | Luft (Übergang) |
| 13 | Upgrading | Firmware-Update | Boden (vermutlich) |
| 14 | Not connected | nicht verbunden | **unbekannt** |
| 15 | APAS | Hinderniserkennung APAS | Luft |
| 16 | Virtual stick state | Virtual Stick | Luft |
| 17 | Live flight Controls | Live-Flugsteuerung (DRC) | Luft |
| 18 | Airborne RTK fixing mode | RTK-Fixing in der Luft | Luft |

### 3.5 `mode_code_reason` – Grund für den aktuellen Zustand (**state**, pushMode 1)

| Wert | Bedeutung |
|---|---|
| 0 | keine Bedeutung |
| 1 | Akkuladung zu niedrig (RTH, Landung) |
| 2 | Akkuspannung zu niedrig (RTH, Landung) |
| 3 | Spannung kritisch niedrig (RTH, Landung) |
| 4 | per RC-Taste angefordert (Start, RTH, Landung) |
| 5 | per App angefordert (Start, RTH, Landung) |
| 6 | **RC-Signalverlust** (RTH, Landung, Schweben) |
| 7 | durch externe Geräte wie Navigation oder SDK ausgelöst (Start, RTH, Landung) |
| 8 | Dock-GEO-Zone betreten (Landung) |
| 9 | RTH ausgelöst, aber zu nah am Home-Point (Landung) |
| 10 | RTH ausgelöst, aber zu weit vom Home-Point entfernt (Landung) |
| 11 | bei Ausführung einer Wegpunktmission angefordert (Start) |
| 12 | im RTH über dem Home-Point angekommen (Landung) |
| 13 | weiterer Sinkflug nach Erreichen von 0,7 m über Grund (zweite Sinkflugstufe) (Landung) |
| 14 | Niedrighöhenschutz durch App oder SDK erzwungen durchbrochen (Landung) |
| 15 | wegen Flugverkehr in der Nähe angefordert (RTH, Landung) |
| 16 | Höhenregelung ausgefallen (RTH, Landung) |
| 17 | nach intelligentem Low-Battery-RTH (Landung) |
| 18 | AP steuert den Flugmodus (manueller Flug) |
| 19 | Hardware-Fehler (RTH, Landung) |
| 20 | Ende des Kollisionsschutzes (Landung) |
| 21 | RTH abgebrochen (Schweben) |
| 22 | Hindernis während RTH (Landung) |

### 3.6 Home-Point

| Feld | Typ | pushMode | Hinweis |
|---|---|---|---|
| `home_latitude` | float | **1 → state** | kommt **nicht** im osd |
| `home_longitude` | float | **1 → state** | kommt **nicht** im osd |
| `home_distance` | float | 0 → osd | Distanz zum Home-Point |

### 3.7 GNSS und RTK (`position_state`, osd)

| Feld | Typ | Werte |
|---|---|---|
| `position_state.is_fixed` | enum_int | 0 nicht gestartet, 1 Fixing läuft, 2 Fix erfolgreich, 3 Fix fehlgeschlagen |
| `position_state.quality` | enum_int | 1–5 („Gear 1“–„Gear 5“, Satelliten-Empfangsstufe) |
| `position_state.gps_number` | int | Anzahl GPS-Satelliten |
| `position_state.rtk_number` | int | Anzahl RTK-Satelliten |

### 3.8 Signalqualität

Das **Fluggerät hat kein Feld für Signalqualität**. Die Verbindungsqualität steht in der **RC-OSD** unter `wireless_link` (Kapitel 3.11).

### 3.9 Payload/Gimbal (osd), Struct mit dem Key `{type-subtype-gimbalindex}`, z. B. `"42-0-0"`

Für die Suche besonders wertvoll ist der **Laser-Entfernungsmesser** des Zielpunkts (H20/H30-Serie):

| Feld | Typ | Einheit |
|---|---|---|
| `gimbal_pitch`, `gimbal_roll`, `gimbal_yaw` | double −180…180 | ° |
| `measure_target_latitude` | double −90…90 | ° |
| `measure_target_longitude` | double −180…180 | ° |
| `measure_target_altitude` | double | m (Bezug **nicht dokumentiert**) |
| `measure_target_distance` | double | m |
| `measure_target_error_state` | enum_int | 0 NORMAL, 1 TOO_CLOSE, 2 TOO_FAR, 3 NO_SIGNAL |
| `payload_index` | text | wie der Key (pushMode 1) |
| `zoom_factor` | double | — |

Weitere Felder: Thermal-Felder `thermal_current_palette_style` (rw), `thermal_gain_mode` (rw), `thermal_isotherm_*` (rw) sowie `thermal_global_temperature_min` und `thermal_global_temperature_max` (°C).

Daneben gibt es das Array `cameras[]` mit `payload_index`, `camera_mode` (0 Foto, 1 Video, 2 Smart Low-Light, 3 Panorama), `recording_state`, `zoom_factor`, `ir_zoom_factor`, `liveview_world_region`, `ir_metering_*` u. a.

### 3.10 Weitere nützliche Felder

| Feld | Typ | pushMode | Bedeutung |
|---|---|---|---|
| `gear` | enum_int | 0 | 0 A, 1 P, 2 NAV, 3 FPV, 4 FARM, 5 S, 6 F, 7 M, 8 G, 9 T |
| `control_source` | text | 1 | aktuelle Steuerquelle: `A`/`B` (RC) oder die UUID eines Browsers |
| `track_id` | text (64) | 0 | Track-ID, z. B. als Flug-ID nutzbar (Semantik **nicht dokumentiert**) |
| `height_limit` | int 20–1500 m | 0 | Höhenbegrenzung (rw) |
| `is_near_height_limit`, `is_near_area_limit` | enum_int | 0 | Nähe zur Höhengrenze oder GEO-Zone |
| `obstacle_avoidance.{horizon,upside,downside}` | enum_int | 0 | Hinderniserkennung (rw) |
| `night_lights_state` | enum_int | 0 | Nachtlichter (rw) |
| `total_flight_time` (s), `total_flight_distance` (m), `total_flight_sorties` | | 0 | Lebenszeitzähler |
| `storage.total`, `storage.used` | int | 0 | KB |
| `firmware_version` | text | 1 | Firmware |
| `country` | text | 0 | Ländercode |

### 3.11 RC-Plus-OSD und State (`thing/product/<RC_SN>/osd|state`)

*Quelle: `60.api-reference/10.pilot-to-cloud/00.mqtt/30.others/20.rc/00.properties.md`*

| Feld | Typ | Einheit | pushMode | Bedeutung |
|---|---|---|---|---|
| `latitude` | double −90…90 | ° | 0 | Position der RC (Pilotenstandort) |
| `longitude` | double −180…180 | ° | 0 | Position der RC |
| `height` | double | m | 0 | ellipsoidische Höhe der RC |
| `capacity_percent` | int 0–100 | % | 0 | Akku der RC |
| `wireless_link.sdr_quality` | int 0–5 | — | 0 | **Signalqualität der Bildübertragung (SDR)** |
| `wireless_link.sdr_link_state` | enum_int | — | 0 | 0 getrennt, 1 verbunden |
| `wireless_link.sdr_freq_band` | float | — (GHz laut Beispiel 2.4) | 0 | Frequenzband |
| `wireless_link.4g_link_state` | enum_int | — | 0 | 0 getrennt, 1 verbunden |
| `wireless_link.4g_quality` | int 0–5 | — | 0 | 4G-Qualität gesamt |
| `wireless_link.4g_uav_quality` | int 0–5 | — | 0 | 4G-Qualität Fluggerät ↔ 4G-Server |
| `wireless_link.4g_gnd_quality` | int 0–5 | — | 0 | 4G-Qualität RC ↔ 4G-Server |
| `wireless_link.link_workmode` | enum_int | — | 0 | 0 SDR, 1 4G-Fusion (laut Doku „Dock's …“) |
| `wireless_link.dongle_number` | int | — | 0 | Anzahl Dongles am Fluggerät |
| `live_capacity` | struct | — | **1 → state** | Livestream-Fähigkeiten (Kapitel 5) |
| `live_status[]` | array | — | **1** | laufende Streams (Kapitel 5) |
| `dongle_infos[]` | array | — | 1 | 4G-Dongle/eSIM |
| `firmware_version` | text | — | 1 | Firmware |

---

## 4. Start und Landung aus `mode_code` ableiten; Verbindungsverlust

*Quelle: `.../30.others/10.aircraft/00.properties.md` (mode_code, mode_code_reason), `.../30.others/20.rc/10.device.md` (update_topo offline), `30.feature-set/10.pilot-feature-set/10.pilot-access-to-cloud.md` (Abschnitt „Pilot disconnect“), `10.overview/40.basic-concept/10.proper-noun.md` (Online-Status)*

Die Doku ordnet die `mode_code`-Werte **nicht ausdrücklich** in „Boden“ und „Luft“ ein. Die folgende Einteilung ist eine **Interpretation**:

- **Am Boden:** `0`, `1`, `2`, dazu `13` (Update) als Sonderfall.
- **In der Luft:** `3`–`12` und `15`–`18`.
- **Unklar:** `14` „Not connected“. Die RC ist dann online, das Fluggerät aber nicht verbunden. Das ist kein Flugzustand, also den letzten bekannten Zustand halten und als „unbekannt“ markieren.

**Empfohlene Ableitung:**
1. **Start** ist der Übergang von der Boden-Menge `{0,1,2}` in die Luft-Menge. Der erste Luft-Code ist meist `3` (manuell) oder `4` (Auto-Takeoff).
2. **Landung** ist der Übergang von der Luft-Menge zurück nach `{0,1,2}`, typischerweise nach `10`/`11`/`12` oder direkt aus `3`.
3. **Entprellen:** Die Doku nennt 0,5 Hz, also eine Nachricht alle 2 s. Kurze Phasen wie `4` oder `10` können darum fehlen, und einzelne Ausreißer sind möglich.
   - Einen Wechsel erst nach **2 aufeinanderfolgenden** Nachrichten im neuen Zustand übernehmen.
   - Zusätzlich gegen `elevation` (z. B. > 1–2 m bedeutet Luft) und `vertical_speed` plausibilisieren.
4. Den **Grund** zeigt `mode_code_reason` (state). Für die Einsatzlage wichtig sind:
   - `6` = RC-Signalverlust,
   - `1`/`2`/`3`/`17` = Akku,
   - `21` = RTH abgebrochen.

   Achtung: `mode_code_reason = 6` sieht die Cloud erst, **wenn der Link wieder steht**. Während des Signalverlusts kommen ja keine Daten.

**Was bei Verbindungsverlust gesendet wird:**

| Situation | Was ankommt | Quelle |
|---|---|---|
| Fluggerät aus oder Funkverbindung RC ↔ Fluggerät weg | `update_topo` auf `sys/product/<RC_SN>/status` mit **`sub_devices: []`** | `20.rc/10.device.md` |
| RC/Pilot 2 verliert die Cloud-Verbindung (Netz weg, App beendet) | **nicht dokumentiert** als DJI-Nachricht | `10.pilot-access-to-cloud.md` beschreibt „Active/Passive offline“ nur als Bild |

- Den Flug auch bei `sub_devices: []` **schließen** bzw. als „Verbindung verloren“ markieren. Ein letztes `mode_code = 0` kommt dann möglicherweise nicht mehr.
- Ein Cloud-Verbindungsverlust der RC ist nur indirekt erkennbar:
  - über das Disconnect-Ereignis des Brokers (Client-Session),
  - über eine eigene MQTT-Last-Will-Nachricht, falls der Broker das anbietet; DJI dokumentiert dazu nichts,
  - über einen **osd-Timeout**: Bei 0,5 Hz bedeuten etwa 3 verpasste Nachrichten (> 6 s) „veraltet“, und deutlich mehr bedeuten „offline“.
- `10.proper-noun.md` unterscheidet „disconnected“ (ohne Abmeldung, Netzproblem) von „offline“ (bewusst, z. B. Herunterfahren). Diese Unterscheidung in der UI übernehmen.

---

## 5. Livestream

*Quelle: `60.api-reference/10.pilot-to-cloud/00.mqtt/30.others/20.rc/00.properties.md` (live_capacity, live_status), `.../30.others/20.rc/20.live.md`, `30.feature-set/10.pilot-feature-set/30.pilot-livestream.md`, `60.api-reference/10.pilot-to-cloud/30.jsbridge.md` (liveshare), `71.error-code.md` (513xxx)*

### 5.1 `live_capacity` (RC-State, pushMode 1)

Kommt auf `thing/product/<RC_SN>/state` und **nur bei Änderung**. Den Wert serverseitig persistieren.

```
live_capacity
├─ available_video_number      int   wählbare Streams gesamt
├─ coexist_video_number_max    int   max. gleichzeitige Streams gesamt
└─ device_list[]                      Videoquellen auf Geräteebene
   ├─ sn                       text  SN der Quelle (Fluggerät)
   ├─ available_video_number   int
   ├─ coexist_video_number_max int
   └─ camera_list[]
      ├─ camera_index          text  "{type-subtype-gimbalindex}", z. B. "42-0-0", "39-0-7"
      ├─ available_video_number   int
      ├─ coexist_video_number_max int
      └─ video_list[]
         ├─ video_index        text  z. B. "normal-0"
         ├─ video_type         text  z. B. "normal"
         └─ switchable_video_types[] text  z. B. ["wide","zoom","thermal"]
```

**`video_id` bildet sich so:** `{sn}/{camera_index}/{video_index}`, z. B. `<AIRCRAFT_SN>/42-0-0/normal-0`.
- `sn` = `device_list[].sn`
- `camera_index` = `camera_list[].camera_index`
- `video_index` = `video_list[].video_index`

Alle drei Teile **aus `live_capacity` übernehmen** und nicht selbst bauen.

### 5.2 Services (alle auf `thing/product/<RC_SN>/services`, Antwort auf `.../services_reply`)

| Methode | `data`-Parameter | Antwort `data` |
|---|---|---|
| `live_start_push` | `url_type` (enum_int), `url` (text), `video_id` (text), `video_quality` (enum_int) | `{ "result": 0 }` |
| `live_stop_push` | `video_id` | `{ "result": 0 }` |
| `live_set_quality` | `video_id`, `video_quality` | `{ "result": 0 }` |
| `live_lens_change` | `video_id`, `video_type` (enum_string) | `{ "result": 0 }` (Tabelle sagt „Data: null“, das Beispiel zeigt `result`) |

**`url_type`** (MQTT): `0` Agora, `1` RTMP, `3` GB28181.

**`url`-Format je Typ:**
- RTMP: `rtmp://<host>:<port>/<app>/<stream>`
- GB28181: `serverIP=…&serverPort=…&serverID=…&agentID=…&agentPassword=…&localPort=…&channel=…`
- Agora: `channel=…&sn=…&token=…&uid=…`

**`video_quality`:**

| Wert | Stufe | Auflösung | Bitrate |
|---|---|---|---|
| 0 | Adaptive | — | — |
| 1 | Smooth | 960×540 | 512 kbps |
| 2 | Standard definition | 1280×720 | 1 Mbps |
| 3 | High definition | 1280×720 | 1,5 Mbps |
| 4 | Ultra-high definition | 1920×1080 | 3 Mbps |

**`video_type`** (`live_lens_change`): `normal` (Standard), `thermal` (Infrarot), `wide` (Weitwinkel), `zoom`.

Beispiel `live_start_push` (RTMP):
```json
{ "tid": "<uuid>", "bid": "<uuid>", "timestamp": 1654070968655, "method": "live_start_push",
  "data": { "url_type": 1, "url": "rtmp://<host>:1935/live/<stream>",
            "video_id": "<AIRCRAFT_SN>/42-0-0/normal-0", "video_quality": 0 } }
```

### 5.3 `live_status[]` (RC)

| Feld | Typ | Bedeutung |
|---|---|---|
| `video_id` | text | Stream-ID |
| `video_type` | text (24) | aktuelles Objektiv |
| `video_quality` | enum_int | 0–4 wie oben |
| `status` | enum_int | 0 kein Stream, 1 Stream läuft |
| `error_status` | int (6 Stellen) | Fehlercode, vgl. 513xxx |

### 5.4 Bekannte Eigenheiten

- **Kein RTSP und kein WebRTC über MQTT:** `url_type` kennt nur 0/1/3. Die Feature-Doku nennt zwar RTSP, aber nur der JSBridge-Weg unterstützt es.
- **Doku-Widerspruch – zwei Enums für den Protokolltyp:**
  - MQTT `url_type`: 0 Agora, 1 RTMP, 3 GB28181.
  - JSBridge `liveshareGetConfig`/`liveshareSetConfig` `type`: 0 Unknown, **1 Agora, 2 RTMP, 3 RTSP, 4 GB28181**.
  - Beide Enums nie vermischen.
- **Doku-Widerspruch – `live_status`:** Die Property-Tabelle führt es mit pushMode 1 (state). Das Sequenzdiagramm sagt „osd topic that contains the live_status field“. **Beide Topics** auswerten.
- **Uneinheitliche Objektivnamen:** `live_lens_change` nutzt `thermal`, die Kamera-Storage-Settings nutzen `ir`.
- **Agora-Token** einmal URL-encoden, sonst scheitert das Parsing in Pilot an Zeichen wie `+`.
- **Vorbedingungen in Pilot 2:**
  - Vorher muss per JSBridge das Live-Modul geladen sein: `window.djiBridge.platformLoadComponent("liveshare", JSON.stringify({videoPublishType: "video-on-demand" | "video-demand-aux-manual", statusCallback: "<jsFn>"}))`. Am besten geschieht das schon beim Login.
  - `video-by-manual` erlaubt **kein** Starten durch den Server.
  - Pilot prüft vor dem Start u. a., **ob die Kamera dekodiert** und **ob die Flugansicht offen ist**. Steht der Pilot im Menü, kommt `result ≠ 0`.
- **Manueller Modus** (`liveshareSetConfig`): Hier wird immer das Hauptbild von Pilot 2 gestreamt. Es folgt dem Kamerawechsel in der App.
- **Fehlercodes** (`71.error-code.md`):

  | Code | Bedeutung |
  |---|---|
  | 513002 | Kamera existiert nicht oder Kameratyp falsch |
  | 513003 | Stream läuft bereits |
  | 513005 | Qualitätseinstellung falsch |
  | 513006 | Start fehlgeschlagen |
  | 513008 | Bildübertragungsdaten fehlerhaft |
  | 513010 | Gerät ohne Netz |
  | 513011 | Livestream nicht aktiviert |
  | 513012 | Objektivwechsel während des Streams nicht unterstützt |
  | 513013 | Protokoll nicht unterstützt |
  | 513014 | Parameter falsch oder unvollständig |
  | 513015 | Netz-Lag |
  | 513016 | Dekodierung fehlgeschlagen |
  | 513017 | pausiert während des Datei-Downloads |
  | 513099 | allgemeiner Fehler |

---

## 6. Überblick: Meilenstein M2 (Kartenelemente) und M3 (Medien-Upload)

### 6.1 M2 – Kartenelemente über HTTPS und WebSocket

*Quelle: `30.feature-set/10.pilot-feature-set/40.pilot-map-elements.md`, `60.api-reference/10.pilot-to-cloud/10.https/10.map-elements/{10.create,20.update,30.obtain,40.delete}.md`, `60.api-reference/10.pilot-to-cloud/20.websocket/10.map-elements/10.message-push.md`, `10.overview/40.basic-concept/{40.https,50.websocket}.md`, `60.api-reference/10.pilot-to-cloud/30.jsbridge.md`*

**JSBridge in der Pilot-Webview:**
1. `platformSetWorkspaceId(uuid)` setzen.
2. Die Module `api` (`{host, token}`) und `ws` (`{host: wss://…, token, connectCallback}`) laden.
3. Danach das Modul `map` laden. Erst dann synchronisiert Pilot 2.

**HTTPS-Endpunkte** (Header `X-Auth-Token`, Antwort-Hülle `{code, message, data}`):

| Methode | Pfad |
|---|---|
| GET | `/map/api/v1/workspaces/{workspace_id}/element-groups?group_id=&is_distributed=` |
| POST | `/map/api/v1/workspaces/{workspace_id}/element-groups/{group_id}/elements` → `{id}` |
| PUT | `/map/api/v1/workspaces/{workspace_id}/elements/{id}` |
| DELETE | `/map/api/v1/workspaces/{workspace_id}/elements/{id}` |

**Elementgruppen:**
- Felder: `id`, `name`, `type`, `is_lock`, `create_time`, `elements[]`.
- `type`: `0` benutzerdefiniert, `1` Default, `2` App-Shared.
- **Pflicht:** Die Liste muss eine Gruppe mit `type: 2` (App-Shared) enthalten, sonst kann Pilot neue Elemente nicht hochladen.

**Elemente:**
- Felder: `id`, `name`, `create_time`, `update_time`, `resource{type, user_name, content}`.
- `resource.type`: `0` Pin, `1` Linie, `2` Polygon.
- `content` ist ein GeoJSON-Feature (`geometry.type` = `Point`/`LineString`/`Polygon`, `coordinates` als `[lon, lat, alt]`) mit `properties.color` und `properties.clampToGround`. Koordinaten sind **WGS84**.

**WebSocket-Pushes** an Pilot, Hülle `{biz_code, version, timestamp, data}`:
- `map_element_create`, `map_element_update`, `map_element_delete`,
- `map_group_refresh` → Pilot lädt die Gruppen per GET neu.
  *Feldbefund:* Das Neuladen ergänzt und ändert nur; was in der Antwort fehlt, behält Pilot 2. Wer Elemente aus der Sicht nimmt, schickt vorher `map_element_delete` je Element.

**Situational Awareness** (optional, Modul `tsa`):
- `GET /manage/api/v1/workspaces/{workspace_id}/devices/topologies`
- WS-Pushes `device_osd`, `device_online`, `device_offline`, `device_update_topo` (`20.websocket/20.situation-awareness/10.message-push.md`).

> **Doku-Fehler:**
> - **Farbtabelle:** ORANGE = `0xB620E0` (lila) und PURPLE = `0x212121` (fast schwarz) sind offensichtlich vertauscht oder falsch. Das Beispiel nutzt `#0091FF`, eine Farbe, die nicht in der Liste steht.
> - **`deviceOsd`-Beispiel:** `latitude` und `longitude` sind vertauscht (`latitude: 113.44…`), und `height` ist als integer deklariert.
> - **POST/PUT:** `40.https.md` behauptet, die Parameter stünden „im Header“. Gemeint ist der JSON-Body.

### 6.2 M3 – Medien-Upload und Speicher-Credentials

*Quelle: `30.feature-set/10.pilot-feature-set/50.pilot-media-management.md`, `60.api-reference/10.pilot-to-cloud/10.https/30.media-management/{10.fast-upload,20.obtain-exited-tiny-fingerprint,30.obtain-temporary-credential,40.mediafile-upload-result-report,50.group-upload-callback}.md`*

**JSBridge:** Workspace-ID setzen, Modul `api` laden, dann Modul `media` laden (Auto-Upload oder manueller Upload).

**Ablauf je Datei:**
1. `POST /media/api/v1/workspaces/{workspace_id}/files/tiny-fingerprints`: Der Server meldet, welche Tiny-Fingerprints schon vorhanden sind.
2. `POST /media/api/v1/workspaces/{workspace_id}/fast-upload`: Existiert die Datei schon, meldet der Server sofort Erfolg.
3. `POST /storage/api/v1/workspaces/{workspace_id}/sts` liefert temporäre Upload-Credentials:
   ```json
   { "code": 0, "message": "success", "data": {
       "bucket": "<bucket>", "endpoint": "https://<s3-endpoint>", "region": "<region>",
       "provider": "minio",            // ali | aws | minio
       "object_key_prefix": "<prefix>",
       "credentials": { "access_key_id": "<…>", "access_key_secret": "<…>",
                        "expire": 3600, "security_token": "<…>" } } }
   ```
   Pilot 2 lädt danach **direkt in den Objektspeicher** (OSS/S3/MinIO) hoch.
4. `POST /media/api/v1/workspaces/{workspace_id}/upload-callback` meldet das Ergebnis. Die Felder:
   - `result` (0 = OK), `name`, `object_key`, `path`, `fingerprint`, `sub_file_type` (0 normal, 1 Panorama),
   - `ext{file_group_id, drone_model_key, payload_model_key, is_original, sn, tinny_fingerprint}`,
   - `metadata{absolute_altitude, relative_altitude, gimbal_yaw_degree, created_time, shoot_position{lat,lng}}`.

   Die Antwort ist `{code, message, data:{object_key}}`.
5. Optional: `POST /media/api/v1/workspaces/{workspace_id}/group-upload-callback` für Dateigruppen.

> **Doku-Fehler:** Das Feld heißt wirklich `tinny_fingerprint` (Tippfehler im Original, so übernehmen). `shoot_position.lat`/`lng` sind als **integer** typisiert; in der Praxis ist double zu erwarten (Feldtest).

**Ergänzungen aus dem Original und der DJI-Demo (17.10.2026):**

- **Modul `media`:** Parameter `{autoUploadPhoto: boolean, autoUploadPhotoType: 0 (Original) | 1 (Vorschaubild), autoUploadVideo: boolean}`, alle optional. Vorher Workspace-ID und Modul `thing` setzen. JS-Schnittstellen: `mediaSetAutoUploadPhoto(bool)`, `mediaGetAutoUploadPhoto()`, `mediaSetUploadPhotoType(int)`, `mediaGetUploadPhotoType()`, `mediaSetAutoUploadVideo(bool)`, `mediaGetAutoUploadVideo()`, `mediaSetDownloadOwner(0 eigene RC | 1 andere)`, `mediaGetDownloadOwner()`.
- **`tiny-fingerprints`:** Body `{"tiny_fingerprints": ["…"]}`, Antwort `data.tiny_fingerprints` = die bekannten aus der Anfrage. Beispielwert: `5aec4c6e78052bf38fab901bcd1a2319_2021_12_8_22_13_10`.
- **`fast-upload`:** Body `{fingerprint (Pflicht), name, path, ext{drone_model_key, is_original, payload_model_key, tinny_fingerprint, sn}}`. Die Demo antwortet `code: 0`, wenn der Fingerprint bekannt ist, sonst einen Fehler-Code bei HTTP 200; dann lädt Pilot 2 normal hoch.
- **`sts`:** ohne Body. `expire` in Sekunden. Die Demo holt bei MinIO per AssumeRole echte Session-Daten und gibt `endpoint` (z. B. `http://192.168.1.1:9000`), `region` (`us-east-1`) und ein festes `object_key_prefix` aus.
- **`upload-callback`:** Pflicht sind nur `result`, `name`, `object_key`. `relative_altitude` ist als integer typisiert, `absolute_altitude` und `gimbal_yaw_degree` als number. `created_time` ist ein String ohne dokumentiertes Format.
- **`group-upload-callback`:** Body `{file_group_id, file_count, file_uploaded_count}`, gedacht für Wegpunkt-Missionen. Die Demo antwortet nur.
- **Suchlage (M3):** Garage statt MinIO, ohne STS. Die Umsetzung steht in `docs/superpowers/plans/2026-10-17-m3-fotos.md`.
