# ADR 0001: Live-Updates per SSE statt WebSocket

- Status: angenommen (16.10.2026, Plan M1)
- Kontext: Spec §3 nennt WebSocket. Der Datenfluss ist nur Server → Browser (Telemetrie 0,5–1 Hz); Befehle (Livebild starten, Linse) sind seltene Einzelaktionen.
- Entscheidung: `GET /api/live` als Server-Sent Events. Beim Verbinden zuerst ein `snapshot`, danach Einzelereignisse (`LiveEvent`). Bei Überlauf (Lagged) sendet der Server erneut einen `snapshot`. Befehle per `POST`.
- Kriterien: läuft ohne Upgrade-Sonderfall durch Traefik und Cloudflare; Browser verbindet sich selbst neu; Lifeline Hub nutzt dasselbe Muster.
- Verworfen: WebSocket (Heartbeat und Neuverbindung selbst bauen, Proxy-Upgrade), Polling (Verzögerung, Last).
- Folgen: Antwort nie komprimieren (Cloudflare puffert sonst). Kein Nachliefern per `Last-Event-ID` – der Snapshot ersetzt es.
