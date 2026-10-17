#!/bin/sh
# Einmalig auf dem vServer: legt den Broker-Admin an und gibt das Passwort EINMAL aus.
# Danach in Suchlage unter Einstellungen → Broker eintragen (ADR 0002) und hier nirgends speichern.
# Braucht SUCHLAGE_BIND_IP und SUCHLAGE_PUBLIC_URL wie jeder Aufruf von compose.m1.yaml.
set -eu
cd "$(dirname "$0")"
FILE=/mosquitto/data/dynamic-security.json
# Ausdrücklich exportiert: Ob eine Zuweisung vor einem Funktionsaufruf in die Umgebung der
# Kommandos darin gelangt, lässt POSIX offen.
DYNSEC_PASS=''
export DYNSEC_PASS

# Einmal-Container auf dem Broker-Volume; -T, damit die Ausgabe ohne Terminal-Zeilenenden kommt.
broker() {
  docker compose -f compose.m1.yaml run --rm --no-deps -T -e DYNSEC_PASS --entrypoint sh mqtt -c "$1"
}

# Erst prüfen, dann anlegen: Ein Fehler von Docker oder Compose bricht hier ab (set -e) und
# erscheint nicht als „bereits eingerichtet“.
state=$(broker "if [ -f $FILE ]; then echo vorhanden; else echo frei; fi")
case "$state" in
  frei) ;;
  vorhanden) echo "Broker ist bereits eingerichtet – nichts geändert." >&2; exit 1 ;;
  *) echo "Unerwartete Antwort beim Prüfen des Brokers: $state" >&2; exit 1 ;;
esac

DYNSEC_PASS=$(openssl rand -hex 24)
# Das Passwort geht über die Umgebung in den Container und steht so nicht in der Kommandozeile von
# docker compose; nur mosquitto_ctrl im Container bekommt es kurz als Argument.
broker "mosquitto_ctrl dynsec init $FILE suchlage-admin \"\$DYNSEC_PASS\" >/dev/null"
echo "Broker-Admin: suchlage-admin"
echo "Passwort (nur jetzt sichtbar, in Suchlage eintragen): $DYNSEC_PASS"
