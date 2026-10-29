# Einsätze in Suchlage und Verknüpfung mit der Flugmappe – Design

- **Stand:** 18.10.2026, aus dem Brainstorming zu SUL-1
- **Status:** Entwurf zur Durchsicht
- **Nachtrag:** `2026-10-29-mehrere-flugmappen-design.md` (SUL-84) ersetzt „Übernehmen“ durch mehrere gleichzeitig verknüpfte Flugmappen und ergänzt das Flugbuch der Flugmappe in Suchlage.
- **Repos:** `rubenvitt/suchlage` (dieses Dokument) und `rubenvitt/uav-checklists` (Flugmappe)

## 1. Zweck

Die Flugmappe ist das eingeführte Werkzeug des Trupps: Einsatzdaten, Vorflugkontrolle, Flugbuch, Nachbereitung und PDF-Bericht. Suchlage ist neu und weiß vieles, was der Truppführer heute in der Flugmappe von Hand einträgt: wann gestartet und gelandet wurde, mit welchem Fluggerät, wie hoch und wie weit geflogen wurde, wo die Suchgebiete liegen.

Die Verknüpfung soll **den Truppführer entlasten**. Was Suchlage ohnehin weiß, tippt er nicht noch einmal ein. Er bestätigt nur noch.

Suchlage kennt bisher keine Einsätze: Flüge, Kartenelemente und Fotos sind global. Deshalb gehören zu diesem Vorhaben drei Teile:

| Teil | Repo | Inhalt |
|---|---|---|
| **A Einsätze in Suchlage** | suchlage | Einsatz mit Zuständen und Abschnitten; Flüge, Kartenelemente und Fotos gehören zu einem Einsatz |
| **B Kartensystem der Flugmappe** | uav-checklists | Leaflet/Geoman durch das Kartensystem von Suchlage ersetzen (MapLibre, terra-draw, VersaTiles) |
| **C Verknüpfung** | beide | Schnittstelle in Suchlage, Anmeldung über Pocket ID, Einsatz verknüpfen, Flugbuch mit Bestätigung, Lagekarte und Kartenfoto |

Reihenfolge: A und B unabhängig voneinander, C setzt beide voraus.

**Erfolg:** Ein Übungseinsatz, bei dem der Truppführer in der Flugmappe keinen Start und keine Landung von Hand eintippt, jeden Flug nur bestätigt, und der PDF-Bericht trotzdem vollständig ist: Flugzeiten, Fluggerät, Höhe, Strecke und eine Karte mit Suchgebieten und Flugspuren.

## 2. Entscheidungen

Von Ruben im Brainstorming (18.10.2026) entschieden:

| Thema | Entscheidung |
|---|---|
| Wo ein Einsatz entsteht | normalerweise in der Flugmappe, die der Truppführer zuerst öffnet (Vorflugkontrolle). Suchlage kann Einsätze auch selbst anlegen, um sie vorzubereiten (Suchgebiete vorab zeichnen). |
| Verknüpfen | Die Flugmappe zeigt offene Einsätze aus Suchlage zur Auswahl oder legt einen neuen an. Immer auf ausdrückliche Wahl des Truppführers, nie still im Hintergrund. |
| Einsatzzustände | *vorbereitet*, *laufend*, *abgeschlossen*. Höchstens ein Einsatz läuft gleichzeitig. |
| Kartenelemente | gehören zu einem Einsatz. Pilot 2 sieht nur die Elemente des laufenden Einsatzes. |
| Abschnitte | Ein Abschnitt der Flugmappe (Verlegung) ist derselbe Abschnitt in Suchlage. |
| Anmeldung | Ohne Pocket-ID-Anmeldung in der Flugmappe und ohne Rechte in Suchlage passiert nichts; die Flugmappe arbeitet dann wie heute. Die Rechte bleiben allein in Suchlage. |
| Erreichbarkeit | Suchlage ist öffentlich unter `suchlage.iuk-ue.de` erreichbar, die Flugmappe unter `uav.iuk-ue.de`. Der Login der Flugmappe ist dort aktiv (Signatur-Backend konfiguriert) und nutzt dieselbe Pocket-ID-Instanz wie Suchlage. |
| Flugbuch | Suchlage zeichnet Flüge automatisch auf. In der Flugmappe muss der Truppführer jeden Flug bestätigen. |
| Lagekarte | Bei verknüpften Einsätzen zeichnet niemand mehr in der Flugmappe; die Karte kommt aus Suchlage. |
| Kartenfoto | Die Foto-Variante der Einsatzkarte bleibt erhalten. Bei verknüpften Einsätzen geht das Foto auch nach Suchlage. |
| Kartensystem der Flugmappe | Leaflet wird überall ersetzt, auch bei Einsätzen ohne Suchlage. |
| Meldepflicht | Der Hinweis „Start und Landung an FüKw melden“ bleibt. Suchlage ersetzt keine Meldung. |

