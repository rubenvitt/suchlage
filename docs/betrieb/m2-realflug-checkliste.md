# M2 Kartenelemente – Abnahme im Feld

Abnahme laut Spec §7: Eine Fläche, die im FüKw gezeichnet wird, erscheint auf der RC Plus. Ein Punkt, den der Pilot setzt, erscheint im FüKw. Abweichungen als Kommentar an die Aufgabe „Feldabnahme M2“ auf dem Board.

**Vorher:** Den neuen Stand ausliefern (Runbook, Abschnitt „Betrieb → Update“). Danach in Pilot 2 die Cloud-Seite von Suchlage einmal neu öffnen. Erst dieses Koppeln lädt die Module `api`, `ws` und `map`. Im Protokoll der Seite stehen danach drei weitere `platformLoadComponent: ok` und `Kartenabgleich: verbunden`.

- [ ] Pilot-Seite: kein „Kartenelemente nicht verfügbar“ im Protokoll. Falls doch: den Fehlercode notieren.
- [ ] Browser → RC: Am Arbeitsplatz eine **Fläche** zeichnen (Zeichnen → Fläche, Eckpunkte setzen, Fertig, Anlegen). Sie erscheint in Pilot 2 auf der Karte. Zeit bis zur Anzeige notieren.
- [ ] RC → Browser: In Pilot 2 einen **Punkt** setzen. Er erscheint am Arbeitsplatz, auf dem Großbild und mobil. Zeit notieren.
- [ ] Linie in beide Richtungen.
- [ ] Ändern in Pilot 2 (verschieben, umbenennen, Farbe) kommt im Browser an. Umbenennen und Umfärben im Browser kommt in Pilot 2 an.
- [ ] Löschen in beide Richtungen.
- [ ] Farben: alle sechs Farben aus dem Browser anlegen und in Pilot 2 vergleichen, vor allem Violett (`#B620E0`) und Schwarz (`#212121`). Welche Farbwerte schickt Pilot 2 selbst? Dazu das Element im Browser auswählen: Ist keine der sechs Farben markiert, schickt Pilot 2 eine eigene.
- [ ] Echo: Erscheint ein in Pilot 2 gesetzter Punkt dort doppelt? (Der Server schickt jede Änderung an alle Pilots, auch an den Absender.)
- [ ] Namen: Wie benennt Pilot 2 neue Elemente (Präfix `Pilot`)? Was steht im Browser als Urheber?
- [ ] Neustart der App Pilot 2: Sind die Elemente danach noch da, ohne die Cloud-Seite neu zu öffnen? Kommen neue aus dem Browser weiter an?
- [ ] Server-Neustart (`docker compose -f compose.m1.yaml restart server`): Pilot 2 verbindet den Kartenabgleich selbst neu; ein danach gezeichneter Punkt kommt an.
- [ ] Höhe: Ein Punkt aus Pilot 2 trägt eine Höhe. Nach Umbenennen im Browser zeigt Pilot 2 ihn an derselben Stelle und Höhe.

Ergebnisse, die den Plan ändern (Echo, Farben, Namen), gehören in `docs/superpowers/plans/2026-10-17-m2-kartenelemente.md` unter „Offene Punkte“.
