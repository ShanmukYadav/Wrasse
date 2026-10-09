#!/usr/bin/env bash
# Fails if Wrasse uses more memory than its budget while idle with the window open
# (PLAN.md §8: macOS < 140 MB, measured baseline 131 MB).
#
#   scripts/check-idle-memory.sh            build the release app, then measure
#   scripts/check-idle-memory.sh --no-build measure the existing release build
#
# What it measures: physical footprint (`footprint`, the number Activity Monitor shows as
# Memory) of the app plus the WebKit helper processes it started (page, GPU, network), after a
# scan and 30 s of idle. macOS only; needs a logged-in session because it opens the window.
# WRASSE_IDLE_LIMIT_MB overrides the budget.
set -euo pipefail
cd "$(dirname "$0")/.."

LIMIT_MB="${WRASSE_IDLE_LIMIT_MB:-140}"
APP="target/release/wrasse-app"

if [[ "$(uname)" != "Darwin" ]]; then
  echo "check-idle-memory: macOS only for now (Windows budget: measure, then add)."
  exit 0
fi

if pgrep -f "wrasse-app" >/dev/null; then
  echo "Quit Wrasse first: a running copy would take the launch (single instance) and skew the numbers."
  exit 2
fi

if [[ "${1:-}" != "--no-build" ]]; then
  (cd app && ../dev.sh npm run tauri build -- --no-bundle >/dev/null)
fi
[[ -x "$APP" ]] || { echo "No release build at $APP"; exit 2; }

helpers() { pgrep -f "com.apple.WebKit.(WebContent|GPU|Networking)" | sort; }
before="$(helpers)"

WRASSE_AUTOSCAN=1 "$APP" >/dev/null 2>&1 &
app_pid=$!
trap 'kill "$app_pid" 2>/dev/null || true' EXIT

# Let the scan finish and the page settle.
sleep 40
mine=$(comm -13 <(echo "$before") <(helpers) | tr '\n' ' ')

footprint_mb() {
  footprint "$1" 2>/dev/null | awk '/phys_footprint:/ { v = $2; if ($3 == "KB") v /= 1024; if ($3 == "GB") v *= 1024; printf "%.1f", v; exit }'
}

worst=0
for sample in 1 2 3; do
  total=0
  for pid in $app_pid $mine; do
    mb=$(footprint_mb "$pid")
    total=$(awk -v a="$total" -v b="${mb:-0}" 'BEGIN { printf "%.1f", a + b }')
  done
  echo "sample $sample: ${total} MB"
  worst=$(awk -v a="$worst" -v b="$total" 'BEGIN { print (b > a) ? b : a }')
  sleep 10
done

if awk -v w="$worst" -v l="$LIMIT_MB" 'BEGIN { exit !(w > l) }'; then
  echo "Idle memory ${worst} MB is over the ${LIMIT_MB} MB budget."
  exit 1
fi
echo "Idle memory check passed: ${worst} MB (budget ${LIMIT_MB} MB)."