## 3. Ziele, Nicht-Ziele, Anti-Ziele

**Ziele**
- Flugbuch füllt sich aus der Telemetrie, der Truppführer bestätigt.
- Suchgebiete werden einmal gezeichnet (in Suchlage) und erscheinen in Pilot 2, in der Flugmappe und im Bericht.
- Einsätze lassen sich in Suchlage vorbereiten.

**Nicht-Ziele**
- Suche (Sektoren, Abdeckung, Hinweise, Export) – bleibt SUL-48; sie baut aber auf den Einsätzen aus Teil A auf.
- Schnittstelle zu Lifeline Hub (SUL-49).
- Löschfristen oder Aufbewahrungsregeln in Suchlage. Daten bleiben wie bisher.
- Fotos oder Video der Drohne im PDF-Bericht. Der Bericht nennt nur, dass es sie in Suchlage gibt.
- Die Papierkarte auf der Lagekarte einpassen (Georeferenzieren). Sie ist in Suchlage nur ein Bild.
- Besatzung, Wetter, SORA oder Checklisten in Suchlage.
- Mehrere gleichzeitig laufende Einsätze oder mehrere Drohnen.
- Betrieb ohne Internet im FüKw (M4, SUL-47). Siehe Risiken.

**Anti-Ziele** (wäre ein Fehlschlag, auch wenn es technisch läuft)
- Der Truppführer hat mehr zu tun als vorher, etwa durch Doppeleinträge, Fehlermeldungen oder Pflichtschritte.
- Die Flugmappe funktioniert nicht mehr ohne Suchlage, ohne Anmeldung oder bei Netzausfall.
- Ein Flug steht im Bericht, den niemand bestätigt hat.
- Ein Flug oder Kartenelement landet unbemerkt im falschen Einsatz.

## 4. Teil A: Einsätze in Suchlage

### Modell

- **Einsatz:** Name (Pflicht), Ort (optional, Freitext), Zustand, angelegt von, Zeitpunkte für Anlegen, Start und Abschluss, Herkunft (`web` oder `flugmappe`), optional die Einsatz-ID der Flugmappe.
- **Abschnitt:** gehört zu einem Einsatz; Bezeichnung, Ort (optional), angelegt um, optional die Abschnitts-ID der Flugmappe. Jeder Einsatz hat mindestens einen Abschnitt; der zuletzt angelegte ist der aktive.
- **Zustände:** *vorbereitet* → *laufend* → *abgeschlossen*. Ein vorbereiteter Einsatz darf direkt abgeschlossen (verworfen) werden. Zurück geht es nicht.
- **Höchstens ein laufender Einsatz** systemweit, in der Datenbank abgesichert (wie `flights_one_open_per_aircraft`). Einen zweiten zu starten ist ein Konflikt (409).

### Zuordnung

