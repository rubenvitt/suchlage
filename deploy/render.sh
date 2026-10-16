#!/bin/sh
# Rendert Geheimnisse aus 1Password in ungetrackte Deploy-Dateien. Lokal ausführen, nie auf dem Server.
set -eu
cd "$(dirname "$0")"
: "${SUCHLAGE_BIND_IP:?Tailscale-IP des vServers, lokal 127.0.0.1}"
ITEM=op://DRK/suchlage-mqtt
TOOL_PASS=$(op read "$ITEM/suchlage")
RC_PASS=$(op read "$ITEM/rc-plus-01")

printf 'SUCHLAGE_BIND_IP=%s\nMQTT_TOOL_PASS=%s\n' "$SUCHLAGE_BIND_IP" "$TOOL_PASS" > .env
docker run --rm -v "$PWD/mosquitto:/m" eclipse-mosquitto:2.0.22 sh -c \
  "rm -f /m/passwd && mosquitto_passwd -b -c /m/passwd suchlage $TOOL_PASS && mosquitto_passwd -b /m/passwd rc-plus-01 $RC_PASS && chmod 0644 /m/passwd"
op inject -f -i mediamtx.tpl.yml -o mediamtx.yml >/dev/null
sed -i.bak "s/__BIND_IP__/$SUCHLAGE_BIND_IP/g" mediamtx.yml && rm -f mediamtx.yml.bak
