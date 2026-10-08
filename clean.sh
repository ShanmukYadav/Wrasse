#!/usr/bin/env bash
# Remove the dev app's data from the user Library (identifier com.shanmukyadav.wrasse).
set -euo pipefail
ID="com.shanmukyadav.wrasse"
for dir in \
  "$HOME/Library/Application Support/$ID" \
  "$HOME/Library/Caches/$ID" \
  "$HOME/Library/WebKit/$ID"; do
  if [[ -e "$dir" ]]; then
    rm -rf "$dir"
    echo "removed $dir"
  fi
done
