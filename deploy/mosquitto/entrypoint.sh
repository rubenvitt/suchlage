#!/bin/sh
# Legt die dynsec-Datei nur an, wenn DYNSEC_INIT_PASSWORD gesetzt ist (Dev). Im Betrieb legt
# deploy/broker-init.sh sie einmal an; fehlt sie, startet der Broker bewusst nicht.
set -eu
FILE=/mosquitto/data/dynamic-security.json
if [ ! -f "$FILE" ]; then
  if [ -n "${DYNSEC_INIT_PASSWORD:-}" ]; then
    mosquitto_ctrl dynsec init "$FILE" "${DYNSEC_INIT_USER:-suchlage-admin}" "$DYNSEC_INIT_PASSWORD" >/dev/null
  else
    echo "Broker nicht eingerichtet: deploy/broker-init.sh ausführen" >&2
    exit 1
  fi
fi
# Das Image-Entrypoint setzt die Rechte von /mosquitto (sonst gehört die dynsec-Datei root und
# Änderungen gehen beim nächsten Neustart verloren) und startet dann den Broker.
exec /docker-entrypoint.sh mosquitto -c /mosquitto/config/mosquitto.conf
