#!/usr/bin/env bash
# Size budgets (PLAN.md §8), after `npm run tauri build`:
#   macOS: .dmg <= 15 MB, installed app (Wrasse.app) <= 40 MB
#   Windows: NSIS .exe <= 15 MB
set -euo pipefail
cd "$(dirname "$0")/.."

bundle_dir=target/release/bundle
case "$(uname -s)" in
  Darwin) file=$(ls "$bundle_dir"/dmg/*.dmg | head -1); limit_mb=15 ;;
  MINGW*|MSYS*|CYGWIN*) file=$(ls "$bundle_dir"/nsis/*.exe | head -1); limit_mb=15 ;;
  *) echo "No installer budget for $(uname -s)"; exit 0 ;;
esac

bytes=$(wc -c < "$file" | tr -d ' ')
limit=$((limit_mb * 1024 * 1024))
echo "$(basename "$file"): $((bytes / 1024)) KB (budget ${limit_mb} MB)"
if (( bytes > limit )); then
  echo "Installer exceeds the ${limit_mb} MB budget."
  exit 1
fi

if [[ "$(uname -s)" == Darwin ]]; then
  app=$(ls -d "$bundle_dir"/macos/*.app | head -1)
  app_kb=$(du -sk "$app" | awk '{print $1}')
  echo "$(basename "$app") installed: $((app_kb / 1024)) MB (budget 40 MB)"
  if (( app_kb > 40 * 1024 )); then
    echo "The installed app exceeds the 40 MB budget."
    exit 1
  fi
fi
