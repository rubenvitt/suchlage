-- Schema SUL-52: Ein Admin kann ein Konto sperren. Gesetzt heißt gesperrt: keine Sitzung, keine
-- Anmeldung, kein Zugriff über die Schnittstelle der Flugmappe. Zeit in ms seit Epoch (UTC).
ALTER TABLE users ADD COLUMN blocked_at_ms INTEGER;
