# ADR 0002: Einstellungen in der App, Env nur Bootstrap

- Status: angenommen (16.10.2026)
- Kontext: Grundsatz „keine Env-Konfiguration“. Lifeline liest ~40 Werte aus Env; das passt hier nicht.
- Entscheidung: Env trägt nur `SUCHLAGE_DATA_DIR`, `SUCHLAGE_BIND`, `SUCHLAGE_PUBLIC_URL`, `RUST_LOG`. OIDC, DJI-App-Daten, Broker- und MediaMTX-Zugang, Allgemeines liegen in der Tabelle `settings` und werden in der App gepflegt. Ohne OIDC-Einstellungen und ohne Benutzer erzeugt der Server beim Start einen Einrichtungstoken (24 h, einmal gültig) und schreibt `…/einrichtung#<token>` einmal ins Log. Der erste OIDC-Login wird Admin (abgelöst am 27.10.2026: Die Rolle kommt bei jedem Login aus zwei Pocket-ID-Gruppen, die in den OIDC-Einstellungen stehen; ohne eine davon kein Zugang).
- Ausnahme: das Broker-Admin-Passwort entsteht beim Einrichten des Brokers (`deploy/broker-init.sh`) und wird dann in der App eingetragen.
- Folgen: Die Datenbank enthält Geheimnisse (OIDC-Client-Secret, DJI-App-Key, MQTT-Passwörter der RCs). Volume-Rechte 0700. Keine Verschlüsselung at rest in der Anwendung; eine Sicherung ist mit `age` verschlüsselt, bevor sie auf die Platte kommt, und verlässt den Host nur so (ADR 0004, SUL-107).
- Dev: Feature `dev` setzt Broker- und MediaMTX-Werte für `deploy/compose.dev.yaml` und erlaubt den Login ohne OIDC.
