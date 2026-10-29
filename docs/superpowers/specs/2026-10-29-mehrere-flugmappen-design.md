# Mehrere Flugmappen an einem Einsatz und das Flugbuch in Suchlage – Nachtrag

- **Stand:** 29.10.2026, aus der Klärung zu SUL-84
- **Status:** Entwurf zur Durchsicht
- **Ändert:** `2026-10-18-einsaetze-und-flugmappe-design.md` (§3 Nicht-Ziele, §4 Modell, §6 Übernehmen, §7 Fehlerfälle). Was hier nicht steht, gilt dort weiter.
- **Repos:** `rubenvitt/suchlage` (dieses Dokument) und `rubenvitt/uav-checklists` (Flugmappe)

## 1. Anlass

Heute hängt an einem Suchlage-Einsatz genau eine Flugmappe. Eine zweite kann den Einsatz nur **übernehmen**, danach steht die erste still („mit anderer Flugmappe verknüpft“). Gedacht war das für Geräte- und Schichtwechsel. Im Einsatz arbeiten aber mehrere Trupps mit je eigener Flugmappe am selben Einsatz, etwa einer mit der M350 RTK und einer mit der M200.

Dazu kommt: Suchlage zeichnet nur die Flüge auf, die über Pilot 2 eingehen (heute die M350). Flüge der M200 stehen nur im Flugbuch der Flugmappe. In Suchlage soll aber **alles** zu sehen sein: das Livebild und eine übersichtliche Liste aller Flüge des Einsatzes, egal ob sie aus der Telemetrie kommen oder von Hand in der Flugmappe eingetragen wurden.

## 2. Entscheidungen

Von Ruben am 29.10.2026 entschieden:

| Thema | Entscheidung |
|---|---|
| Mehrere Flugmappen | Mehrere Flugmappen dürfen **gleichzeitig** mit einem Einsatz verknüpft sein, gleichberechtigt. „Übernehmen“ und „abgelöst“ entfallen. |
| Flüge in Suchlage | Suchlage zeigt je Einsatz alle Flüge: aus der Telemetrie und die von Hand in einer Flugmappe eingetragenen (z. B. M200). Die Flugmappe meldet dafür ihr Flugbuch an Suchlage. |

Entscheidungen dieses Entwurfs (bei der Durchsicht zu bestätigen):

| Thema | Entscheidung |
|---|---|
| Bestätigen | Ein Telemetrie-Flug ist in allen verknüpften Flugmappen Vorschlag, bis eine ihn bestätigt. Die Bestätigung geht an Suchlage; danach ist er in den anderen kein Vorschlag mehr. Eine zweite Bestätigung desselben Flugs ist ein Konflikt (409). So steht kein Flug in zwei Berichten. |
| Abschnitte | Jeder Abschnitt gehört der Flugmappe, die ihn angelegt hat. Eine Verlegung eines Trupps legt einen Abschnitt an, der nur für **diese** Flugmappe der aktive ist. Für Suchlage (Browser, Pilot 2, Großbild) bleibt der zuletzt angelegte der aktive. |
| Abschnitt eines Flugs | Telemetrie: wie bisher der beim Start aktive Abschnitt des Einsatzes. Beim Bestätigen setzt die Flugmappe den Abschnitt ihres Eintrags, Suchlage übernimmt ihn. Handeintrag: der Abschnitt aus der Flugmappe. |
| Abschluss | Der Abschluss in einer Flugmappe meldet **ihren** Teil als fertig. Der Einsatz schließt erst, wenn keine verknüpfte Flugmappe mehr offen ist. Suchlage kann wie bisher jederzeit abschließen (Ausweg bei verlorener Flugmappe). |
| Geräte- und Schichtwechsel | Ein neues Gerät verknüpft sich einfach dazu. Die alte Verknüpfung bleibt, bis sie fertig gemeldet oder in Suchlage gelöst wird; sie stört niemanden. |

## 3. Änderungen an der bisherigen Spec

- **§3 Nicht-Ziele:** „mehrere Drohnen“ gilt weiter für die **Telemetrie** (eine Drohne über Pilot 2). Mehrere Flugmappen und Handeinträge anderer Fluggeräte sind jetzt Ziel.
- **§4 Modell:** Ein Einsatz hat **keine oder mehrere** Verknüpfungen mit Flugmappen statt optional einer Einsatz-ID. Ein Abschnitt trägt, welche Verknüpfung ihn angelegt hat.
- **§4 Zuordnung, Flug:** Ein bestätigter Flug bekommt den Abschnitt aus der Bestätigung (siehe §2). Ein Handeintrag gehört dem Einsatz und Abschnitt, mit dem die Flugmappe ihn meldet.
- **§6 Übernehmen:** entfällt. An seine Stelle tritt „Dazukommen“: Verknüpfen eines Einsatzes, der schon mit einer anderen Flugmappe verknüpft ist, ist kein Konflikt mehr.
- **§6 Abschluss nachholen:** bleibt, bezieht sich aber auf den Teil der Flugmappe.
- **§7 Fehlerfälle:** „Zwei Flugmappen verknüpfen denselben Einsatz → zweite bekommt 409“ entfällt. Neu siehe §6 dieses Nachtrags.

