# Suchlage

Lokaler, internetunabhängiger Lagehub für Personensuchen im Katastrophenschutz: UAV-Livebild und Telemetrie, Suchsektoren, Abdeckungsnachweis, Hinweise und Export für die Einsatzleitung.

Nachfolger von `uav-lagebild` (archiviert). Stand: M1 abgenommen, M2 (Kartenelemente) umgesetzt, Feldabnahme offen.

Jeder Push auf `main` wird ein Release `v<Version>` mit Image in GHCR; welcher Stand läuft, zeigt die Systemseite (Runbook: `docs/betrieb/runbook-deployment.md`, Schritt 3).

## Entwickeln

    mise install          # Node 26.7.0, pnpm 11.10.0 (Rust 1.99.0 über rust-toolchain.toml)
    mise run ui           # nur die Oberfläche mit Beispieldaten, ohne Backend – http://127.0.0.1:5173
    mise run dev          # Broker, MediaMTX, Server, Oberfläche und Simulator – http://127.0.0.1:5173
    mise run check        # Qualitäts-Gate
    mise run dev:smoke    # Ende-zu-Ende mit echten Containern

Voraussetzungen: Docker, ffmpeg, jq. Kein 1Password nötig.
