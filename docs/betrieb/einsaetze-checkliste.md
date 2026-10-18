# Einsätze – Abnahme im Feld

Abnahme laut Spec §4 (Einsätze und Flugmappe) und Spec „Einsatz-Bedienung und Löschen“. Abweichungen
als Kommentar an die Aufgabe SUL-65, SUL-76 bzw. SUL-78 auf dem Board. Kann im selben Termin wie die Feldabnahmen M2 und M3 laufen.

**Vorher:**

1. Neuen Stand (0.4.0) ausliefern (Runbook, „Betrieb → Update“). Die Migration 0004 läuft beim Start
   von selbst; vorhandene Flüge, Punkte und Fotos stehen danach unter „ohne Einsatz“. **Achtung:**
   Kurz nach dem Start löscht der Server Punkte ohne Einsatz, die seit mehr als 24 h nicht geändert
   wurden (Runbook, „Betrieb → Update“). Was bleiben soll, vor dem Update einem Einsatz zuordnen.
2. Pilot 2 ist gekoppelt, die Fernsteuerung ist verbunden.
3. Angemeldet mit einem Konto der Rolle `admin` (Löschen) und in einem zweiten Browser mit `operator`.

**Abnahme:**

- [ ] Ohne laufenden Einsatz zeigt `/` die **Startseite**. „Nur vorbereiten“ mit Name und Ort legt
      einen Einsatz an und öffnet die Lage mit „Vorbereitung: <Name>“. Ein Suchgebiet zeichnen; Pilot 2
      zeigt es noch nicht. „Zurück“ führt zur Startseite, der Einsatz steht unter den vorbereiteten.
- [ ] Auf der Startseite „Starten“ beim vorbereiteten Einsatz: Die Lage öffnet sich, die Kopfleiste
      zeigt Name, „Laufend“ mit Dauer und den aktiven Abschnitt. Ein zweiter lässt sich nicht starten
      (Meldung „Es läuft schon ein Einsatz“).
- [ ] Im zweiten Browser `/` neu laden: Die Lage des laufenden Einsatzes öffnet sich direkt, ohne
      Startseite.
- [ ] Pilot 2 zeigt das Suchgebiet. Falls nicht sofort: Karte in Pilot 2 neu öffnen (Zeit bis zur
      Anzeige notieren).
- [ ] In Pilot 2 einen Punkt setzen: Er erscheint im Browser auf der Lage des laufenden Einsatzes.
- [ ] Flug starten und landen: Der Flug steht in der Flugliste dieses Einsatzes und nicht unter
      „ohne Einsatz“.
- [ ] Menü in der Kopfleiste: „Einsatz bearbeiten“ ändert Name oder Ort, „Flüge dieses Einsatzes“
      öffnet die Flugliste des Einsatzes.
- [ ] Optional: „Neuer Abschnitt …“ im Menü; ein weiterer Flug gehört zum aktiven Abschnitt, der
      frühere Flug bleibt im alten.
- [ ] „Einsatz abschließen …“ während eines Flugs: abgelehnt, der Text des Servers steht im Dialog.
- [ ] Nach der Landung **abschließen**: Danach kommt die Startseite. Auf `/einsaetze` steht der
      Einsatz im Archiv; geöffnet zeigt die Lage „Archiv: <Name> · abgeschlossen“ und lässt sich nicht
      bearbeiten.
- [ ] Pilot 2 zeigt jetzt die Elemente „ohne Einsatz“, nicht mehr die des abgeschlossenen Einsatzes.

**Ohne Einsatz:**

- [ ] Startseite „Überspringen“: Die Kopfleiste zeigt deutlich „Ohne Einsatz“ und „Zeichnungen werden
      nach 24 h gelöscht“. Neu laden bleibt in der Lage; ein neuer Browser-Tab zeigt wieder die
      Startseite.
- [ ] Einen Punkt ohne Einsatz zeichnen und Uhrzeit notieren. Nach gut 24 h (spätestens 10 min
      danach) ist er im Browser und in Pilot 2 verschwunden. Flüge und Fotos ohne Einsatz bleiben.

**Löschen (nur `admin`):**

- [ ] Im Flugdetail eines Testflugs „Foto löschen …“ an einer Auslösung: Die Rückfrage nennt die
      Linsen, danach ist das Foto aus Detail und Lage verschwunden.
- [ ] „Video löschen …“: Danach meldet das Detail „Kein Video aufgezeichnet“; Spur und Fotos bleiben.
- [ ] „Flug löschen …“: Die Rückfrage nennt Spur, Video und Fotos, danach kommt die Flugliste. Ein
      zweiter Browser, der dasselbe Flugdetail offen hat, springt von selbst zur Flugliste.
- [ ] Im Archiv „Löschen …“ am abgeschlossenen Testeinsatz: Die Rückfrage nennt die Mengen und gibt
      erst nach Eintippen des Namens frei. Danach sind Flüge, Fotos und Zeichnungen weg, und unter
      `/data/video/live/<RC>` am Server liegen keine Segmente dieser Flüge mehr.
- [ ] Mit `operator` angemeldet: keine Löschknöpfe im Flugdetail und im Archiv.

**Feldfragen (Plan Einsätze, „Offene Punkte“):**

- [ ] Lädt Pilot 2 die Karte bei `map_group_refresh` von selbst neu? Falls nicht: Karte neu öffnen
      ist der Rückfall (Hinweis an den Piloten).
- [ ] Ignoriert Pilot 2 `map_element_delete` und `map_element_update` für Elemente, die es nie
      geladen hat, ohne Fehlermeldung oder Absturz? Protokoll der Seite ansehen.
- [ ] Landet ein Punkt, den Pilot 2 setzt, während **kein** Einsatz läuft, unter „ohne Einsatz“ und
      bleibt er in Pilot 2 sichtbar?

Ergebnisse, die den Plan ändern (Neuladen in Pilot 2, Verhalten bei Löschen/Ändern), gehören in
`docs/superpowers/plans/2026-10-18-einsaetze.md` unter „Offene Punkte“.