| Was | Gehört zu |
|---|---|
| Flug | dem Einsatz und Abschnitt, die beim **Start** des Flugs laufen bzw. aktiv sind. Spätere Wechsel ändern einen laufenden Flug nicht. |
| Kartenelement aus Pilot 2 | dem laufenden Einsatz |
| Kartenelement aus dem Browser | dem Einsatz, der im Browser gerade ausgewählt ist (auch einem vorbereiteten) |
| Foto | dem Einsatz seines Flugs; ohne Flug dem beim Hochladen laufenden Einsatz |
| Kartenfoto (Papierkarte) | dem Abschnitt, für den es hochgeladen wurde |

Was ohne laufenden Einsatz entsteht, gehört zu **keinem Einsatz** und erscheint in Suchlage unter „ohne Einsatz“. `operator` und `admin` können es nachträglich einem Einsatz zuordnen (Flüge mit ihren Fotos gemeinsam; Kartenelemente einzeln).

Bestandsdaten (alles vor der Migration) gehören zu keinem Einsatz.

### Pilot 2

Die gemeinsame Elementgruppe (`SHARED_GROUP_ID`) liefert nur noch die Elemente des laufenden Einsatzes, ohne laufenden Einsatz die Elemente „ohne Einsatz“. So verschwindet ein Punkt, den der Pilot ohne Einsatz setzt, nicht beim nächsten Laden *(Entscheidung des Entwurfs)*. Wechselt der laufende Einsatz (Start, Abschluss), muss Pilot 2 seine Karte neu laden; wie das über `/pilot/ws` angestoßen wird, klärt der Plan gegen die DJI-Doku.

### Bedienung in Suchlage

- Einsatzliste (laufend, vorbereitet, abgeschlossen, „ohne Einsatz“); Anlegen, Starten, Abschließen, neuen Abschnitt anlegen für `operator` und `admin`.
- Lagekarte und Flugliste zeigen standardmäßig den laufenden Einsatz; ein anderer Einsatz lässt sich auswählen, um ihn vorzubereiten oder nachzusehen. Abgeschlossene Einsätze sind nur lesbar.
- Großbild (Kiosk) zeigt immer den laufenden Einsatz.
- Der Live-Zustand (SSE) enthält den laufenden Einsatz mit aktivem Abschnitt, damit alle Ansichten umschalten.
- Abschließen während eines laufenden Flugs wird abgelehnt (422); erst landen.
- Bei einem mit der Flugmappe verknüpften Einsatz kommen Abschnitte nur aus der Flugmappe; Suchlage bietet „Abschnitt anlegen“ dort nicht an (422) *(Entscheidung des Entwurfs)*.
- Abschließen geht in Suchlage immer, auch bei verknüpften Einsätzen. Das ist der Ausweg, wenn eine Flugmappe verloren gegangen ist.

## 5. Teil B: Kartensystem der Flugmappe

Die Flugmappe hat eine Karte: die Einsatzkarte je Abschnitt (`src/components/map/EinsatzMap.tsx`, `useGeomanDrawing.ts`, `mapExport.ts`) mit Zeichnen von Polygon, Kreis, Marker und Linie, Flächenberechnung und einem Bild für den PDF-Bericht.

- Leaflet, react-leaflet und Geoman werden durch MapLibre und terra-draw ersetzt, wie in Suchlage. Der Basiskartenstil (VersaTiles Shortbread, hell und dunkel) wird aus Suchlage übernommen, mit Herkunftskopf.
- Gleicher Funktionsumfang wie heute: die vier Zeichenwerkzeuge, Flächenberechnung, Bild für den Bericht, Foto-Variante.
- Bereits gespeicherte Einsatzkarten (GeoJSON in `localStorage`) werden weiter angezeigt und gehen in den Bericht.
- Offline-Fähigkeit bleibt: Der Service Worker cacht künftig Stil, Kacheln, Glyphen und Sprites von VersaTiles statt der OSM-Kacheln.
- Teil B ist eigenständig nützlich und hängt nicht an Suchlage.

## 6. Teil C: Verknüpfung

### Anmeldung und Rechte

