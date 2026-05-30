#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
report=""
families=()

usage() {
  cat <<'USAGE'
Usage:
  scripts/fetch.sh <report> <family> [family ...]

Examples:
  scripts/fetch.sh legacy samesuite
  scripts/fetch.sh docboy docboy-dmg
  scripts/fetch.sh gbmicrotest gbmicrotest
  scripts/fetch.sh gb-emulator-shootout acid

Report ids:
  legacy
  docboy
  gbmicrotest
  gb-emulator-shootout
USAGE
}

if [[ ${1:-} == "-h" || ${1:-} == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -gt 0 ]]; then
  report=$1
  shift
fi
families=("$@")

if [[ -z $report ]]; then
  echo "report is required; use scripts/fetch.sh legacy <family>, scripts/fetch.sh docboy <family>, scripts/fetch.sh gbmicrotest <family>, or scripts/fetch.sh gb-emulator-shootout <family>" >&2
  exit 2
fi

if [[ ${#families[@]} -eq 0 ]]; then
  echo "at least one family is required after report $report" >&2
  exit 2
fi

cd "$repo_root"
exec cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- "$report" "${families[@]}"
