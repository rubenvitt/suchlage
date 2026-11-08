#!/bin/sh
# Auf dem vServer, wenn die Datenbank oder eine Sicherung bekannt geworden ist (ADR 0004, Runbook
# „Geheimnisse erneuern“): baut die Zugänge des Brokers neu auf und gibt das neue Passwort von
# `suchlage-admin` EINMAL aus. Danach gilt kein bisheriger Zugang mehr, auch keiner, den jemand mit
# dem alten Admin-Passwort zusätzlich angelegt hat. Deshalb neu aufbauen statt nur
# `setClientPassword`: Das bräuchte das alte Passwort und ließe fremde Clients stehen.
#
# Vorher in Suchlage alle Fernsteuerungen löschen, danach neu koppeln: Bis dahin legt der Server
# die Zugänge einer noch eingetragenen Fernsteuerung beim nächsten Verbinden mit ihrem alten
# Passwort wieder an. Braucht SUCHLAGE_BIND_IP und SUCHLAGE_PUBLIC_URL wie jeder Aufruf von
# compose.m1.yaml.
set -eu
cd "$(dirname "$0")"
[ "${1:-}" = --ja ] || {
  echo "Aufruf: broker-erneuern.sh --ja (trennt alle MQTT-Verbindungen, Runbook „Geheimnisse erneuern“)" >&2
  exit 2
}
DC="docker compose -f compose.m1.yaml"

# Bricht ein Schritt ab, bleibt der Broker bewusst gestoppt: Ein Start mit der alten Datei ließe die
# alten Zugänge wieder gelten, ohne Datei startet er nicht (entrypoint.sh). Ein zweiter Lauf setzt
# an jeder Stelle wieder auf.
trap '[ $? -eq 0 ] || echo "Abgebrochen, der Broker bleibt gestoppt. Fehler beheben und broker-erneuern.sh --ja erneut ausführen." >&2' EXIT

# Gestoppt, damit der Broker die alte Datei nicht beim nächsten Befehl aus dem Speicher zurückschreibt.
$DC stop mqtt
# Das Volume trägt nur diese Datei (persistence false); broker-init.sh legt sie neu an.
$DC run --rm --no-deps -T --entrypoint rm mqtt -f /mosquitto/data/dynamic-security.json
./broker-init.sh
$DC up -d mqtt
