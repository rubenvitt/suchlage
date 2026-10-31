#!/usr/bin/env bash
# Auf dem vServer: spielt ein entschlüsseltes Archiv aus deploy/backup.sh zurück (ADR 0004, SUL-107).
# Aufruf vom eigenen Rechner, der private Schlüssel bleibt dort:
#   ssh <host> /srv/suchlage/deploy/restore.sh stoppen
#   age -d -i suchlage-backup.key server-data-<Stempel>.tgz.age | ssh <host> /srv/suchlage/deploy/restore.sh server-data
#   ssh <host> /srv/suchlage/deploy/restore.sh starten
#
# Packt erst neben den alten Inhalt aus und tauscht erst, wenn `tar` fertig ist. Bricht der Strom ab
# (falscher Schlüssel, Netz, volle Platte), bleibt das Ziel unverändert. Braucht dafür kurz den
# doppelten Platz im Volume. Läuft nur, solange Server, Fotospeicher und MediaMTX gestoppt sind.
set -euo pipefail
. "$(dirname "$0")/sicherung.sh"

dienste="server fotos media"
case "${1:-}" in
  stoppen) $DC stop $dienste; exit ;;
  starten)
    # `start` wiederholt das schon beendete video-init nicht.
    if hat_video_init; then $DC run --rm video-init; fi
    $DC start media fotos server
    exit ;;
esac
src=$(quelle "${1:-}") ||
  { echo "Aufruf: restore.sh stoppen|starten|server-data|fotos-meta|fotos-data|video|garage-secrets < archiv.tgz" >&2; exit 2; }

# Dieselbe Sperre wie backup.sh: Eine Sicherung startete sonst die Dienste mitten im Zurückspielen.
exec 9>"$ROOT/backup/.lauf.sperre"
flock -n 9 || { echo "Es läuft schon eine Sicherung oder Wiederherstellung" >&2; exit 1; }

# Eigene Zuweisung, damit set -e greift, wenn Compose selbst scheitert (etwa ohne .env).
# video-init läuft vor `media` und schreibt per chown -R ins Video-Volume.
if hat_video_init; then dienste="$dienste video-init"; fi
running=$($DC ps -q --status running --status restarting --status paused $dienste)
if [ -n "$running" ]; then
  echo "Erst stoppen: restore.sh stoppen" >&2
  exit 1
fi

docker run --rm -i -v "$src":/d "$IMAGE" sh -c '
  set -e
  neu=/d/.wiederherstellung
  trap "rm -rf $neu" EXIT
  rm -rf $neu && mkdir $neu
  tar xzf - -C $neu
  # Ab hier bleibt der ausgepackte Stand liegen, falls der Tausch scheitert.
  trap - EXIT
  find /d -mindepth 1 -maxdepth 1 ! -name .wiederherstellung -exec rm -rf {} +
  for f in $neu/* $neu/.[!.]* $neu/..?*; do
    [ -e "$f" ] || [ -L "$f" ] || continue
    mv "$f" /d/
  done
  # Rechte und Besitzer der Wurzel stehen im Archiv als ".", also an $neu.
  chown "$(stat -c %u:%g $neu)" /d
  chmod "$(stat -c %a $neu)" /d
  rmdir $neu
'
echo "$1 zurückgespielt"
