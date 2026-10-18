#!/bin/sh
# Einmalig auf dem vServer: richtet Garage für die Fotos ein (M3) und gibt die Werte für Suchlage
# EINMAL aus. Danach in Suchlage unter Einstellungen → Fotospeicher eintragen (ADR 0002) und hier
# nirgends speichern. Braucht SUCHLAGE_BIND_IP und SUCHLAGE_PUBLIC_URL wie jeder Aufruf von
# compose.m1.yaml.
#
# 1. RPC-Geheimnis in /srv/suchlage/garage-secrets (neben deploy/, damit rsync --delete es nicht löscht)
# 2. Garage starten (`--single-node` legt das Layout selbst an)
# 3. Bucket, Server-Schlüssel (Lesen und Schreiben) und ein Admin-Token, der nur Schlüssel verwalten darf
set -eu
cd "$(dirname "$0")"
BUCKET=suchlage-fotos
SECRETS=../garage-secrets
DC="docker compose -f compose.m1.yaml"

g() { $DC exec -T fotos /garage "$@"; }

if [ ! -f "$SECRETS/rpc_secret" ]; then
  # umask vor dem Anlegen: Garage verweigert lesbare Geheimnisse.
  (umask 077 && mkdir -p "$SECRETS" && openssl rand -hex 32 > "$SECRETS/rpc_secret")
fi

$DC up -d fotos
i=0
until g status >/dev/null 2>&1; do
  i=$((i + 1)); [ $i -lt 30 ] || { echo "Garage antwortet nicht – docker compose -f compose.m1.yaml logs fotos" >&2; exit 1; }
  sleep 1
done

# Erst prüfen, dann anlegen: Ein zweiter Lauf soll keinen zweiten Schlüssel erzeugen.
if g bucket info "$BUCKET" >/dev/null 2>&1; then
  echo "Fotospeicher ist bereits eingerichtet – nichts geändert." >&2
  exit 1
fi

g bucket create "$BUCKET" >/dev/null
echo "== Server-Schlüssel (Key ID und Secret key in Suchlage eintragen) =="
g key create suchlage-server
g bucket allow --read --write "$BUCKET" --key suchlage-server >/dev/null
echo
echo "== Admin-Token (nur jetzt sichtbar, in Suchlage eintragen) =="
g admin-token create \
  --scope GetBucketInfo,CreateKey,AllowBucketKey,ListKeys,DeleteKey \
  suchlage-server
echo
echo "Weiter in Suchlage unter Einstellungen → Fotospeicher:"
echo "  S3-Adresse (Server):           http://fotos:3900"
echo "  S3-Adresse (Fernsteuerung):    http://${SUCHLAGE_BIND_IP:-<Tailscale-IP>}:3900"
echo "  Admin-Adresse:                 http://fotos:3903"
echo "  Bucket: $BUCKET   Region: garage"
