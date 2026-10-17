-- Schema M3: Fotos aus Pilot 2. Zeiten in ms seit Epoch (UTC).

-- Eine Zeile je Datei im Fotospeicher (Garage). `shot_key` fasst die Linsen einer Auslösung
-- zusammen (Fernsteuerung und Dateiname ohne Linsen-Endung). Ohne gültige Position bleibt
-- lat/lon leer: Das Foto ist dann gespeichert, aber nicht auf der Karte.
CREATE TABLE photos (
  id                  INTEGER PRIMARY KEY,
  object_key          TEXT NOT NULL UNIQUE,
  name                TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 255),
  fingerprint         TEXT,
  tiny_fingerprint    TEXT,
  shot_key            TEXT NOT NULL,
  lens                TEXT NOT NULL CHECK (lens IN ('thermal', 'wide', 'zoom', 'visible', 'unknown')),
  gateway_sn          TEXT NOT NULL,
  aircraft_sn         TEXT,
  flight_id           INTEGER REFERENCES flights(id) ON DELETE SET NULL,
  taken_at_ms         INTEGER,
  uploaded_at_ms      INTEGER NOT NULL,
  lat                 REAL,
  lon                 REAL,
  absolute_altitude_m REAL,
  relative_altitude_m REAL,
  gimbal_yaw_deg      REAL,
  payload_model_key   TEXT,
  thumb_ready         INTEGER NOT NULL DEFAULT 0 CHECK (thumb_ready IN (0, 1)),
  CHECK ((lat IS NULL) = (lon IS NULL))
);
CREATE INDEX photos_by_shot ON photos (shot_key);
CREATE INDEX photos_by_flight ON photos (flight_id);
CREATE INDEX photos_by_time ON photos (COALESCE(taken_at_ms, uploaded_at_ms));
CREATE INDEX photos_by_fingerprint ON photos (fingerprint);
CREATE INDEX photos_by_tiny_fingerprint ON photos (tiny_fingerprint);
