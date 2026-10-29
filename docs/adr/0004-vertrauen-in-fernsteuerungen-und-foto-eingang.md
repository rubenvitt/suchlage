# ADR 0004: Vertrauen in gekoppelte Fernsteuerungen, Eingang für Fotos

- Status: angenommen (29.10.2026)
- Anlass: Security-Review vom 27.10.2026. Mehrere Findings setzen eine bösartige gekoppelte Fernsteuerung voraus.
- Vertrauensmodell: Eine gekoppelte Fernsteuerung ist ein Gerät des DRK, gekoppelt von einem Admin. Suchlage vertraut ihren Meldungen. Absichtliche Täuschung durch ein gekoppeltes Gerät wird nicht abgewehrt. Das ist bewusst so.
  - Was die Fernsteuerung meldet, gilt: Topologie (`update_topo`), das Feld `gateway` in der Telemetrie und Name und Ablageort im Upload-Callback. Wer etwas publiziert, sieht Suchlage ohnehin nicht (ADR 0003).
  - Ein Gerät kann sich mit einer Meldung gegenüber einem anderen gekoppelten Gerät ausgeben. Das geht über `thing/product/+/osd|state` oder einen Unterknoten in `update_topo`. Akzeptiertes Risiko.
  - Ebenso akzeptiert:
    - viele Fotos zu einer Auslösung (die Arbeit wächst mit der Gruppe);
    - Wettläufe zwischen erneutem Koppeln und Widerruf;
    - das Zeitfenster zwischen der Bereichsprüfung einer Pilot-Kartenänderung und dem Schreiben.

    Widerrufen heißt: Gerät zurückholen und Kopplung löschen.
  - Was trotzdem begrenzt bleibt, weil es auch ein fehlerhaftes Gerät oder eine fehlerhafte Firmware treffen kann: Zahl und Länge unbekannter Feldnamen (`situation/state.rs`), Größe des gemergten Gerätestands (`dji/translate.rs`). Außerdem zeigt der Browser Originale nur als JPEG oder PNG, mit `nosniff` und `sandbox` (`web/photos.rs`).
- Fotos: Garage (v2.4.1, geprüft im Quellcode `src/api/s3/router.rs`, `src/model/permission.rs`) kennt Rechte nur je Schlüssel und Bucket: `read`, `write`, `owner`. Es gibt keine Präfixe und keine Bucket-Policies. `write` umfasst `PutObject`, `DeleteObject(s)` und sogar `PutBucketLifecycleConfiguration`. Ein Upload-Schlüssel auf `suchlage-fotos` könnte damit jedes Original löschen oder überschreiben, auch das einer anderen Fernsteuerung.
- Entscheidung: Fernsteuerungen bekommen per `sts` nur `write` auf den Eingangs-Bucket `<bucket>-eingang`. Den Namen bildet der Server aus dem Bucket der Einstellungen (`storage::inbox_bucket`). Beim Upload-Callback kopiert der Server das Foto mit `CopyObject` unter demselben Schlüssel nach `suchlage-fotos` und löscht es dann im Eingang. Ohne SSE-C verweist Garage beim Kopieren auf dieselben Blöcke, es entstehen keine neuen Daten. Ein wiederholter Callback findet das Foto schon im Fotospeicher.
- Verworfen: ein eigener Bucket je Fernsteuerung (der Admin-Token müsste Buckets anlegen dürfen, und der Gewinn wäre im Vertrauensmodell klein) und ein Upload-Gateway im Server (eigene S3-Signaturprüfung für Uploads bis 64 MB).
- Folgen:
  - `fotos-init.sh` legt den Eingang an. Auf einem schon eingerichteten Server legt ein erneuter Lauf nur diesen an.
  - Der Server-Schlüssel braucht `read` und `write` auf beide Buckets.
  - Im Eingang kann eine Fernsteuerung ein noch nicht übernommenes Foto einer anderen überschreiben. Das ist ein Fenster von Sekunden und im Vertrauensmodell akzeptiert.
  - shortcut: Fotos, zu denen nie ein Callback kommt, bleiben im Eingang liegen. Wenn das in der Praxis vorkommt, eine Lifecycle-Regel (`Expiration`, von Garage unterstützt) für den Eingang setzen.
