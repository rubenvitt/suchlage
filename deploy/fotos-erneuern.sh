#!/bin/sh
# Auf dem vServer, wenn die Datenbank oder eine Sicherung bekannt geworden ist (ADR 0004, Runbook
# „Geheimnisse erneuern“): erneuert alle Geheimnisse des Fotospeichers (Garage) und gibt die neuen
# Werte für Suchlage EINMAL aus.
#
# 1. RPC-Geheimnis in /srv/suchlage/garage-secrets (liegt in jeder Sicherung), Garage neu starten
# 2. ALLE S3-Schlüssel und Admin-Tokens löschen: den Server-Schlüssel, die Upload-Schlüssel der
#    Fernsteuerungen und alles, was jemand mit dem alten Admin-Token zusätzlich angelegt hat
# 3. Server-Schlüssel und Admin-Token neu anlegen wie fotos-init.sh
#
# Buckets und Fotos bleiben. Bis die neuen Werte unter Einstellungen → Fotospeicher stehen, nimmt
# Suchlage keine Fotos an und zeigt keine. Nicht während eines Flugs: laufende Uploads brechen ab.
# Braucht SUCHLAGE_BIND_IP und SUCHLAGE_PUBLIC_URL wie jeder Aufruf von compose.m1.yaml.
set -eu
cd "$(dirname "$0")"
[ "${1:-}" = --ja ] || {
  echo "Aufruf: fotos-erneuern.sh --ja (löscht alle Schlüssel des Fotospeichers, Runbook „Geheimnisse erneuern“)" >&2
  exit 2
}
BUCKET=suchlage-fotos
# Name fest aus dem Bucket abgeleitet, siehe src/storage/mod.rs (inbox_bucket).
INBOX=$BUCKET-eingang
SECRETS=../garage-secrets
DC="docker compose -f compose.m1.yaml"

# RUST_LOG=warn: Die Tabellen von `key list` und `admin-token list` werden unten gelesen.
g() { $DC exec -T -e RUST_LOG=warn fotos /garage "$@"; }

[ -f "$SECRETS/rpc_secret" ] || { echo "Fotospeicher nicht eingerichtet: fotos-init.sh ausführen" >&2; exit 1; }

# Erst neben das alte schreiben, dann tauschen: Bricht openssl ab, bleibt das alte Geheimnis stehen.
(umask 077 && openssl rand -hex 32 > "$SECRETS/rpc_secret.neu")
mv "$SECRETS/rpc_secret.neu" "$SECRETS/rpc_secret"
$DC up -d --force-recreate fotos
i=0
until g status >/dev/null 2>&1; do
  i=$((i + 1)); [ $i -lt 30 ] || { echo "Garage antwortet nicht – docker compose -f compose.m1.yaml logs fotos" >&2; exit 1; }
  sleep 1
done
for b in "$BUCKET" "$INBOX"; do
  g bucket info "$b" >/dev/null 2>&1 || { echo "Bucket $b fehlt – erst fotos-init.sh ausführen" >&2; exit 1; }
done

# IDs aus der ersten Spalte; Kopfzeile und Leerzeilen fallen am Muster heraus.
for id in $(g key list | awk '$1 ~ /^GK[0-9a-f]+$/ { print $1 }'); do
  g key delete --yes "$id" >/dev/null
done
for id in $(g admin-token list | awk 'NR > 1 && $1 ~ /^[0-9a-f]+$/ { print $1 }'); do
  g admin-token delete --yes "$id" >/dev/null
done
rest=$(g key list | awk '$1 ~ /^GK/' ; g admin-token list | awk 'NR > 1 && NF')
[ -z "$rest" ] || { printf 'Nicht alles gelöscht:\n%s\n' "$rest" >&2; exit 1; }

echo "== Server-Schlüssel (Key ID und Secret key in Suchlage eintragen) =="
g key create suchlage-server
g bucket allow --read --write "$BUCKET" --key suchlage-server >/dev/null
g bucket allow --read --write "$INBOX" --key suchlage-server >/dev/null
echo
echo "== Admin-Token (nur jetzt sichtbar, in Suchlage eintragen) =="
g admin-token create \
  --scope GetBucketInfo,CreateKey,AllowBucketKey,ListKeys,DeleteKey \
  suchlage-server
echo
echo "Weiter in Suchlage unter Einstellungen → Fotospeicher: Schlüssel-ID, geheimer Schlüssel und"
echo "Admin-Token neu eintragen, die Adressen bleiben."
