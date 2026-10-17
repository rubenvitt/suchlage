# Regeln für Suchlage

- Spec: `docs/superpowers/specs/2026-10-16-suchlage-design.md`, Plan M1: `docs/superpowers/plans/2026-10-16-m1-live-lage.md`.
- Qualitäts-Gate: `scripts/check.sh` muss vor jedem Commit grün sein (im Strang-Worktree `--rust` bzw. `--frontend`). Es formatiert Rust selbst; mit `CI=1` prüft es nur.
- Commits mit den Pfaden des Tasks, nie `git add frontend` oder `git add .`.
- Bausteingrenzen: Nur `src/dji/` kennt DJI-Typen. `src/situation/` ist rein (kein tokio/sqlx/axum/rumqttc/reqwest). `src/dji/` importiert nie `runtime` oder `web`. `src/contract/` hängt von nichts im Crate ab.
- Traits nur bei zwei echten Implementierungen. Testbarkeit über reine Funktionen und Aktor-Kanäle.
- Neuer Code englisch, Texte und Kommentare deutsch mit echten Umlauten. Von Lifeline übernommene Dateien behalten ihre Namen und tragen den Herkunftskopf.
- Statuscodes: 400 Feld, 422 Zusammenhang/Übergang, 409 Konflikt, 401 ohne Sitzung, 403 ohne Rolle, 503 Abhängigkeit fehlt.
- Vertrag ändern: Typ in `src/contract/` anpassen, `cargo test contract` erzeugt `frontend/src/contract/`, beides zusammen committen. Entfernte Typen: die erzeugte `.ts`-Datei von Hand löschen.
- Keine Env-Konfiguration außer `SUCHLAGE_DATA_DIR`, `SUCHLAGE_BIND`, `SUCHLAGE_PUBLIC_URL`, `RUST_LOG`.
- Dev-Login nur hinter dem Cargo-Feature `dev`.
- Mitschnitte und `data/` nie committen; Fixtures nur synthetisch.
