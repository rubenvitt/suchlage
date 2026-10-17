# Abweichungen Minor-Funde Frontend (Abschluss-Review M1)

Behebt die Minor-Funde aus dem Abschluss-Review von M1, Strang Frontend. Jeder Fund wurde vorher am
Code geprüft. Alle trafen zu, keiner war schon behoben.

## Großbild-Leiste bei 1280×720 (`views/views.css`)

- **Messung vorher:** `scrollWidth == clientWidth` galt schon vor der Änderung, für die Seite und für
  jede Zelle. `overflow-wrap: anywhere` bricht Ziffern um, statt sie überlaufen zu lassen. Gemessen
  wurde deshalb zusätzlich die Zeilenzahl je Wert. Ergebnis bei 1280×720: Flugzeit „10:0/7“ in zwei
  Zeilen, „manueller Flug“ in drei. „Rückkehr zum Startpunkt“ brauchte fünf Zeilen, das Alter beim
  Verlust drei.
- **Fix:** Unter 1900 px hat die Leiste zwei Zeilen zu fünf Spuren. Oben stehen die fünf Zahlen,
  unten Modus (zwei Spuren), Flugzeit und Alter (zwei Spuren). Ab 1900 px bleibt die eine Zeile
  aus F8.
- **Abweichung vom Vorschlag:** Der Review schlug eine Grenze bei 1700 px und darunter
  `auto-fit minmax(200px, 1fr)` vor. Beides wurde verworfen:
  - Mit `auto-fit` bekäme der Modus nur eine Spur, „manueller Flug“ bräche weiter um.
  - Bei 1700 px läuft der Modus in der einen Zeile über drei Zeilen (gemessen). Die Grenze liegt
    deshalb bei 1900 px.
- **Messung nachher**, Vorschau-Modus, Chromium:
  - Gemessen bei 1280×720 und 1920×1080, zusätzlich bei 1700×900 und 1899×1000.
  - Überall gilt `scrollWidth == clientWidth` und `scrollHeight == clientHeight`.
  - Jede Zahl steht in einer Zeile. Modus und Alter brauchen höchstens zwei Zeilen, auch mit den
    längsten Texten. Die Texte wurden dafür per DOM eingesetzt: „Rückkehr zum Startpunkt“,
    „Verbindung verloren 14:31:02“, „10,0 m/s“, „1,25 km“, „-120 m“ und „54:59“.
  - Die Leiste springt dabei nicht, die Höhe bleibt bei 242 px bzw. 121 px.
- **Preis:** Bei 1280×720 ist die Leiste 242 px hoch statt 165 px wie vorher mit umgebrochenen
  Zahlen. Der Karte bleiben 425 px statt 502 px.

## Mauszeiger im Großbild (`app/frame.css`)

maplibre-gl.css setzt `grab`/`grabbing` über der Karte und `pointer` an der Quellenangabe, mit bis
zu drei Klassen. Die neue Regel hat vier Klassen und gilt deshalb unabhängig von der Ladereihenfolge.
`interactive: false` wurde nicht gewählt: Dafür bräuchte `SituationMap` eine weitere Option.
Gemessen über ein 40-px-Raster der ganzen Fläche bei 1280×720 und 1920×1080: überall `cursor: none`.
Vorher war es auf 41 % (1280×720) bzw. 49 % (1920×1080) der Fläche `grab`.

## Bedienhilfen der Karte (`map/SituationMap.tsx`)

`locale` setzt die drei Texte, die ohne eigene MapLibre-Steuerelemente vorkommen:

- `Map.Title`
- `AttributionControl.ToggleAttribution`
- `AttributionControl.MapFeedback`

`Map.Title` heißt „Karte“ statt des vorgeschlagenen „Lagekarte“. Die Zeichenfläche steht in der
Region „Lagekarte“ (bzw. dem `label` der Ansicht), zwei gleichnamige verschachtelte Regionen wären
doppelt. ARIA-Snapshot am Arbeitsplatz: `region "Lagekarte"` → `region "Karte"`. Der
Quellenknopf heißt „Quellenangabe ein- oder ausblenden“.

## WebRTC nach einem Bild (`video/playerMachine.ts`)

Zwei `failed` in Folge wechseln nur noch dann auf HLS, wenn `hadPicture` nicht gesetzt ist. Das
heißt: WebRTC hat in diesem Stream noch kein Bild geliefert. `hadPicture` mit Transport `webrtc` kann
nur aus einem laufenden WebRTC-Bild entstehen, denn aus HLS geht es nicht zurück.

