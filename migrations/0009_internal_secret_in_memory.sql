-- SUL-108: Das Geheimnis des internen MediaMTX-Lesers (ADR 0003) entsteht seit diesem Stand bei
-- jedem Start neu und lebt nur im Speicher des Servers. Der alte, gespeicherte Wert fällt weg, damit
-- er in keiner Datenbank und keiner künftigen Sicherung mehr steht (ADR 0004).
DELETE FROM settings WHERE key = 'internal_secret';
