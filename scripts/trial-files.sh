#!/usr/bin/env bash
# Disposable files for the fresh-account test (docs/fresh-account-test.md). Only ever touches
# ~/WrasseTrial.
#   scripts/trial-files.sh create   make ~/WrasseTrial with a few files and a checksum list
#   scripts/trial-files.sh check    compare the files there now with the checksum list
#   scripts/trial-files.sh remove   delete ~/WrasseTrial
set -euo pipefail
DIR="$HOME/WrasseTrial"
SUMS="$HOME/WrasseTrial-checksums.txt"
case "${1:-}" in
  create)
    mkdir -p "$DIR/big" "$DIR/old"
    mkfile 600m "$DIR/big/trial-video.mov"
    mkfile 120m "$DIR/old/trial-archive.zip"
    cp "$DIR/old/trial-archive.zip" "$DIR/old/trial-archive copy.zip"
    printf 'notes\n' > "$DIR/notes.txt"
    touch -t 202301010000 "$DIR/old/trial-archive.zip" "$DIR/old/trial-archive copy.zip"
    (cd "$DIR" && find . -type f -print0 | sort -z | xargs -0 shasum -a 256) > "$SUMS"
    echo "Created $DIR ($(du -sh "$DIR" | cut -f1)); checksums in $SUMS"
    ;;
  check)
    (cd "$DIR" && shasum -a 256 -c "$SUMS")
    ;;
  remove)
    rm -rf -- "$DIR" "$SUMS"
    echo "Removed $DIR"
    ;;
  *) echo "usage: scripts/trial-files.sh create|check|remove"; exit 2 ;;
esac
