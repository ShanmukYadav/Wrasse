#!/usr/bin/env bash
# CI only: puts the Wrasse code-signing certificate in a temporary keychain and exports its
# identity as APPLE_SIGNING_IDENTITY, which the Tauri bundler signs the app with.
#
# Why: macOS remembers privacy permissions (Full Disk Access, Finder automation) per code
# signature. An ad-hoc signature changes with every build, so each new build lost them and asked
# again. A fixed certificate keeps the same identity across builds and updates. It is self-made,
# not Apple's, so first-launch Gatekeeper steps stay the same.
#
# Needs WRASSE_SIGNING_P12 (the .p12, base64) and WRASSE_SIGNING_P12_PASSWORD. With --required it
# fails when they're missing; otherwise the build stays ad hoc.
set -euo pipefail

if [[ -z "${WRASSE_SIGNING_P12:-}" ]]; then
  if [[ "${1:-}" == "--required" ]]; then
    echo "The WRASSE_SIGNING_P12 secret is missing: release builds must be signed." >&2
    exit 1
  fi
  echo "No signing certificate: this build is signed ad hoc."
  exit 0
fi

keychain="$RUNNER_TEMP/wrasse-signing.keychain-db"
password=$(openssl rand -hex 24)
p12="$RUNNER_TEMP/wrasse-signing.p12"

security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
printf '%s' "$WRASSE_SIGNING_P12" | base64 --decode > "$p12"
security import "$p12" -k "$keychain" -P "$WRASSE_SIGNING_P12_PASSWORD" -T /usr/bin/codesign >/dev/null
rm -f "$p12"
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$password" "$keychain" >/dev/null
# shellcheck disable=SC2046
security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')

identity=$(security find-identity -p codesigning "$keychain" | awk '/"Wrasse Code Signing"/ { print $2; exit }')
if [[ -z "$identity" ]]; then
  echo "The certificate in WRASSE_SIGNING_P12 isn't a \"Wrasse Code Signing\" identity." >&2
  exit 1
fi
echo "APPLE_SIGNING_IDENTITY=$identity" >> "$GITHUB_ENV"
echo "Signing with the Wrasse certificate ($identity)."
