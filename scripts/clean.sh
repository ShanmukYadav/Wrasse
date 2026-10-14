#!/usr/bin/env bash
# Remove the dev app's data from the user Library (identifier com.shanmukyadav.wrasse).
#
#   scripts/clean.sh         removes caches, scan data and settings, but KEEPS the quarantine tray
#                      (it may hold files you still want back)
#   scripts/clean.sh --all   also deletes the tray for good, after asking
set -euo pipefail
ID="com.shanmukyadav.wrasse"
DATA="$HOME/Library/Application Support/$ID"
TRAY="$DATA/Quarantine"

all=false
[[ "${1:-}" == "--all" ]] && all=true

if [[ -d "$TRAY" ]]; then
  size=$(du -sh "$TRAY" 2>/dev/null | cut -f1)
  if $all; then
    read -r -p "Delete the quarantine tray ($size) for good? Files in it can't be restored afterwards. [y/N] " answer
    [[ "$answer" =~ ^[Yy]$ ]] || { echo "Nothing removed."; exit 1; }
  else
    echo "Keeping the quarantine tray ($size) at $TRAY. Use --all to delete it too."
  fi
fi

for dir in "$HOME/Library/Caches/$ID" "$HOME/Library/WebKit/$ID"; do
  if [[ -e "$dir" ]]; then
    rm -rf "$dir"
    echo "removed $dir"
  fi
done

if [[ -d "$DATA" ]]; then
  if $all; then
    rm -rf "$DATA"
    echo "removed $DATA"
  else
    find "$DATA" -mindepth 1 -maxdepth 1 ! -name Quarantine -exec rm -rf {} +
    echo "removed $DATA (except Quarantine)"
  fi
fi
