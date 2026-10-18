-- Schema SUL-66: Verknüpfung mit der Flugmappe. Zeiten in ms seit Epoch (UTC). Bestehende Einsätze
-- stammen aus dem Browser (`origin = 'web'`) und sind mit keiner Flugmappe verknüpft.

ALTER TABLE missions ADD COLUMN origin TEXT NOT NULL DEFAULT 'web' CHECK (origin IN ('web', 'flugmappe'));
-- Einsatz-ID der Flugmappe (UUID), Anzeigename des Kontos, das verknüpft hat, und wann.
ALTER TABLE missions ADD COLUMN flugmappe_mission_id TEXT;
ALTER TABLE missions ADD COLUMN flugmappe_linked_by TEXT;
ALTER TABLE missions ADD COLUMN flugmappe_linked_at_ms INTEGER;
-- Gesetzt, sobald die Kartenelemente der Flugmappe übernommen sind (einmal je Einsatz).
ALTER TABLE missions ADD COLUMN flugmappe_map_imported_at_ms INTEGER;
-- Eine Flugmappe gehört zu höchstens einem Einsatz. SQLite erlaubt UNIQUE nicht in ADD COLUMN.
CREATE UNIQUE INDEX missions_by_flugmappe ON missions (flugmappe_mission_id)
  WHERE flugmappe_mission_id IS NOT NULL;

-- Abschnitts-ID der Flugmappe; gesetzt, wenn ein Kartenfoto (Papierkarte) liegt.
ALTER TABLE mission_sections ADD COLUMN flugmappe_section_id TEXT;
ALTER TABLE mission_sections ADD COLUMN map_photo_at_ms INTEGER;
CREATE UNIQUE INDEX mission_sections_by_flugmappe ON mission_sections (mission_id, flugmappe_section_id)
  WHERE flugmappe_section_id IS NOT NULL;
