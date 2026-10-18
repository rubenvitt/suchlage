# Einsätze – Abnahme im Feld

Abnahme laut Spec §4 (Einsätze und Flugmappe). Abweichungen als Kommentar an die Aufgabe SUL-65 bzw.
SUL-76 auf dem Board. Kann im selben Termin wie die Feldabnahmen M2 und M3 laufen.

**Vorher:**

1. Neuen Stand (0.4.0) ausliefern (Runbook, „Betrieb → Update“). Die Migration 0004 läuft beim Start
   von selbst; vorhandene Flüge, Punkte und Fotos stehen danach unter „ohne Einsatz“.
2. Pilot 2 ist gekoppelt, die Fernsteuerung ist verbunden.

**Abnahme:**

- [ ] Auf `/einsaetze` einen Einsatz mit Namen und Ort anlegen und ein Suchgebiet als Kartenelement
      zuordnen (Lage, „Einsatz zuordnen“). Er steht unter „Vorbereitet“.
- [ ] Einsatz **starten**. Er steht unter „Laufend“; ein zweiter lässt sich nicht starten (Meldung
      „Es läuft schon ein Einsatz“).
- [ ] Pilot 2 zeigt das Suchgebiet. Falls nicht sofort: Karte in Pilot 2 neu öffnen (Zeit bis zur
      Anzeige notieren).
- [ ] In Pilot 2 einen Punkt setzen: Er erscheint im Browser auf der Lage und gehört zum laufenden
      Einsatz (Einsatzwahl „Einsatz wählen“ in der Leiste der Lage).
- [ ] Flug starten und landen: Der Flug steht in der Flugliste dieses Einsatzes und nicht unter
      „ohne Einsatz“.
- [ ] Optional: Abschnitt anlegen; ein weiterer Flug gehört zum aktiven Abschnitt, der frühere Flug
      bleibt im alten.
- [ ] Einsatz **abschließen**. Er steht unter „Abgeschlossen“ und ist nur noch lesbar.
- [ ] Pilot 2 zeigt jetzt die Elemente „ohne Einsatz“, nicht mehr die des abgeschlossenen Einsatzes.

**Feldfragen (Plan Einsätze, „Offene Punkte“):**

- [ ] Lädt Pilot 2 die Karte bei `map_group_refresh` von selbst neu? Falls nicht: Karte neu öffnen
      ist der Rückfall (Hinweis an den Piloten).
- [ ] Ignoriert Pilot 2 `map_element_delete` und `map_element_update` für Elemente, die es nie
      geladen hat, ohne Fehlermeldung oder Absturz? Protokoll der Seite ansehen.
- [ ] Landet ein Punkt, den Pilot 2 setzt, während **kein** Einsatz läuft, unter „ohne Einsatz“ und
      bleibt er in Pilot 2 sichtbar?

Ergebnisse, die den Plan ändern (Neuladen in Pilot 2, Verhalten bei Löschen/Ändern), gehören in
`docs/superpowers/plans/2026-10-18-einsaetze.md` unter „Offene Punkte“.
