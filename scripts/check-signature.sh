#!/usr/bin/env bash
# Fails unless the built app is signed with the Wrasse certificate: the whole bundle sealed, and
# a designated requirement that names the bundle ID and the certificate (so macOS keeps its
# privacy permissions across builds). Usage: scripts/check-signature.sh <Wrasse.app>
set -euo pipefail
app="${1:?usage: check-signature.sh <Wrasse.app>}"

codesign --verify --strict --deep "$app"
requirement=$(codesign -d -r- "$app" 2>&1)
if ! grep -q 'identifier "com.shanmukyadav.wrasse" and certificate leaf' <<<"$requirement"; then
  echo "Wrasse.app isn't signed with the Wrasse certificate:" >&2
  echo "$requirement" >&2
  exit 1
fi
codesign -dv "$app" 2>&1 | grep -E '^(Identifier|Authority|TeamIdentifier)='
echo "Signature check passed."
