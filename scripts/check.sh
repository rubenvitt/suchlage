#!/bin/sh
# Qualitäts-Gate: vor jedem Commit, unverändert auch in einer späteren CI (dort mit CI=1).
#   scripts/check.sh            alles
#   scripts/check.sh --rust     nur Rust, Vertrag, Pilot-Seite (Backend-Strang)
#   scripts/check.sh --frontend nur das Frontend (Frontend-Strang)
set -eu
cd "$(dirname "$0")/.."
part=${1:-all}
case "$part" in all|--rust|--frontend) ;; *) echo "Aufruf: scripts/check.sh [--rust|--frontend]"; exit 2 ;; esac

if [ "$part" != --frontend ]; then
  echo "→ Modulgrenzen"
  # Ohne \b, damit GNU- und BSD-grep gleich prüfen. `w` steht für „kein Bezeichnerzeichen“.
  w='[^[:alnum:]_]'
  runtime_crates='(axum|sqlx|tokio|rumqttc|reqwest)'
  # Alle .rs-Dateien als eine Zeile: So fallen auch mehrzeilige Gruppenimporte `use crate::{…};` auf.
  flat() { find "$@" -name '*.rs' -exec cat {} + | tr '\n' ' '; }
  if [ -d src/situation ] && grep -rnE "(^|$w)$runtime_crates::|(^|$w)use (::)?$runtime_crates($w|$)" src/situation; then
    echo "FAIL src/situation importiert Laufzeit-Crates"; exit 1
  fi
  if flat src -not -path 'src/dji/*' | grep -oE "dji::(\{([^;]*$w)?)?wire($w|$)"; then
    echo "FAIL dji::wire wird außerhalb von src/dji benutzt"; exit 1
  fi
  if [ -d src/dji ] && flat src/dji | grep -oE "crate::(\{([^;]*$w)?)?(runtime|web)($w|$)"; then
    echo "FAIL src/dji hängt an runtime oder web (Zyklus)"; exit 1
  fi
  if [ -d src/contract ] && grep -rnE "crate::" src/contract | grep -vE "crate::contract($w|$)"; then
    echo "FAIL src/contract hängt an anderen Teilen des Crates"; exit 1
  fi

  echo "→ Rust"
  # Lokal formatiert das Gate selbst; in CI prüft es nur.
  if [ "${CI:-}" = 1 ]; then cargo fmt --check; else cargo fmt; fi
  cargo clippy --all-targets -- -D warnings
  cargo clippy --all-targets --features dev -- -D warnings

  # Der Vertrag wird bei cargo test neu erzeugt. Ändert sich dabei etwas, war er veraltet.
  contract_hash() { [ -d frontend/src/contract ] && find frontend/src/contract -type f -exec shasum {} + | sort | shasum || echo none; }
  before=$(contract_hash)
  cargo test
  cargo test --features dev
  if [ "$before" != "$(contract_hash)" ]; then
    echo "FAIL Vertrag war veraltet und wurde neu erzeugt – Änderungen in frontend/src/contract prüfen und mitcommitten"
    exit 1
  fi

  if [ -d src/dji/pilot ]; then
    echo "→ Pilot-Seite"
    node --test src/dji/pilot/
  fi
fi

if [ "$part" != --rust ] && [ -f frontend/package.json ]; then
  echo "→ Frontend"
  pnpm -C frontend install --frozen-lockfile
  pnpm -C frontend lint
  pnpm -C frontend typecheck
  pnpm -C frontend test
  pnpm -C frontend build
fi
echo "ok"
