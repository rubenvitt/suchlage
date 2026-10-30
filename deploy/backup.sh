#!/usr/bin/env bash
# Auf dem vServer: sichert Suchlage verschlüsselt nach /srv/suchlage/backup (ADR 0004, SUL-107).
# Aufruf: ssh <host> /srv/suchlage/deploy/backup.sh [--video]
#
# Server und Fotospeicher stehen dafür kurz still, damit beide SQLite-Datenbanken (Suchlage und
# die Metadaten von Garage) konsistent sind. Nicht während eines Flugs. Mit --video kommen die
# Flugvideos dazu, dann steht auch `media` still.
#
# Jedes Archiv geht als Datenstrom aus `tar` direkt in `age` und liegt nie unverschlüsselt auf der
# Platte. Verschlüsselt wird an die öffentlichen Schlüssel in /srv/suchlage/backup-empfaenger.txt
# (neben deploy/, damit rsync --delete sie nicht löscht). Den privaten Schlüssel gibt es nur im
# Passwortmanager; Entschlüsseln geht deshalb nur dort, wo er ist, nie auf dem Host.
#
# bash statt sh wegen pipefail: Ein Fehler von `tar` darf nicht hinter einem erfolgreichen `age`
# verschwinden.
set -euo pipefail
cd "$(dirname "$0")"
ROOT=$(cd .. && pwd)
RECIPIENTS=$ROOT/backup-empfaenger.txt
TARGET=$ROOT/backup
DC="docker compose -f compose.m1.yaml"
IMAGE=busybox:1.37.0-uclibc

volumes="server-data fotos-meta fotos-data"
services="server fotos"
case "${1:-}" in
  "") ;;
  --video) volumes="$volumes video"; services="$services media" ;;
  *) echo "Aufruf: backup.sh [--video]" >&2; exit 2 ;;
esac

command -v age >/dev/null || { echo "age fehlt am Host (apt install age)" >&2; exit 1; }
grep -q '^age1' "$RECIPIENTS" 2>/dev/null ||
  { echo "Kein öffentlicher Schlüssel in $RECIPIENTS, siehe Runbook „Sicherung“" >&2; exit 1; }
[ -d "$TARGET" ] && [ -w "$TARGET" ] ||
  { echo "$TARGET fehlt oder ist nicht beschreibbar, siehe Runbook „Sicherung“" >&2; exit 1; }

# Docker legt ein fehlendes Volume oder Verzeichnis beim Einhängen leer an. Ein leeres Archiv
# sähe gültig aus und leerte bei der Wiederherstellung das echte Volume.
for v in $volumes; do
  docker volume inspect "suchlage-m1_$v" >/dev/null 2>&1 ||
    { echo "Volume suchlage-m1_$v fehlt, nichts gesichert" >&2; exit 1; }
done
[ -f "$ROOT/garage-secrets/rpc_secret" ] ||
  { echo "$ROOT/garage-secrets/rpc_secret fehlt, nichts gesichert" >&2; exit 1; }

# Ein zweiter Lauf startete sonst die Dienste, während der erste noch packt.
exec 9>"$TARGET/.lauf.sperre"
flock -n 9 || { echo "Es läuft schon eine Sicherung" >&2; exit 1; }

# Alle Archive eines Laufs tragen denselben Stempel und gehören zusammen.
stamp=$(date +%Y%m%dT%H%M%S)
part=""
stopped=""
cleanup() {
  [ -z "$part" ] || rm -f "$part"
  # Auch nach einem Fehler wieder starten: Eine gescheiterte Sicherung darf den Stack nicht
  # stehen lassen.
  [ -z "$stopped" ] || $DC start $stopped
}
trap cleanup EXIT

# $1 Name des Archivs, $2 Quelle für `docker run -v` (Volume oder absoluter Pfad)
archive() {
  local out=$TARGET/$1-$stamp.tgz.age
  part=$out.part
  docker run --rm -v "$2":/d:ro "$IMAGE" tar czf - -C /d . | (umask 077 && age -R "$RECIPIENTS" -o "$part")
  mv "$part" "$out"
  part=""
  echo "$out"
}

# Vor dem Stoppen merken: Bricht `stop` mittendrin ab, startet cleanup die schon gestoppten wieder.
stopped=$services
$DC stop $services
for v in $volumes; do
  archive "$v" "suchlage-m1_$v"
done
archive garage-secrets "$ROOT/garage-secrets"

# Unverschlüsselte Archive aus der Zeit vor SUL-107 dürfen hier nicht liegen bleiben.
old=$(find "$TARGET" -maxdepth 1 -name '*.tgz' -print)
if [ -n "$old" ]; then
  echo "Unverschlüsselte Archive liegen noch in $TARGET, nach geprüfter Wiederherstellung löschen:" >&2
  echo "$old" >&2
fi