- Die Flugmappe hat nach dem Pocket-ID-Login ein Zugriffstoken (heute für das Signatur-Backend). Sie schickt es als `Authorization: Bearer` an Suchlage.
- Suchlage prüft das Token gegen den Pocket-ID-Aussteller aus seinen OIDC-Einstellungen (Signatur über JWKS, `iss`, `exp`, `aud` = Client-ID der Flugmappe) und sucht das Konto über Aussteller und `sub`.
- Konto mit Rolle `operator` oder `admin` → erlaubt. Kein Konto (wer sich nie in Suchlage angemeldet hat) oder andere Rolle → 403. Ungültiges oder fehlendes Token → 401. Konten werden über diese Schnittstelle nie angelegt.
- In den Suchlage-Einstellungen (in der App, keine Env-Variable) gibt es einen Abschnitt „Flugmappe“: Client-ID der Flugmappe und erlaubter Ursprung (`https://uav.iuk-ue.de`) für CORS. Ist er leer, ist die Schnittstelle aus (503).
- Die Schnittstelle ist ein eigener, schmaler Bereich (z. B. `/api/flugmappe/v1/…`), getrennt von der sitzungsbasierten Browser-API.

### Schnittstelle (fachlich)

| Aufruf | Zweck |
|---|---|
| Wer bin ich | Probe: Name und Rolle. Die Flugmappe zeigt die Suchlage-Teile nur, wenn das klappt. |
| Offene Einsätze | vorbereitete und laufenden Einsatz zur Auswahl |
| Einsatz anlegen | mit Name, Ort, erstem Abschnitt und den IDs der Flugmappe; wird sofort *laufend* |
| Mit Einsatz verknüpfen | vorbereiteten Einsatz übernehmen, IDs der Flugmappe setzen, Einsatz wird *laufend*. Ein bereits laufender Einsatz ohne Flugmappe-Verknüpfung darf ebenfalls übernommen werden. |
| Abschnitt anlegen | bei einer Verlegung in der Flugmappe; wird aktiver Abschnitt |
| Abschnitt ändern | Bezeichnung und Ort nachreichen, meist den Ort, der erst nach der Verlegung feststeht (SUL-81) |
| Flüge des Einsatzes | je Flug: ID, Abschnitt, Start, Ende, Endegrund, Fluggerät (Seriennummer), maximale Höhe, Strecke, Zahl der Fotos |
| Karte des Einsatzes | GeoJSON: Kartenelemente und Flugspuren, optional je Abschnitt |
| Kartenelemente übernehmen | einmalig beim Verknüpfen: Zeichnungen und Standort aus der Flugmappe in den aktiven Abschnitt |
| Kartenfoto hochladen | Foto der Papierkarte für einen Abschnitt |
| Einsatz abschließen | beim Abschluss in der Flugmappe |

Konflikte: Ein anderer Einsatz läuft schon → 409. Einsatz ist bereits mit einem anderen Flugmappe-Einsatz verknüpft → 409. Einsatz ist abgeschlossen → 422.

**Übernehmen** *(Entscheidung des Entwurfs)*: Läuft bereits ein Einsatz, der mit einer anderen Flugmappe verknüpft ist (Gerätewechsel, Schichtwechsel, verlorene Browserdaten), zeigt die Flugmappe ihn mit Name, Start und wer ihn verknüpft hat und bietet „Übernehmen“ an. Nach ausdrücklicher Bestätigung ersetzt die neue Verknüpfung die alte; die alte Flugmappe bekommt danach keine neuen Flüge mehr. Ein anderer laufender Einsatz, der nicht passt, wird nicht aus der Flugmappe geschlossen, dafür ist Suchlage da.

**Abschluss nachholen** *(Entscheidung des Entwurfs)*: Läuft beim Abschluss in der Flugmappe noch ein Flug oder ist Suchlage nicht erreichbar, merkt sich die Flugmappe den Abschluss und holt ihn nach (nach der Landung bzw. wenn Suchlage wieder erreichbar ist), solange der Einsatz in der Flugmappe existiert. Der Truppführer muss nichts tun.