## 4. Teil A: Suchlage

### Modell

- **Verknüpfung** (neue Tabelle `mission_flugmappe_links`): Einsatz, Einsatz-ID der Flugmappe (UUID, systemweit eindeutig wie bisher), verknüpft von, verknüpft um, Fluggerät der Flugmappe (Bezeichnung, z. B. „Matrice 200“), fertig um (optional), gelöst um (optional). Die Migration übernimmt die bestehende Verknüpfung aus `missions.flugmappe_*` und entfernt die Spalten.
- **Abschnitt:** neu „angelegt von Verknüpfung“ (optional; in Suchlage angelegte haben keine). Die Abschnitts-ID der Flugmappe ist nur innerhalb ihrer Verknüpfung eindeutig (`(Verknüpfung, Abschnitts-ID)` statt heute `(Einsatz, Abschnitts-ID)`), damit gleiche lokale IDs zweier Flugmappen nicht kollidieren.
- **Flugbucheintrag der Flugmappe** (neue Tabelle): Verknüpfung, Eintrags-ID der Flugmappe (UUID), optional der Suchlage-Flug (bei bestätigten), Abschnitt, Start, Landung (optional, solange in der Luft), Fluggerät, Fernpilot, Landestatus. Ein Suchlage-Flug hat höchstens einen Eintrag (eindeutig).
- Ein Handeintrag ist **kein** Flug im Sinne von `flights` (keine Spur, kein Video, keine Fotos). Er erscheint in der Flugliste und im Einsatz, nicht auf der Karte.

### Regeln

- **Bindung:** Jede verknüpfte, nicht gelöste Flugmappe darf am Einsatz lesen und schreiben. Eine gelöste bekommt 409 („Verknüpfung in Suchlage gelöst“).
- **Verknüpfen:** wie bisher (vorbereitet → laufend, kein anderer Einsatz läuft), aber ohne `take_over`. Ist die Flugmappe schon mit einem anderen Einsatz verknüpft und diese Verknüpfung nicht gelöst, 409 wie bisher. Eine gelöste Verknüpfung blockiert kein neues Verknüpfen.
- **Abschnitt anlegen:** wird der aktive Abschnitt des Einsatzes und der Flugmappe. Die Flugmappe adressiert wie bisher über ihre eigene Abschnitts-ID; IDs anderer Flugmappen sieht sie nicht.
- **Flugbuch melden:** Die Flugmappe schreibt jeden Eintrag mit seiner ID (`PUT`, wiederholbar) und löscht ihn mit `DELETE`, wenn er in der Flugmappe gelöscht wird. Ein Eintrag mit Suchlage-Flug, den schon eine andere Flugmappe gemeldet hat, ist ein Konflikt (409). Fehlt Netz, sammelt die Flugmappe die Meldungen und holt sie nach.
- **Abschluss aus der Flugmappe:** setzt „fertig um“ an der Verknüpfung. Sind danach alle nicht gelösten Verknüpfungen fertig, schließt der Einsatz (Regeln wie bisher: kein laufender Flug, sonst merkt die Flugmappe es sich). Ein Eintrag nach „fertig“ wird noch angenommen, solange der Einsatz offen ist.
- **Lösen in Suchlage:** `operator` und `admin` können eine Verknüpfung lösen (verlorenes Gerät). Gemeldete Einträge bleiben.

### Bedienung in Suchlage

- **Einsatz:** zeigt die verknüpften Flugmappen mit Fluggerät, wer verknüpft hat, Zustand (offen, fertig, gelöst) und „Lösen“.
- **Flugliste des Einsatzes:** alle Flüge nach Startzeit. Je Zeile: Start, Landung, Dauer, Fluggerät, Abschnitt, Quelle („Telemetrie“, „Telemetrie, bestätigt in Flugmappe <Fluggerät>“ oder „Flugmappe“). Telemetrie-Flüge führen wie bisher zum Flugdetail mit Spur und Video, Handeinträge haben keine Detailseite.
- Die Liste „ohne Einsatz“ und das Flugdetail bleiben, wie sie sind.

### Schnittstelle (`/api/flugmappe/v2`, Änderungen gegenüber v1)

