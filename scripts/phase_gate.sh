#!/usr/bin/env bash
# Phase-end gates in two overlapping lanes.
#
#   wasm lane  release WASM packages (own target dir, starts at once)
#   ui lane    once the packages exist: ui-check, ui-e2e, ui-harness, ui-install, docs build,
#              then notebooks (after the Python build)
#   cargo lane lint -> rust tests -> doctests -> python build+tests, then the dev WASM tests,
#              which rewrite the packages and so wait for the ui lane to finish
# Usage: phase_gate.sh [--full] [--dry-run] [--base REF]
set -u
cd "$(dirname "$0")/.."

full=0 dry=0 base=master
while [ $# -gt 0 ]; do
  case $1 in
    --full) full=1 ;;
    --dry-run) dry=1 ;;
    --base) base=$2; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

out=${GATE_OUT:-${TMPDIR:-/tmp}/phase-gate-$(date +%Y%m%d-%H%M%S)}
mkdir -p "$out"
summary=$out/summary.txt
: >"$summary"

selection=$(python3 scripts/gate_select.py --base "$base") || exit 2
if [ "$full" = 1 ]; then
  selection=$(python3 - <<'P'
import sys
sys.path.insert(0, "scripts")
import gate_select
print("\n".join(f"gate {g}" for g in gate_select.GATES))
P
)
fi
want() { printf '%s\n' "$selection" | grep -qx "gate $1"; }
doctest_packages=$(printf '%s\n' "$selection" | sed -n 's/^doctest_packages //p')
echo "gates: $(printf '%s\n' "$selection" | sed -n 's/^gate //p' | tr '\n' ' ')" | tee -a "$summary"
[ -n "$doctest_packages" ] && echo "doctest packages: $doctest_packages" | tee -a "$summary"

# gate <name> <cmd...>: run one gate, log it, record seconds and exit code.
gate() {
  local name=$1; shift
  if [ "$dry" = 1 ]; then echo "[dry-run] $name: $*"; return 0; fi
  local start=$SECONDS
  "$@" >"$out/$name.log" 2>&1
  local rc=$?
  echo "$name rc=$rc secs=$((SECONDS - start)) load=$(uptime | sed 's/.*averages*: //')" >>"$summary"
  return $rc
}

needs_pkg() {
  want ui-check || want ui-e2e || want ui-harness || want ui-install || want docs-build
}

# Release WASM packages in their own target dir, so this lane never waits on the
# native cargo lane's build lock. Every UI gate then runs on the release packages.
wasm_lane() {
  if needs_pkg; then
    CARGO_TARGET_DIR=${GATE_WASM_TARGET:-target/gate-wasm} gate wasm-pkg mise run wasm-pkg \
      || touch "$out/wasm-pkg.failed"
  fi
  touch "$out/wasm-pkg.ready"
}

cargo_lane() {
  local rc=0
  want lint && { gate all-lint mise run all-lint || rc=1; }
  want rust-test && { gate rust-test mise run rust-test || rc=1; }
  if want rust-doctest; then
    DOCTEST_PACKAGES=$doctest_packages gate rust-doctest mise run rust-test --doctest || rc=1
  fi
  # python-test rebuilds the extension itself, so no separate python-build.
  want python && { gate python-test mise run python-test --all || rc=1; }
  touch "$out/python.ready"
  # wasm-test rebuilds the dev packages into the same pkg directories the UI gates
  # and ui-install read, so it runs once the UI lane is finished with them.
  if want wasm; then
    wait_for "$out/ui.done"
    gate wasm-test mise run wasm-test || rc=1
    gate wasm-doctest mise run wasm-test --doctest || rc=1
  fi
  return $rc
}

ui_lane() {
  local rc=0
  wait_for "$out/wasm-pkg.ready"
  if [ -e "$out/wasm-pkg.failed" ]; then
    echo "ui gates skipped: wasm-pkg failed" >>"$summary"
    rc=1
  else
    want ui-check && { gate ui-check mise run ui-check || rc=1; }
    want ui-e2e && { gate ui-e2e mise run ui-e2e || rc=1; }
    want ui-harness && { gate ui-harness mise run ui-harness || rc=1; }
    want ui-install && { gate ui-install mise run ui-install || rc=1; }
    want docs-build && { gate ui-docs-build mise run ui-docs-build || rc=1; }
  fi
  touch "$out/ui.done"
  # Notebooks need the Python extension, not the WASM packages.
  wait_for "$out/python.ready"
  want notebooks && { gate notebooks mise run python-examples || rc=1; }
  return $rc
}

wait_for() {
  [ "$dry" = 1 ] && return 0
  while [ ! -e "$1" ]; do sleep 5; done
}

wasm_lane & wasm_pid=$!
cargo_lane & cargo_pid=$!
ui_lane & ui_pid=$!
wait $wasm_pid
wait $cargo_pid; cargo_rc=$?
wait $ui_pid; ui_rc=$?
echo "cargo lane rc=$cargo_rc, ui lane rc=$ui_rc; logs in $out"
[ $cargo_rc -eq 0 ] && [ $ui_rc -eq 0 ] && [ ! -e "$out/wasm-pkg.failed" ]
