#!/bin/sh
# Bestätigt die Geräte-Topologie (update_topo), die Pilot 2 nach dem MQTT-Login meldet.
set -u
mosquitto_sub -h mqtt -u suchlage -P "$MQTT_TOOL_PASS" -t 'sys/product/+/status' -F '%j' |
while IFS= read -r line; do
  # %j liefert topic + payload als String; ungültiges JSON im Payload wird übersprungen
  reply=$(printf '%s' "$line" | jq -c '
    .topic as $t | (.payload | fromjson? // empty)
    | select(.method == "update_topo")
    | {topic: ($t + "_reply"), msg: {bid, tid, timestamp: (now * 1000 | floor), method, data: {result: 0}}}' 2>/dev/null) || continue
  [ -n "$reply" ] || continue
  printf '%s' "$reply" | jq -c .msg |
    mosquitto_pub -h mqtt -u suchlage -P "$MQTT_TOOL_PASS" -t "$(printf '%s' "$reply" | jq -r .topic)" -s
done
