-- SUL-49: Ein Großbild-Zugang für Lifeline Hub darf die Vorschaubilder des laufenden Einsatzes
-- abrufen. Fotos zeigen Personen; ein reiner Anzeige-Zugang bekommt sie nicht ungefragt dazu.
ALTER TABLE kiosk_tokens ADD COLUMN photos INTEGER NOT NULL DEFAULT 0;
