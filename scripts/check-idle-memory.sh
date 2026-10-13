#!/usr/bin/env bash
# Memory of one Wrasse launch, counted only for Wrasse's own tree.
#
#   scripts/check-idle-memory.sh                 build the release app, one launch
#   scripts/check-idle-memory.sh --no-build      measure the existing release build
#   scripts/check-idle-memory.sh --launches 5    repeat the cold launch this many times
#   scripts/check-idle-memory.sh --full-scan     one launch with a throwaway data folder, so the
#                                                scan reads the whole disk; checks the peak during it
#
# The tree is the app pid plus WebKit WebContent, GPU and Networking helpers that
# belong to it (parent chain, responsible pid, or that app's launchd services).
# Other apps' WebKit processes are not counted. See scripts/wrasse-tree.py.
#
# Budgets (PLAN.md §8), all phys_footprint (the Memory column in Activity Monitor):
#   peak  the highest simultaneous total during the first 60 s (250 MB)
#   idle  the worst of three samples taken at 60 s, 70 s and 80 s, window open (100 MB)
#   scan  with --full-scan: the highest total while a full scan runs, first 150 s (300 MB)
# The owner accepted ~1% over these on 2026-10-13 (101 / 253 / 303 MB); the check still reports
# against the budgets themselves. A feature that breaks one is fixed, never the budget.
# WRASSE_PEAK_LIMIT_MB, WRASSE_IDLE_LIMIT_MB and WRASSE_SCAN_LIMIT_MB override them.
# macOS only. Needs a logged-in session because it opens the window.
set -euo pipefail
cd "$(dirname "$0")/.."

IDLE_MB="${WRASSE_IDLE_LIMIT_MB:-100}"
PEAK_MB="${WRASSE_PEAK_LIMIT_MB:-250}"
SCAN_MB="${WRASSE_SCAN_LIMIT_MB:-300}"
FULL_SCAN=0
APP="target/release/wrasse-app"
LAUNCHES=1
BUILD=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-build) BUILD=0; shift ;;
    --full-scan) FULL_SCAN=1; shift ;;
    --launches)
      [[ $# -ge 2 ]] || { echo "check-idle-memory: --launches needs a number"; exit 2; }
      LAUNCHES="$2"
      shift 2
      ;;
    *) echo "check-idle-memory: unknown argument $1"; exit 2 ;;
  esac
done
[[ "$LAUNCHES" =~ ^[1-9][0-9]*$ ]] || { echo "check-idle-memory: --launches must be a positive number"; exit 2; }

if [[ "$(uname)" != "Darwin" ]]; then
  echo "check-idle-memory: macOS only for now (Windows budget: measure, then add)."
  exit 0
fi

if pgrep -x wrasse-app >/dev/null; then
  echo "Quit Wrasse first: a running copy would take the launch (single instance) and skew the numbers."
  exit 2
fi

if [[ "$BUILD" -eq 1 ]]; then
  (cd app && ../dev.sh npm run tauri build -- --no-bundle >/dev/null)
fi
[[ -x "$APP" ]] || { echo "No release build at $APP"; exit 2; }

# Current footprint and the process's own high-water, in MB.
footprint_pair() {
  local out
  out=$(footprint "$1" 2>/dev/null || true)
  awk '
    /phys_footprint_peak:/ { pv = $2; if ($3 == "KB") pv /= 1024; if ($3 == "GB") pv *= 1024; next }
    /phys_footprint:/ { cv = $2; if ($3 == "KB") cv /= 1024; if ($3 == "GB") cv *= 1024 }
    END { printf "%.1f %.1f\n", (cv == "" ? 0 : cv), (pv == "" ? 0 : pv) }
  ' <<<"$out"
}

role_of() {
  local cmd
  cmd=$(ps -o command= -p "$1" 2>/dev/null || echo gone)
  case "$cmd" in
    *WebContent*) echo web ;;
    *WebKit.GPU*) echo gpu ;;
    *WebKit.Networking*) echo net ;;
    *wrasse-app*) echo app ;;
    *) echo other ;;
  esac
}

# Sets app_mb web_mb gpu_mb net_mb other_mb total_mb, and updates *_peak high-waters.
measure() {
  local pid cur peak role pids
  app_mb=0; web_mb=0; gpu_mb=0; net_mb=0; other_mb=0
  pids=$(/usr/bin/python3 scripts/wrasse-tree.py "$app_pid")
  for pid in $pids; do
    read -r cur peak < <(footprint_pair "$pid") || true
    [[ -n "${cur:-}" ]] || continue
    role=$(role_of "$pid")
    case "$role" in
      app) app_mb=$cur ;;
      web) web_mb=$(awk -v a="$web_mb" -v b="$cur" 'BEGIN { printf "%.1f", a + b }') ;;
      gpu) gpu_mb=$(awk -v a="$gpu_mb" -v b="$cur" 'BEGIN { printf "%.1f", a + b }') ;;
      net) net_mb=$(awk -v a="$net_mb" -v b="$cur" 'BEGIN { printf "%.1f", a + b }') ;;
      *) other_mb=$(awk -v a="$other_mb" -v b="$cur" 'BEGIN { printf "%.1f", a + b }') ;;
    esac
    case "$role" in
      app) app_peak=$(awk -v a="$app_peak" -v b="$peak" 'BEGIN { print (b > a) ? b : a }') ;;
      web) web_peak=$(awk -v a="$web_peak" -v b="$peak" 'BEGIN { print (b > a) ? b : a }') ;;
      gpu) gpu_peak=$(awk -v a="$gpu_peak" -v b="$peak" 'BEGIN { print (b > a) ? b : a }') ;;
      net) net_peak=$(awk -v a="$net_peak" -v b="$peak" 'BEGIN { print (b > a) ? b : a }') ;;
    esac
  done
  total_mb=$(awk -v a="$app_mb" -v w="$web_mb" -v g="$gpu_mb" -v n="$net_mb" -v o="$other_mb" \
    'BEGIN { printf "%.1f", a + w + g + n + o }')
}

