-- Schema: Einsätze mit Abschnitten. Zeiten in ms seit Epoch (UTC). Bestehende Zeilen bleiben
-- ohne Einsatz (mission_id NULL).

CREATE TABLE missions (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
  location      TEXT,
  state         TEXT NOT NULL CHECK (state IN ('prepared', 'running', 'closed')),
  created_by    TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  started_at_ms INTEGER,
  closed_at_ms  INTEGER
);
-- Höchstens ein laufender Einsatz, auch über einen Neustart hinweg.
CREATE UNIQUE INDEX missions_one_running ON missions (state) WHERE state = 'running';

CREATE TABLE mission_sections (
  id            INTEGER PRIMARY KEY,
  mission_id    INTEGER NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
  label         TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 120),
  location      TEXT,
  created_at_ms INTEGER NOT NULL
);
CREATE INDEX mission_sections_by_mission ON mission_sections (mission_id);

ALTER TABLE flights ADD COLUMN mission_id INTEGER REFERENCES missions(id);
ALTER TABLE flights ADD COLUMN section_id INTEGER REFERENCES mission_sections(id);
ALTER TABLE map_elements ADD COLUMN mission_id INTEGER REFERENCES missions(id);
ALTER TABLE photos ADD COLUMN mission_id INTEGER REFERENCES missions(id);
CREATE INDEX flights_by_mission ON flights (mission_id);
CREATE INDEX map_elements_by_mission ON map_elements (mission_id);
CREATE INDEX photos_by_mission ON photos (mission_id);
