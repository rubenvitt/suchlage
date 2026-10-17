# M1 Realflug-Abnahme – Checkliste

Abnahme laut Spec §7: echter Flug, Position und Bild im FüKw und mobil sichtbar, Flug danach vollständig in der Liste, Verzögerung gemessen (Ziel ≤ 3 s über LTE).

- [ ] Pilot 2 öffnet den Kopplungslink, Seite meldet „Angemeldet als …“; Suchlage zeigt die Fernsteuerung. Im Protokoll der Seite steht `platformGetRemoteControllerSN: ok` erst nach `platformIsVerified: ok`.
- [ ] Fluggerät eingeschaltet: Marker, Startpunkt und Trupp-Zeichen auf der Karte; Instrumentenleiste ohne Schraffur.
- [ ] Einheiten plausibel: Höhe ü. Start, Geschwindigkeit, Abstand (Referenz Kap. 3 – im Feld prüfen; Abweichungen hier notieren).
- [ ] Start erkannt (Flug läuft), Livebild startet automatisch; sonst „Livebild starten“ und Fehlermeldung notieren.
- [ ] Linsenwechsel Weit/Zoom/Wärme – wechselt das Bild? (`live_lens_change` antwortet laut Forum teils nicht)
- [ ] Großbild im FüKw über Zugangslink: Bild und Position ohne Bedienung.
- [ ] Mobil außerhalb des VPN: Player wechselt nach ≤ 8 s auf „verzögert (HLS)“.
- [ ] Verzögerung: Stoppuhr vor die Kamera, Browser daneben filmen; 10 Messungen WebRTC und HLS (Median, Maximum).
- [ ] Funkabriss simulieren (Fernsteuerung abschirmen): Schraffur „vor N s“, danach neues Spursegment.
- [ ] Server-Neustart im Flug (`docker compose -f compose.m1.yaml restart server` in `/srv/suchlage/deploy`): derselbe Flug läuft weiter, Startpunkt und Livebild-Quellen sind sofort wieder da (`device_state`), „Livebild starten“ funktioniert ohne neues Einschalten.
- [ ] Landung erkannt; Flug in der Liste mit Spur und Video, Download funktioniert.
- [ ] Systemansicht: nicht ausgewertete Felder notieren; Rohmitschnitt `/data/records/*.jsonl` aus dem Volume `server-data` sichern, außerhalb des Repos (`docker compose -f compose.m1.yaml cp server:/data/records ./records-m1`).
- [ ] Mitschnitt bereinigen (Koordinaten verschieben, Seriennummern ersetzen) und als zweite Replay-Fixture vorschlagen.
