#!/usr/bin/env bash
# Memory regression check (owner, 2026-10-14): the new build against the previous release build,
# side by side in the same session, so the Mac's own state (which moves idle by about 10 MB from
# one hour to the next) affects both alike.
#
#   scripts/check-memory-regression.sh <previous wrasse-app binary> [--no-build]
#
# The previous binary: the last release's .dmg › Wrasse.app/Contents/MacOS/wrasse-app (mount the
# .dmg and copy it somewhere first). The new build is target/release/wrasse-app (built first
# unless --no-build). Five untouched launches of each, alternating; fails if the new build's
# median idle or median first-minute peak is more than 5 MB above the previous build's.
set -euo pipefail
cd "$(dirname "$0")/.."

PREV=${1:?usage: scripts/check-memory-regression.sh <previous wrasse-app binary> [--no-build]}
[[ -x "$PREV" ]] || { echo "Not an executable: $PREV"; exit 2; }
NEW=target/release/wrasse-app
LAUNCHES=5
MARGIN_MB=5

if [[ "${2:-}" != "--no-build" ]]; then
  (cd app && ../dev.sh npm run tauri build -- --no-bundle >/dev/null)
fi

# One launch of a binary: prints "peak idle" in MB (the worst of the three idle samples).
one() {
  local out
  out=$(scripts/check-idle-memory.sh --app "$1" --launches 1 2>&1 || true)
  awk '/^summary launch/ { peak = $5; idle = 0; for (i = 8; i <= NF; i++) if ($i + 0 > idle) idle = $i + 0 }
       END { if (peak == "") exit 1; print peak, idle }' <<<"$out"
}

median() { sort -n | awk '{ v[NR] = $1 } END { print (NR % 2) ? v[(NR + 1) / 2] : (v[NR / 2] + v[NR / 2 + 1]) / 2 }'; }

prev_peaks=(); prev_idles=(); new_peaks=(); new_idles=()
for i in $(seq 1 "$LAUNCHES"); do
  read -r p d < <(one "$PREV")
  prev_peaks+=("$p"); prev_idles+=("$d")
  echo "launch $i previous: peak $p MB, idle $d MB"
  read -r p d < <(one "$NEW")
  new_peaks+=("$p"); new_idles+=("$d")
  echo "launch $i new:      peak $p MB, idle $d MB"
done

pp=$(printf '%s\n' "${prev_peaks[@]}" | median)
pi=$(printf '%s\n' "${prev_idles[@]}" | median)
np=$(printf '%s\n' "${new_peaks[@]}" | median)
ni=$(printf '%s\n' "${new_idles[@]}" | median)
echo "Median previous: peak ${pp} MB, idle ${pi} MB"
echo "Median new:      peak ${np} MB, idle ${ni} MB"

fail=0
if awk -v n="$ni" -v p="$pi" -v m="$MARGIN_MB" 'BEGIN { exit !(n > p + m) }'; then
  echo "Idle grew by more than ${MARGIN_MB} MB."
  fail=1
fi
if awk -v n="$np" -v p="$pp" -v m="$MARGIN_MB" 'BEGIN { exit !(n > p + m) }'; then
  echo "Peak grew by more than ${MARGIN_MB} MB."
  fail=1
fi
[[ "$fail" -eq 0 ]] && echo "Memory regression check passed (within ${MARGIN_MB} MB of the previous build)."
exit "$fail"
