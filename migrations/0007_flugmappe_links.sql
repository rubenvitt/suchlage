-- Schema SUL-113: mehrere Flugmappen an einem Einsatz und ihr Flugbuch. Zeiten in ms seit Epoch
-- (UTC). Die bestehende Verknüpfung aus `missions.flugmappe_*` wird die erste Verknüpfung.

CREATE TABLE mission_flugmappe_links (
  id                   INTEGER PRIMARY KEY,
  mission_id           INTEGER NOT NULL REFERENCES missions(id) ON DELETE CASCADE,
  -- Einsatz-ID der Flugmappe (UUID, klein geschrieben).
  flugmappe_mission_id TEXT NOT NULL,
  -- Anzeigename des Kontos, das verknüpft hat.
  linked_by            TEXT NOT NULL,
  linked_at_ms         INTEGER NOT NULL,
  -- Fluggerät der Flugmappe, z. B. „Matrice 200“.
  aircraft_label       TEXT NOT NULL CHECK (length(aircraft_label) BETWEEN 1 AND 120),
  -- Die Flugmappe hat ihren Teil abgeschlossen.
  done_at_ms           INTEGER,
  -- In Suchlage gelöst (verlorenes Gerät); die Flugmappe darf dann nichts mehr.
  released_at_ms       INTEGER
);
-- Je Einsatz höchstens eine Verknüpfung mit derselben Flugmappe; Wiederverknüpfen belebt sie.
CREATE UNIQUE INDEX mission_flugmappe_links_by_mission ON mission_flugmappe_links (mission_id, flugmappe_mission_id);
-- Systemweit höchstens eine nicht gelöste Verknüpfung je Flugmappe.
CREATE UNIQUE INDEX mission_flugmappe_links_active ON mission_flugmappe_links (flugmappe_mission_id)
  WHERE released_at_ms IS NULL;

-- Ein abgeschlossener Einsatz hat auch den Teil seiner Flugmappe abgeschlossen.
INSERT INTO mission_flugmappe_links
  (mission_id, flugmappe_mission_id, linked_by, linked_at_ms, aircraft_label, done_at_ms)
SELECT id, flugmappe_mission_id, flugmappe_linked_by, flugmappe_linked_at_ms, 'Fluggerät unbekannt',
       CASE WHEN state = 'closed' THEN closed_at_ms END
FROM missions WHERE flugmappe_mission_id IS NOT NULL;

-- Abschnitt: welche Verknüpfung ihn angelegt (oder beim Verknüpfen übernommen) hat. Die
-- Abschnitts-ID der Flugmappe ist nur innerhalb ihrer Verknüpfung eindeutig.
ALTER TABLE mission_sections ADD COLUMN link_id INTEGER REFERENCES mission_flugmappe_links(id);
UPDATE mission_sections SET link_id =
  (SELECT l.id FROM mission_flugmappe_links l WHERE l.mission_id = mission_sections.mission_id)
WHERE flugmappe_section_id IS NOT NULL;
UPDATE mission_sections SET flugmappe_section_id = NULL WHERE link_id IS NULL;
DROP INDEX mission_sections_by_flugmappe;
CREATE UNIQUE INDEX mission_sections_by_flugmappe ON mission_sections (link_id, flugmappe_section_id)
  WHERE flugmappe_section_id IS NOT NULL;

DROP INDEX missions_by_flugmappe;
ALTER TABLE missions DROP COLUMN flugmappe_mission_id;
ALTER TABLE missions DROP COLUMN flugmappe_linked_by;
ALTER TABLE missions DROP COLUMN flugmappe_linked_at_ms;

-- Flugbucheintrag der Flugmappe: bestätigter Telemetrie-Flug (`flight_id`) oder Handeintrag.
-- `deleted = 1` ist eine Löschmarke: Sie hält nur die Revision, damit eine verspätete ältere
-- Meldung nichts wiederherstellt.
CREATE TABLE flugmappe_logbook_entries (
  id             INTEGER PRIMARY KEY,
  link_id        INTEGER NOT NULL REFERENCES mission_flugmappe_links(id) ON DELETE CASCADE,
  -- Eintrags-ID der Flugmappe (UUID).
  entry_id       TEXT NOT NULL,
  revision       INTEGER NOT NULL CHECK (revision >= 1),
  deleted        INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
  -- Ein gelöschter Flug macht aus der Bestätigung einen Handeintrag.
  flight_id      INTEGER REFERENCES flights(id) ON DELETE SET NULL,
  section_id     INTEGER REFERENCES mission_sections(id) ON DELETE SET NULL,
  started_at_ms  INTEGER,
  landed_at_ms   INTEGER,
  aircraft_label TEXT,
  pilot          TEXT,
  landing_status TEXT,
  -- Änderungszeitpunkt in der Flugmappe, nur zur Anzeige.
  changed_at_ms  INTEGER,
  received_at_ms INTEGER NOT NULL,
  CHECK (deleted = 1 OR (started_at_ms IS NOT NULL AND aircraft_label IS NOT NULL)),
  CHECK (deleted = 0 OR flight_id IS NULL)
);
CREATE UNIQUE INDEX flugmappe_logbook_by_link ON flugmappe_logbook_entries (link_id, entry_id);
-- Ein Suchlage-Flug steht in höchstens einem Flugbuch.
CREATE UNIQUE INDEX flugmappe_logbook_by_flight ON flugmappe_logbook_entries (flight_id)
  WHERE flight_id IS NOT NULL;