over() { awk -v v="$1" -v l="$2" 'BEGIN { exit !(v > l) }'; }

fail=0
worst_peak=0
worst_idle=0
WINDOW=60
if [[ "$FULL_SCAN" -eq 1 ]]; then
  # A throwaway data folder: no saved scan or scan tree, so the launch scan reads everything.
  # The real data folder (tray, logs, cache) is not touched.
  WRASSE_DATA_DIR=$(mktemp -d -t wrasse-fullscan)
  export WRASSE_DATA_DIR
  LAUNCHES=1
  WINDOW=150
fi

for launch in $(seq 1 "$LAUNCHES"); do
  echo "=== launch $launch ==="
  WRASSE_AUTOSCAN=1 "$APP" >/dev/null 2>&1 &
  app_pid=$!
  trap 'kill "$app_pid" 2>/dev/null || true' EXIT
  echo "app pid $app_pid"

  app_peak=0; web_peak=0; gpu_peak=0; net_peak=0
  launch_peak=0
  peak_at=0
  peak_line=""
  start=$(date +%s)
  while true; do
    now=$(date +%s)
    elapsed=$((now - start))
    if [[ "$elapsed" -gt "$WINDOW" ]]; then
      break
    fi
    if ! kill -0 "$app_pid" 2>/dev/null; then
      echo "app exited before the minute was up"
      exit 1
    fi
    measure
    if awk -v t="$total_mb" -v p="$launch_peak" 'BEGIN { exit !(t > p) }'; then
      launch_peak=$total_mb
      peak_at=$elapsed
      peak_line="app ${app_mb} web ${web_mb} gpu ${gpu_mb} net ${net_mb} other ${other_mb}"
    fi
    if [[ $((elapsed % 5)) -eq 0 ]]; then
      echo "t=${elapsed}s ${total_mb} MB (app ${app_mb} web ${web_mb} gpu ${gpu_mb} net ${net_mb} other ${other_mb})"
    fi
    sleep 1
  done

  echo "peak launch $launch: ${launch_peak} MB at t=${peak_at}s (${peak_line})"
  echo "high-water launch $launch: app ${app_peak} web ${web_peak} gpu ${gpu_peak} net ${net_peak}"
  worst_peak=$(awk -v a="$worst_peak" -v b="$launch_peak" 'BEGIN { print (b > a) ? b : a }')

  idle_worst=0
  idle_line=""
  samples="1 2 3"
  [[ "$FULL_SCAN" -eq 1 ]] && samples=""
  for sample in $samples; do
    measure
    echo "idle sample $sample: ${total_mb} MB (app ${app_mb} web ${web_mb} gpu ${gpu_mb} net ${net_mb} other ${other_mb})"
    idle_line="${idle_line}${total_mb} "
    if awk -v t="$total_mb" -v p="$idle_worst" 'BEGIN { exit !(t > p) }'; then
      idle_worst=$total_mb
    fi
    if [[ "$sample" -lt 3 ]]; then
      sleep 10
    fi
  done
  echo "summary launch $launch: peak ${launch_peak} MB, idle ${idle_line}MB"
  worst_idle=$(awk -v a="$worst_idle" -v b="$idle_worst" 'BEGIN { print (b > a) ? b : a }')

  kill "$app_pid" 2>/dev/null || true
  wait "$app_pid" 2>/dev/null || true
  for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
    kill -0 "$app_pid" 2>/dev/null || break
    sleep 1
  done
  if kill -0 "$app_pid" 2>/dev/null; then
    echo "app did not exit"
    exit 1
  fi
  sleep 2
done
trap - EXIT

if [[ "$FULL_SCAN" -eq 1 ]]; then
  [[ "$WRASSE_DATA_DIR" == */wrasse-fullscan* ]] && rm -rf "$WRASSE_DATA_DIR"
  echo "Peak during a full scan ${worst_peak} MB (budget ${SCAN_MB} MB)."
  if over "$worst_peak" "$SCAN_MB"; then
    echo "Memory during a full scan ${worst_peak} MB is over the ${SCAN_MB} MB budget."
    exit 1
  fi
  echo "Full-scan memory check passed: ${worst_peak} MB."
  exit 0
fi
echo "Worst simultaneous peak ${worst_peak} MB (budget ${PEAK_MB} MB)."
echo "Worst idle sample ${worst_idle} MB (budget ${IDLE_MB} MB)."
if over "$worst_peak" "$PEAK_MB"; then
  echo "Peak memory ${worst_peak} MB is over the ${PEAK_MB} MB first-minute budget."
  fail=1
fi
if over "$worst_idle" "$IDLE_MB"; then
  echo "Idle memory ${worst_idle} MB is over the ${IDLE_MB} MB budget."
  fail=1
fi
if [[ "$fail" -ne 0 ]]; then
  exit 1
fi
echo "Memory check passed: peak ${worst_peak} MB, idle ${worst_idle} MB."
