-- IDA-359: Ein Eintrag im Einsatztagebuch der IUK-Suite ist mit diesem Einsatz verknüpft (Zeit in
-- ms seit Epoch, UTC). Gesetzt bleibt er für immer: Der Eintrag ist versiegelt, der Einsatz lässt
-- sich danach nicht mehr löschen und wird dem Einsatztagebuch nicht mehr angeboten.
ALTER TABLE missions ADD COLUMN etb_linked_at_ms INTEGER;
