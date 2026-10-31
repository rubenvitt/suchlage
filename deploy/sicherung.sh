# Gemeinsame Einstellungen von backup.sh und restore.sh (SUL-107). Wird nur eingebunden.
#
# Ohne Anpassung gilt das Layout aus dem Runbook: /srv/suchlage mit deploy/compose.m1.yaml und
# Docker-Volumes suchlage-m1_*. Ein Host mit anderem Layout (etwa Bind-Mounts unter /data/suchlage)
# legt neben deploy/ eine Datei sicherung.env an, die diese Werte überschreibt:
#   SUCHLAGE_COMPOSE      Compose-Datei, absolut
#   SUCHLAGE_SERVER_DATA, SUCHLAGE_FOTOS_META, SUCHLAGE_FOTOS_DATA, SUCHLAGE_VIDEO
#                         Quelle je Archiv: Volume-Name oder absoluter Pfad
# Die Datei liegt außerhalb von deploy/, damit `rsync --delete` sie nicht löscht.
ROOT=$(cd "$(dirname "$0")/.." && pwd)
[ ! -f "$ROOT/sicherung.env" ] || . "$ROOT/sicherung.env"
DC="docker compose -f ${SUCHLAGE_COMPOSE:-$ROOT/deploy/compose.m1.yaml}"
IMAGE=busybox:1.37.0-uclibc

# $1 Name des Archivs → Quelle für `docker run -v`
quelle() {
  case "$1" in
    server-data) echo "${SUCHLAGE_SERVER_DATA:-suchlage-m1_server-data}" ;;
    fotos-meta) echo "${SUCHLAGE_FOTOS_META:-suchlage-m1_fotos-meta}" ;;
    fotos-data) echo "${SUCHLAGE_FOTOS_DATA:-suchlage-m1_fotos-data}" ;;
    video) echo "${SUCHLAGE_VIDEO:-suchlage-m1_video}" ;;
    garage-secrets) echo "$ROOT/garage-secrets" ;;
    *) return 1 ;;
  esac
}

# Docker legt ein fehlendes Volume oder Verzeichnis beim Einhängen leer an. Ein leeres Archiv
# sähe gültig aus und leerte bei der Wiederherstellung das echte Volume.
quelle_da() {
  case "$1" in
    /*) [ -d "$1" ] ;;
    *) docker volume inspect "$1" >/dev/null 2>&1 ;;
  esac
}

# video-init gibt die Videoablage dem Server-Nutzer. Nicht jeder Host hat den Dienst.
hat_video_init() {
  $DC config --services | grep -qx video-init
}
