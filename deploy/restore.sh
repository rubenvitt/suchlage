#!/usr/bin/env bash
# Auf dem vServer: spielt ein entschlüsseltes Archiv aus deploy/backup.sh zurück (ADR 0004, SUL-107).
# Aufruf vom eigenen Rechner, der private Schlüssel bleibt dort:
#   age -d -i suchlage-backup.key server-data-<Stempel>.tgz.age | ssh <host> /srv/suchlage/deploy/restore.sh server-data
#
# Packt erst neben den alten Inhalt aus und tauscht erst, wenn `tar` fertig ist. Bricht der Strom ab
# (falscher Schlüssel, Netz, volle Platte), bleibt das Ziel unverändert. Braucht dafür kurz den
# doppelten Platz im Volume. Läuft nur, solange Server, Fotospeicher und MediaMTX gestoppt sind.
set -euo pipefail
cd "$(dirname "$0")"
ROOT=$(cd .. && pwd)
DC="docker compose -f compose.m1.yaml"
IMAGE=busybox:1.37.0-uclibc

case "${1:-}" in
  server-data|fotos-meta|fotos-data|video) src=suchlage-m1_$1 ;;
  garage-secrets) src=$ROOT/garage-secrets ;;
  *) echo "Aufruf: restore.sh server-data|fotos-meta|fotos-data|video|garage-secrets < archiv.tgz" >&2; exit 2 ;;
esac

if [ -n "$($DC ps -q --status running server fotos media)" ]; then
  echo "Erst stoppen: $DC stop server fotos media" >&2
  exit 1
fi

docker run --rm -i -v "$src":/d "$IMAGE" sh -c '
  set -e
  neu=/d/.wiederherstellung
  trap "rm -rf $neu" EXIT
  rm -rf $neu && mkdir $neu
  tar xzf - -C $neu
  find /d -mindepth 1 -maxdepth 1 ! -name .wiederherstellung -exec rm -rf {} +
  find $neu -mindepth 1 -maxdepth 1 -exec mv {} /d/ \;
'
echo "$1 zurückgespielt"
