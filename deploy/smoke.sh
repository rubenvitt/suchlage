#!/bin/sh
# Lokaler Ende-zu-Ende-Test des Spike-Stacks. Startet den Stack auf 127.0.0.1 und prüft jeden Baustein.
# Läuft gegen Testgeheimnisse (op-Attrappe), nie gegen 1Password: kein Freigabedialog, keine echten DJI-Schlüssel in lokalen Dateien.
set -u
cd "$(dirname "$0")"
export SUCHLAGE_BIND_IP=127.0.0.1
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# op-Attrappe: `op read <ref>` und `op inject -f -i <in> -o <out>`; Wert deterministisch je Referenz,
# damit render.sh und smoke.sh unabhängig voneinander dieselben Geheimnisse sehen.
cat > "$TMP/op" <<'EOF'
#!/bin/sh
set -eu
val() { printf 'test%s' "$(printf '%s' "$1" | cksum | cut -d' ' -f1)"; }
case $1 in
  read) val "$2" ;;
  inject)
    shift; in= out=
    while [ $# -gt 0 ]; do case $1 in -i) in=$2; shift ;; -o) out=$2; shift ;; esac; shift; done
    cp "$in" "$out"
    grep -o '{{ op://[^ ]* }}' "$in" | sort -u | while read -r _ ref _; do
      sed -i.bak "s|{{ $ref }}|$(val "$ref")|g" "$out" && rm -f "$out.bak"
    done ;;
  *) echo "op-Attrappe: $1 nicht unterstützt" >&2; exit 2 ;;
esac
EOF
chmod +x "$TMP/op"
export PATH="$TMP:$PATH"

./render.sh || { echo "FAIL render.sh"; exit 1; }
dc() { docker compose --progress quiet "$@"; }
FAIL=0
check() { if [ "$1" = 0 ]; then echo "ok   $2"; else echo "FAIL $2"; FAIL=1; fi; }
TOOL_PASS=$(op read op://DRK/suchlage-mqtt/suchlage)
RC_PASS=$(op read op://DRK/suchlage-mqtt/rc-plus-01)
# sub_one <user> <pass> <topic>: erste Nachricht (max. 5 s) ausgeben
sub_one() { dc exec -T mqtt mosquitto_sub -h localhost -u "$1" -P "$2" -t "$3" -C 1 -W 5; }
# pub <user> <pass> <topic> <payload>
pub() { dc exec -T mqtt mosquitto_pub -h localhost -u "$1" -P "$2" -t "$3" -m "$4"; }

rm -rf ../records && mkdir -p ../records
dc up -d --build --wait mqtt recorder || dc up -d --build mqtt recorder
sleep 2

# Anonymer Zugriff wird abgelehnt
dc exec -T mqtt mosquitto_pub -h localhost -t thing/product/TEST/osd -m x 2>/dev/null
[ $? -ne 0 ]; check $? "anonymer Publish abgelehnt"

# rc-plus-01 darf auf thing/product/# publizieren, der Mitschnitt speichert es
pub rc-plus-01 "$RC_PASS" thing/product/TEST/osd '{"data":{"latitude":52.96}}'
sleep 1
grep -q '"topic":"thing/product/TEST/osd"' ../records/*.jsonl; check $? "osd landet im Mitschnitt"

# Payload mit Zeilenumbruch bleibt eine Zeile (Review Focus 1)
before=$(cat ../records/*.jsonl | wc -l)
pub rc-plus-01 "$RC_PASS" thing/product/TEST/state "$(printf '{"a":\n1}')"
sleep 1
after=$(cat ../records/*.jsonl | wc -l)
[ $((after - before)) -eq 1 ]; check $? "mehrzeiliger Payload = eine Mitschnittzeile"

# ACL: rc-plus-01 darf nicht außerhalb seiner Topics publizieren
( sub_one suchlage "$TOOL_PASS" foo/bar > "$TMP/acl.out" 2>/dev/null ) &
sleep 1; pub rc-plus-01 "$RC_PASS" foo/bar verboten; wait
[ ! -s "$TMP/acl.out" ]; check $? "ACL blockt fremde Topics"

# Broker-Neustart: Mitschnitt läuft weiter (Review Focus 4)
dc restart mqtt; sleep 3
pub rc-plus-01 "$RC_PASS" thing/product/TEST/osd '{"after":"restart"}'
sleep 2
grep -q 'after.*restart' ../records/*.jsonl; check $? "Mitschnitt überlebt Broker-Neustart"

# --- weitere Prüfungen folgen in Task 2–4 ---

dc down
exit $FAIL
