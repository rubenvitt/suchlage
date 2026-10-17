-- Schema M2: Kartenelemente und API-Token der Fernsteuerungen. Zeiten in ms seit Epoch (UTC).

-- Punkte, Linien und Flächen aus Browser und Pilot 2. Die ID vergibt Pilot 2 selbst (UUID), für
-- Elemente aus dem Browser der Server. `points` ist JSON: [{"lat":…,"lon":…,"alt_m":…|null}, …],
-- bei Flächen der Ring ohne wiederholten Schlusspunkt.
CREATE TABLE map_elements (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
  kind            TEXT NOT NULL CHECK (kind IN ('point', 'line', 'polygon')),
  points          TEXT NOT NULL,
  color           TEXT NOT NULL,
  clamp_to_ground INTEGER NOT NULL CHECK (clamp_to_ground IN (0, 1)),
  author          TEXT NOT NULL,
  origin          TEXT NOT NULL CHECK (origin IN ('pilot', 'web')),
  created_at_ms   INTEGER NOT NULL,
  updated_at_ms   INTEGER NOT NULL
);

-- Token für die Kartenelemente (HTTPS und WebSocket von Pilot 2). Nur der Hash; jedes Koppeln
-- erzeugt einen neuen. SQLite erlaubt UNIQUE nicht in ADD COLUMN, deshalb ein eigener Index.
ALTER TABLE remote_controllers ADD COLUMN api_token_hash TEXT;
CREATE UNIQUE INDEX remote_controllers_api_token ON remote_controllers (api_token_hash);
