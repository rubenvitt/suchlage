# Eingangs-Bucket im Dev-Garage (ADR 0004). `--default-bucket` legt nur `suchlage-fotos` an; der
# Schlüssel ist der feste Dev-Schlüssel aus deploy/compose.dev.yaml. Braucht `$DC` des Aufrufers.
fotos_inbox() {
  $DC exec -T fotos /garage bucket info suchlage-fotos-eingang >/dev/null 2>&1 ||
    $DC exec -T fotos /garage bucket create suchlage-fotos-eingang >/dev/null
  $DC exec -T fotos /garage bucket allow --read --write suchlage-fotos-eingang \
    --key GK0123456789abcdef01234567 >/dev/null
}
