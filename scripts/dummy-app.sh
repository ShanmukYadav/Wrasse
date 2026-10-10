#!/usr/bin/env bash
# Dummy app for the Phase 3 checkpoint on a real Mac (never a real app):
#   scripts/dummy-app.sh create   ~/Applications/WrasseDummy.app (com.wrasse-test.dummy) with
#                                 leftovers in ~/Library, and leftovers of a "removed" app
#                                 (com.wrasse-gone-test.app, dated 60 days ago)
#   scripts/dummy-app.sh files    list every dummy file (for checksums)
#   scripts/dummy-app.sh remove   delete exactly those dummy paths, nothing else
set -euo pipefail
ID=com.wrasse-test.dummy
GONE=com.wrasse-gone-test.app
APP="$HOME/Applications/WrasseDummy.app"
LIB="$HOME/Library"
PATHS=(
  "$APP"
  "$LIB/Caches/$ID"
  "$LIB/Application Support/WrasseDummy"
  "$LIB/Preferences/$ID.plist"
  "$LIB/Saved Application State/$ID.savedState"
  "$LIB/Logs/$ID"
  "$LIB/HTTPStorages/$ID"
  "$LIB/WebKit/$ID"
  "$LIB/Caches/$GONE"
  "$LIB/Application Support/$GONE"
)

case "${1:-}" in
  create)
    mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
    mkdir -p "$APP/Contents"
    cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>$ID</string>
  <key>CFBundleName</key><string>WrasseDummy</string>
  <key>CFBundleExecutable</key><string>WrasseDummy</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSUIElement</key><true/>
</dict></plist>
PLIST
    # A real, invisible Mac app that does nothing but answer Quit, so "running" and "Quit first"
    # can be tested without a real app. Built with the Command Line Tools compiler (a copied system
    # binary would be killed by code signing). No Dock icon, no window.
    printf '%s\n' '#import <Cocoa/Cocoa.h>' \
      'int main(void) { @autoreleasepool { [NSApplication sharedApplication];' \
      '[NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory]; [NSApp run]; } return 0; }' |
      cc -x objective-c -framework Cocoa -o "$APP/Contents/MacOS/WrasseDummy" -
    head -c 3000000 /dev/urandom > "$APP/Contents/Resources/assets.bin"
    mkdir -p "$LIB/Caches/$ID" "$LIB/Application Support/WrasseDummy" "$LIB/Saved Application State/$ID.savedState" \
      "$LIB/Logs/$ID" "$LIB/HTTPStorages/$ID" "$LIB/WebKit/$ID" "$LIB/Caches/$GONE" "$LIB/Application Support/$GONE"
    head -c 400000 /dev/urandom > "$LIB/Caches/$ID/Cache.db"
    head -c 120000 /dev/urandom > "$LIB/Application Support/WrasseDummy/state.json"
    # A real plist. Random bytes here are not a preference file: when the dummy actually quits,
    # cfprefsd throws them away, and the checksum round trip no longer has a file to compare.
    cat > "$LIB/Preferences/$ID.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>WrasseDummy</key><string>keep</string>
</dict></plist>
PLIST
    head -c 8000 /dev/urandom > "$LIB/Saved Application State/$ID.savedState/windows.plist"
    head -c 9000 /dev/urandom > "$LIB/Logs/$ID/today.log"
    head -c 1000 /dev/urandom > "$LIB/HTTPStorages/$ID/httpstorages.sqlite"
    head -c 1000 /dev/urandom > "$LIB/WebKit/$ID/storage.db"
    head -c 700000 /dev/urandom > "$LIB/Caches/$GONE/blob"
    head -c 200000 /dev/urandom > "$LIB/Application Support/$GONE/data"
    old=$(date -v-60d +%Y%m%d%H%M)
    touch -t "$old" "$LIB/Caches/$GONE/blob" "$LIB/Caches/$GONE" "$LIB/Application Support/$GONE/data" "$LIB/Application Support/$GONE"
    echo "Created the dummy app and its leftovers."
    ;;
  files)
    for p in "${PATHS[@]}"; do [[ -e "$p" ]] && find "$p" -type f; done | sort
    ;;
  remove)
    for p in "${PATHS[@]}"; do
      [[ -e "$p" ]] && rm -rf -- "$p" && echo "removed $p"
    done
    ;;
  *)
    echo "usage: $0 create|files|remove" >&2
    exit 2
    ;;
esac
