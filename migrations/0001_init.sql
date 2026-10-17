-- Schema M1. Zeiten in ms seit Epoch (UTC).

CREATE TABLE users (
  id            INTEGER PRIMARY KEY,
  oidc_issuer   TEXT NOT NULL,
  oidc_subject  TEXT NOT NULL,
  display_name  TEXT NOT NULL,
  role          TEXT NOT NULL CHECK (role IN ('admin', 'operator')),
  created_at_ms INTEGER NOT NULL,
  UNIQUE (oidc_issuer, oidc_subject)
);

CREATE TABLE kiosk_tokens (
  id            INTEGER PRIMARY KEY,
  label         TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 80),
  token_hash    TEXT NOT NULL UNIQUE,
  created_at_ms INTEGER NOT NULL
);

CREATE TABLE sessions (
  token_hash     TEXT PRIMARY KEY,
  user_id        INTEGER REFERENCES users(id) ON DELETE CASCADE,
  kiosk_token_id INTEGER REFERENCES kiosk_tokens(id) ON DELETE CASCADE,
  expires_at_ms  INTEGER NOT NULL,
  created_at_ms  INTEGER NOT NULL,
  CHECK ((user_id IS NULL) <> (kiosk_token_id IS NULL))
);

CREATE TABLE settings (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE remote_controllers (
  id                INTEGER PRIMARY KEY,
  label             TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 80),
  pairing_code_hash TEXT NOT NULL UNIQUE,
  sn                TEXT UNIQUE,
  mqtt_password     TEXT,
  stream_key        TEXT,
  created_at_ms     INTEGER NOT NULL,
  paired_at_ms      INTEGER,
  CHECK ((sn IS NULL) = (paired_at_ms IS NULL))
);

CREATE TABLE flights (
  id              INTEGER PRIMARY KEY,
  aircraft_sn     TEXT NOT NULL,
  gateway_sn      TEXT NOT NULL,
  started_at_ms   INTEGER NOT NULL,
  ended_at_ms     INTEGER,
  end_reason      TEXT CHECK (end_reason IN ('landed', 'connection_lost')),
  max_elevation_m REAL,
  distance_m      REAL NOT NULL DEFAULT 0,
  CHECK ((ended_at_ms IS NULL) = (end_reason IS NULL))
);
-- Spec §5: höchstens ein laufender Flug je Fluggerät – auch über einen Neustart hinweg.
CREATE UNIQUE INDEX flights_one_open_per_aircraft ON flights (aircraft_sn) WHERE ended_at_ms IS NULL;
CREATE INDEX flights_by_start ON flights (started_at_ms DESC);

CREATE TABLE track_points (
  flight_id   INTEGER NOT NULL REFERENCES flights(id) ON DELETE CASCADE,
  t_ms        INTEGER NOT NULL,
  segment     INTEGER NOT NULL,
  lat         REAL NOT NULL,
  lon         REAL NOT NULL,
  elevation_m REAL,
  PRIMARY KEY (flight_id, t_ms)
) WITHOUT ROWID;

-- Letzter gemergter Gerätestand (osd+state) je Gerät. DJI schickt live_capacity, live_status und
-- den Startpunkt nur bei Änderung; nach einem Neustart kämen sie sonst nie wieder (Referenz 5.1).
CREATE TABLE device_state (
  sn            TEXT PRIMARY KEY,
  gateway_sn    TEXT NOT NULL,
  data          TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
