#!/bin/sh
# Spike-Werkzeug: Livestream-Befehle an Pilot 2.
#   live.sh start <gateway_sn> <video_id> <rtmp_url>
#   live.sh stop  <gateway_sn> <video_id>
#   live.sh lens  <gateway_sn> <video_id> <normal|thermal|wide|zoom>
set -eu
cmd=$1 gw=$2 vid=$3
case $cmd in
  start) method=live_start_push
         data=$(jq -nc --arg u "$4" --arg v "$vid" '{url_type: 1, url: $u, video_id: $v, video_quality: 0}') ;;
  stop)  method=live_stop_push
         data=$(jq -nc --arg v "$vid" '{video_id: $v}') ;;
  lens)  method=live_lens_change
         data=$(jq -nc --arg v "$vid" --arg t "$4" '{video_id: $v, video_type: $t}') ;;
  *) echo "Unbekannter Befehl: $cmd" >&2; exit 2 ;;
esac
jq -nc --arg m "$method" --arg b "$(cat /proc/sys/kernel/random/uuid)" --arg t "$(cat /proc/sys/kernel/random/uuid)" \
  --argjson d "$data" '{bid: $b, tid: $t, timestamp: (now * 1000 | floor), method: $m, data: $d}' |
  mosquitto_pub -h mqtt -u suchlage -P "$MQTT_TOOL_PASS" -t "thing/product/$gw/services" -s
echo "gesendet: $method an $gw – Antwort im Mitschnitt unter thing/product/$gw/services_reply"