Die Vertragstypen entstehen wie üblich in `src/contract/`. Die Flugmappe übernimmt die erzeugten TypeScript-Dateien als Kopie mit Herkunftskopf.

### Flugmappe: Verknüpfen

- Die Suchlage-Teile erscheinen nur, wenn eine Suchlage-Adresse konfiguriert ist, der Nutzer angemeldet ist und die Probe gelingt. Sonst bleibt die Flugmappe wie heute, ohne Hinweis oder Fehler.
- In der Phase „Einsatzdaten“ gibt es einen Abschnitt „Suchlage“: offene Einsätze zur Auswahl oder „Neu in Suchlage anlegen“ (Name und Ort aus der Flugmappe vorbelegt). Der Truppführer wählt ausdrücklich; vorher passiert nichts.
- Die Verknüpfung wird am Einsatz der Flugmappe gespeichert. Eine Verlegung legt in Suchlage einen neuen Abschnitt an.
- Der Abschluss in der Nachbereitung schließt den Einsatz in Suchlage; geht das gerade nicht, holt die Flugmappe es nach (siehe „Abschluss nachholen“).

### Flugmappe: Flugbuch

- Bei verknüpftem Einsatz fragt die Flugmappe die Flüge regelmäßig ab, solange die Phase „Flüge“ offen ist (häufiger während eines Flugs).
- Ein von Suchlage erkannter Flug erscheint als **unbestätigter Eintrag** mit Zeiten und Werten aus Suchlage; Fernpilot und LRB sind wie heute vorbelegt. Ein laufender Flug ist als laufend erkennbar.
- Der Truppführer **bestätigt** den Eintrag (ein Tipp). Erst dann ist er ein normaler Flugbucheintrag mit Bezug auf den Suchlage-Flug. Landestatus und Bemerkung trägt er wie heute ein; Zeiten bleiben korrigierbar.
- Hat er schon selbst auf „Start“ getippt, wird nichts doppelt angelegt: Die Flugmappe schlägt vor, seinen Eintrag mit dem erkannten Flug zusammenzuführen, wenn die Startzeiten nahe beieinander liegen. Er bestätigt das; dann gelten die Zeiten aus der Telemetrie.
- Endet ein Flug mit Verbindungsabriss statt Landung, steht das als Hinweis am Eintrag; die Landezeit trägt der Truppführer dann selbst ein.
- Manuelles Erfassen bleibt jederzeit möglich, auch ohne Suchlage oder bei Netzausfall.

### Flugmappe: Einsatzkarte und Bericht

- Bei verknüpftem Einsatz zeigt die Einsatzkarte die Karte des Abschnitts aus Suchlage (Suchgebiete, Punkte, Flugspuren) nur zum Ansehen, mit einem Link „In Suchlage bearbeiten“. Gezeichnet wird nur in Suchlage.
- **Beim Verknüpfen** (von Ruben bestätigt): Was in der Einsatzkarte der Flugmappe schon gezeichnet ist, wird einmalig als Kartenelemente in den aktiven Abschnitt nach Suchlage übernommen. Der Standort wird zum Ort des Abschnitts und erscheint in Suchlage als Punkt. Die Schnittstelle braucht dafür einen Aufruf „Kartenelemente übernehmen“.
- **Bei Netzausfall** zeigt die Flugmappe den zuletzt geladenen Stand der Suchlage-Karte nur zum Ansehen; lokales Zeichnen gibt es bei verknüpften Einsätzen nicht.
- Die Foto-Variante bleibt; ein Foto geht zusätzlich als Kartenfoto des Abschnitts nach Suchlage.
- Der PDF-Bericht nimmt im Abschnitt Einsatzkarte das Bild dieser Suchlage-Karte (beim Erstellen frisch geladen) und, falls vorhanden, das Foto.
- Im Flugbuch des Berichts stehen nur bestätigte Flüge, ergänzt um Fluggerät, maximale Höhe und Strecke aus Suchlage. Gibt es in Suchlage Fotos oder Video zum Flug, nennt der Bericht das mit Verweis auf Suchlage.