- Nach einem Bild versucht der Player es alle 3 s weiter über WebRTC. `ice_timeout` wechselt wie
  bisher auf HLS. Ohne UDP (außerhalb des VPN) greift so weiter die 8-s-Frist.
- `attempt` zählt dabei weiter hoch, keine Anzeige liest den Wert.
- Der Playwright-Smoke „Rückfall auf HLS“ antwortet ab dem ersten Versuch mit WHEP 500, ohne Bild
  davor. Er ist davon nicht betroffen. Er läuft nicht im Gate und wurde hier nicht ausgeführt.

## Späte WHEP-Antwort (`video/whep.ts`, `video/usePlayer.ts`)

`startWhep` bekommt ein `AbortSignal` als Pflichtparameter. `usePlayer` legt je Effekt einen
`AbortController` an und bricht ihn im Cleanup ab. Der Abbruch wirkt so:

- Er schließt die Peer-Verbindung. Das verhindert im Browser ein spätes `ontrack`, und eine laufende
  Aushandlung scheitert.
- Er bricht den `fetch` ab.
- `ontrack` prüft zusätzlich `signal.aborted`.

`close()` setzt `srcObject` bewusst nicht zurück. Das letzte Bild soll unter der Schraffur stehen
bleiben (Commit 7625e57). Derselbe Fehler betraf auch den Wechsel der Fernsteuerung: Ein spätes
`ontrack` der alten Fernsteuerung hätte deren Kamera gezeigt.

Neue Tests:

- `whep.test.ts`: Eine Antwort nach dem Abbruch weist der Aufruf ab, die Verbindung ist zu, und ein
  danach gefeuertes `ontrack` setzt nichts. Ohne den Fix schlägt der Test fehl, das wurde geprüft.
  Ein Gegenstück zeigt, dass `ontrack` ohne Abbruch das Bild setzt.
- `VideoPanel.test.tsx`: Nach der ICE-Frist ist das Signal der offenen Aushandlung abgebrochen.

## Systemansicht und Datumsformat (`admin/*`)

- Der Live-Strom zeigt bei `open` jetzt „verbunden“ statt „steht“, wie der Broker. Der Test fragt
  beide Chips je Datenfeld ab, weil „verbunden“ jetzt zweimal vorkommt.
- Fernsteuerungen und Großbild-Zugänge zeigen Zeitpunkte als „16.10.2026, 12:26“, wie die Flugliste.
  Das ersetzt die F9-Entscheidung „15.10.26, 13:26“ (dort gegen Sekunden gewählt, nicht gegen das
  vierstellige Jahr).

## Flugliste nach Neuverbinden (`live/LiveProvider.tsx`)

Ein `snapshot` invalidiert jetzt wie `flight_started`/`flight_ended` den Schlüssel `['flights']`.
Weil der Schlüssel als Präfix gilt, lädt damit auch jedes Flugdetail (`['flights', id]`) neu.

- Beim ersten Snapshot nach dem Seitenaufruf gibt es dadurch einen zusätzlichen Abruf der Flugliste.
  Das ist gewollt, der Abruf ist billig.
- Der bestehende Test zählt jetzt 1 → 2 (Snapshot) → 2 (`aircraft`) → 3 (`flight_ended`).
- Neuer Test: Nach `onerror` und erneutem Snapshot laden Flugliste und Flugdetail je einmal neu.

## Einrichtung: Pocket-ID-Gruppe (`admin/SetupPage.tsx`)

Neu ist eine Warnung (`Alert type="warning"`) direkt unter der Callback-URL, vor dem Formular:
„Client auf die I&K-Gruppe beschränken“. Der Text verweist auf „Allowed User Groups“ in Pocket ID.
Der Grund: `store::users::upsert_oidc` macht das erste Konto zum Admin und jedes weitere zum Operator.
Suchlage selbst gibt Konten nicht frei. Die Warnung ist die einzige `Alert` mit `showIcon`, weil sie
in der Karte neben Text und Formular auffallen muss. Der Test prüft, dass sie da ist.

## Screenshots

Im Scratchpad unter `m1-ui-review/minors/`, Vorschau-Modus, dunkel:

- Großbild bei 1280×720 vorher und nachher, bei 1920×1080 nachher
- dieselben Größen mit den eingesetzten langen Texten (`…-dom-injiziert`)
- System, Fernsteuerungen und Großbild-Zugänge bei 1440 px
- Einrichtung bei 1440 px und 390 px
