#!/usr/bin/env bash
# Wrasse's own data must stay under 200 MB (PLAN.md §8): the scan cache, the scan tree, logs and
# settings in its data folder. The Quarantine folder (the tray) is left out: it holds the user's
# files, not Wrasse's. Read-only.
set -euo pipefail
LIMIT_MB="${WRASSE_DATA_LIMIT_MB:-200}"
case "$(uname -s)" in
  Darwin) dir="${WRASSE_DATA_DIR:-$HOME/Library/Application Support/com.shanmukyadav.wrasse}" ;;
  *) echo "check-data-size: macOS only for now"; exit 0 ;;
esac
if [[ ! -d "$dir" ]]; then
  echo "No data folder yet ($dir): nothing to check."
  exit 0
fi
total_kb=$(du -sk "$dir" | awk '{print $1}')
tray_kb=0
[[ -d "$dir/Quarantine" ]] && tray_kb=$(du -sk "$dir/Quarantine" | awk '{print $1}')
own_kb=$((total_kb - tray_kb))
echo "Wrasse data: $((own_kb / 1024)) MB without the tray ($((tray_kb / 1024)) MB in the tray); budget ${LIMIT_MB} MB."
if (( own_kb > LIMIT_MB * 1024 )); then
  echo "Wrasse's own data is over the ${LIMIT_MB} MB budget."
  exit 1
fi