| Aufruf | Änderung |
|---|---|
| Wer bin ich | unverändert |
| Offene Einsätze, Einsatz | `links` (Liste aller Verknüpfungen) statt `link`; die Flugmappe erkennt ihre an der UUID |
| Einsatz anlegen, Mit Einsatz verknüpfen | neu: Fluggerät der Flugmappe; `take_over` entfällt |
| Flüge des Einsatzes | je Flug neu: bestätigt von (Fluggerät der anderen Flugmappe) oder `null`; eigene Bestätigungen erkennt die Flugmappe an ihrem Eintrag |
| Flugbucheintrag melden | neu: `PUT /missions/{id}/logbook/{entry_id}`, `DELETE` ebenso |
| Einsatz abschließen | meldet den Teil der Flugmappe als fertig; Antwort sagt, ob der Einsatz geschlossen ist |

Die Felder ändern sich nicht abwärtskompatibel. Weil die Flugmappe die Typen als Kopie übernimmt und nur eine Flugmappe-Version im Feld ist, steigt die Version auf **v2** (`/api/flugmappe/v2`, `api_version: 2`). Suchlage bietet v1 nicht weiter an; die Flugmappe zeigt bei `api_version` 1 bzw. 404 auf v2 die Suchlage-Teile nicht (wie ohne Anmeldung). Reihenfolge beim Ausrollen: Suchlage und Flugmappe am selben Tag, Suchlage zuerst.

CORS für den Flugmappe-Bereich erlaubt zusätzlich `DELETE` (für das Löschen von Flugbucheinträgen); der Preflight-Test prüft das mit.

## 5. Teil B: Flugmappe

- **Abschnitt „Suchlage“:** zeigt die anderen verknüpften Flugmappen (Fluggerät, wer). Kein „Übernehmen“-Dialog und kein „abgelöst“-Hinweis mehr.
- **Verknüpfen:** schickt das gewählte Fluggerät mit.
- **Flugbuch:** Vorschläge nur für Flüge, die keine andere Flugmappe bestätigt hat. Ein von einer anderen bestätigter Flug verschwindet als Vorschlag; war er hier gerade offen, sagt ein Hinweis, wer ihn hat.
- **Melden:** Jeder Flugbucheintrag (bestätigt oder von Hand) geht an Suchlage, ebenso Änderungen und Löschen. Eine Ausgangsliste wie beim Abschluss holt Meldungen nach. Ein 409 beim Melden einer Bestätigung macht den Eintrag wieder zum Handeintrag ohne Suchlage-Bezug und zeigt einen Hinweis; nichts geht verloren.
- **Abschluss:** heißt in der Flugmappe weiter „Einsatz abschließen“. Ist der Suchlage-Einsatz danach noch offen, weil andere Trupps fliegen, zeigt die Flugmappe das als Hinweis, nicht als Fehler.
- Ohne Suchlage, ohne Anmeldung oder bei Netzausfall arbeitet die Flugmappe wie heute.

## 6. Fehlerfälle

| Fall | Verhalten |
|---|---|
| Zwei Flugmappen bestätigen denselben Flug fast gleichzeitig | die zweite Meldung bekommt 409; ihr Eintrag wird Handeintrag mit Hinweis „Flug wurde von <Fluggerät> bestätigt“ |
| Flugmappe verloren | in Suchlage „Lösen“; Einsatz schließt, wenn die übrigen fertig sind, oder in Suchlage |
| Gelöste Flugmappe meldet sich wieder | 409, Hinweis „Verknüpfung in Suchlage gelöst“; sie kann neu verknüpfen |
| Handeintrag nach Abschluss des Einsatzes | 422 wie bisher; die Flugmappe behält den Eintrag, Suchlage zeigt ihn nicht |
| Gleiche Zeiten in Handeintrag und Telemetrie-Flug | kein Zusammenführen in Suchlage. Wer die M350 von Hand einträgt statt zu bestätigen, sieht den Flug doppelt; die Flugmappe bietet wie bisher das Zusammenführen an. |

## 7. Tests

- Suchlage: Regeln als reine Funktionen (Bindung mit mehreren Verknüpfungen, Abschluss erst bei der letzten, Bestätigung eindeutig, Lösen); Migration mit einer bestehenden Verknüpfung; Schnittstelle mit Test-IdP für zwei Flugmappen am selben Einsatz (409 bei doppelter Bestätigung, 422 nach Abschluss).
- Flugmappe: Vorschläge ohne fremd bestätigte Flüge, Ausgangsliste für Meldungen, 409 beim Melden, Abschluss mit offenem Einsatz.
- Feldabnahme (SUL-82): zwei Flugmappen am selben Übungseinsatz, eine mit M350 (bestätigt), eine mit M200 (von Hand); Suchlage zeigt alle Flüge.

## 8. Offen für den Plan

- genaue Feldnamen und ob Fernpilot und Landestatus in Suchlage angezeigt werden oder nur gespeichert
- ob Suchlage die Bezeichnungen der Fluggeräte aus der Flugmappe übernimmt oder eine eigene Liste führt
- Aussehen der Flugliste im Einsatz (Tabelle oder Karten wie heute)