## 7. Fehlerfälle

| Fall | Verhalten |
|---|---|
| Suchlage nicht erreichbar | Flugmappe zeigt dezent „Suchlage nicht erreichbar“, manuelles Flugbuch funktioniert; nach Wiederkehr werden Flüge nachgeladen |
| Token abgelaufen | stille Erneuerung wie heute; scheitert sie, Suchlage-Teile ruhen bis zur Neuanmeldung |
| Konto ohne Rechte | Suchlage-Teile erscheinen nicht |
| Verknüpfter Einsatz in Suchlage abgeschlossen oder gelöscht | Flugmappe zeigt es an, bestätigte Einträge bleiben, keine neuen Vorschläge |
| Flug startet, bevor in der Flugmappe verknüpft wurde | Flug gehört zu „ohne Einsatz“; wird er nachträglich in Suchlage zugeordnet, taucht er im Flugbuch als Vorschlag auf |
| Zwei Flugmappen verknüpfen denselben Einsatz | zweite bekommt 409 |

## 8. Sicherheit

- Die Schnittstelle akzeptiert nur Tokens des in Suchlage eingestellten Pocket-ID-Ausstellers mit der eingestellten Client-ID der Flugmappe.
- CORS nur für den eingestellten Ursprung, nur für den Flugmappe-Bereich.
- Rechte kommen ausschließlich aus Suchlage (Rolle des Kontos). Spätere Kontosperre (SUL-52) muss auch hier greifen.
- Das Kartenfoto wird wie andere Uploads geprüft (Typ, Größe).
- Das Repo der Flugmappe ist öffentlich: keine Suchlage-Adressen außer über Build-Konfiguration, keine Secrets.

## 9. Tests

- Suchlage: Zuordnungsregeln und Zustandsübergänge als reine Funktionen; Migration mit Bestandsdaten; Schnittstelle mit Test-IdP (vorhanden in `src/auth/oidc.rs`) für 401/403/409/422.
- Flugmappe: Zusammenführen manueller und erkannter Flüge, Bericht mit bestätigten und unbestätigten Flügen, Anzeige ohne Anmeldung.
- Feldabnahme als Checkliste: Übungseinsatz nach dem Erfolgskriterium aus Abschnitt 1.

## 10. Risiken

| Risiko | Umgang |
|---|---|
| Pocket-ID-Zugriffstokens sind keine prüfbaren JWTs oder tragen eine andere `aud` | Im Plan als erster Schritt prüfen. Rückfall: Suchlage prüft das Token über den Userinfo-Endpunkt. |
| Pilot 2 lädt die Kartenelemente beim Einsatzwechsel nicht neu | Im Simulator und im Feld prüfen; schlimmstenfalls Hinweis an den Piloten, die Karte neu zu öffnen |
| M4: Im FüKw ohne Internet erreicht die öffentliche Flugmappe den lokalen Suchlage-Server nicht | in SUL-47 klären (z. B. Suchlage-Adresse in der Flugmappe einstellbar) |
| MapLibre-Bild für den PDF-Bericht (Canvas-Export) | im Plan von Teil B früh prüfen |
| Zwei Repos, zwei Release-Zyklen | Schnittstelle versioniert (`v1`); Flugmappe prüft per Probe, ob Suchlage sie kann |

## 11. Offen für die Pläne

- genaue Pfade, Feldnamen und Abfrageintervalle
- wie nahe Startzeiten für das Zusammenführen liegen müssen
- Aussehen des Abschnitts „Suchlage“ in der Flugmappe und der Einsatzliste in Suchlage
- Umgang mit Kreisen aus Geoman beim Umstieg auf terra-draw
