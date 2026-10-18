# Freie Ports für die Test-Stacks (dev-smoke, e2e), damit sie neben `mise run dev` und fremden
# Containern laufen. Wird mit `. scripts/lib/ports.sh` eingebunden.

# Setzt und exportiert SL_*_PORT auf freie Ports, aufsteigend ab einem Zufallsstart in 20000–60000,
# und gibt sie einmal aus. Belegt heißt: TCP antwortet (nc -z) oder lsof zeigt einen UDP-Socket;
# nc -zu taugt dafür nicht (meldet jeden UDP-Port als offen), netstat zeigt unter macOS nichts.
# shortcut: ohne lsof (oder bei fremden Root-Sockets unter Linux) bleibt UDP ungeprüft; dann bricht
# `compose up` laut ab, und ein neuer Lauf würfelt neu.
pick_ports() {
  _port=$((($$ * 7919 + $(date +%s)) % 40000 + 20000))
  for _var in SL_SERVER_PORT SL_MQTT_PORT SL_RTMP_PORT SL_HLS_PORT SL_WEBRTC_PORT SL_WEBRTC_UDP_PORT \
    SL_PLAYBACK_PORT SL_S3_PORT SL_S3_ADMIN_PORT; do
    while nc -z 127.0.0.1 "$_port" 2>/dev/null || lsof -nP -iUDP:"$_port" >/dev/null 2>&1; do
      _port=$((_port + 1))
    done
    eval "$_var=$_port"
    export "$_var"
    _port=$((_port + 1))
  done
  echo "Ports: server $SL_SERVER_PORT, mqtt $SL_MQTT_PORT, rtmp $SL_RTMP_PORT, hls $SL_HLS_PORT," \
    "webrtc $SL_WEBRTC_PORT, webrtc-udp $SL_WEBRTC_UDP_PORT, playback $SL_PLAYBACK_PORT," \
    "s3 $SL_S3_PORT, s3-admin $SL_S3_ADMIN_PORT"
}

# Das Feature `dev` legt Broker, MediaMTX und Fotospeicher beim Start auf die festen Dev-Ports
# (src/auth/dev.rs). Hier zeigen sie auf die gewählten Ports, mit denselben Dev-Zugangsdaten
# aus deploy/compose.dev.yaml. Braucht die Sitzung des Dev-Logins im Cookie-Glas $1 unter $2.
use_ports() {
  put() {
    curl -sf -o /dev/null -b "$1" -X PUT -H 'content-type: application/json' -d "$4" "$2/api/settings/$3" ||
      { echo "FAIL Einstellung $3 nicht gesetzt"; return 1; }
  }
  put "$1" "$2" broker "{\"host\":\"127.0.0.1\",\"port\":$SL_MQTT_PORT,\"admin_username\":\"suchlage-admin\",\"admin_password\":\"dev-admin\",\"rc_url\":\"tcp://127.0.0.1:$SL_MQTT_PORT\"}" &&
    put "$1" "$2" media "{\"webrtc_url\":\"http://127.0.0.1:$SL_WEBRTC_PORT\",\"hls_url\":\"http://127.0.0.1:$SL_HLS_PORT\",\"playback_url\":\"http://127.0.0.1:$SL_PLAYBACK_PORT\",\"rtmp_url\":\"rtmp://127.0.0.1:$SL_RTMP_PORT\"}" &&
    put "$1" "$2" photo_storage "{\"s3_url\":\"http://127.0.0.1:$SL_S3_PORT\",\"rc_url\":\"http://127.0.0.1:$SL_S3_PORT\",\"admin_url\":\"http://127.0.0.1:$SL_S3_ADMIN_PORT\",\"bucket\":\"suchlage-fotos\",\"region\":\"garage\",\"access_key_id\":\"GK0123456789abcdef01234567\",\"secret_access_key\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"admin_token\":\"dev-fotos-admin\"}"
}
