#!/usr/bin/env bash
# Fails if SECURITY_AUDIT.md's lists differ from the code: protected locations, read-only
# programs, delegated actions and app-library suffixes. Run by CI and the pre-commit hook.
set -euo pipefail
cd "$(dirname "$0")/.."

doc=SECURITY_AUDIT.md
fail=0

# The lines of a doc block: between "<!-- AUDIT:<name> -->" and "<!-- /AUDIT -->", fences dropped.
doc_block() {
  awk -v name="$1" '
    $0 ~ "<!-- AUDIT:" name " -->" { on = 1; next }
    on && /<!-- \/AUDIT -->/ { on = 0 }
    on && !/^```/ && NF { sub(/^[ \t]+/, ""); print }
  ' "$doc"
}

compare() {
  local name="$1" from_code="$2"
  if ! diff <(doc_block "$name") <(printf '%s\n' "$from_code") >/dev/null; then
    echo "SECURITY_AUDIT.md is out of sync with the code ($name):"
    diff <(doc_block "$name") <(printf '%s\n' "$from_code") | sed 's/^/  /' || true
    echo "  (< in the doc, > in the code)"
    fail=1
  fi
}

# Protected locations: hard("/System".into()), hard(h("Library/Keychains")), pick(...), folder(...).
code_protected=$(awk '/AUDIT-BEGIN macos-protected/{on=1;next} /AUDIT-END macos-protected/{on=0} on' \
    crates/wrasse-core/src/platform/macos/protect.rs |
  sed -nE 's/^[[:space:]]*(hard|pick|folder)\((h\("([^"]*)"\)|"([^"]*)"\.into\(\))\),?$/\1 \3\4/p' |
  awk '{ kind = ($1 == "pick") ? "hand-pick" : $1; path = $2; for (i = 3; i <= NF; i++) path = path " " $i;
         if (path !~ /^\//) path = "~/" path; print kind " " path }')
compare macos-protected "$code_protected"

code_programs=$(sed -nE 's/^const READ_ONLY: \[&str; [0-9]+\] = \[(.*)\];$/\1/p' crates/wrasse-core/src/platform/macos/commands.rs |
  tr ',' '\n' | sed -E 's/[[:space:]"]//g' | grep -v '^$')
compare read-only-programs "$code_programs"

code_delegated=$(awk '/pub enum Delegated/{on=1;next} on && /^}/{on=0} on' crates/wrasse-core/src/safety/mod.rs |
  sed -nE 's/^[[:space:]]*([A-Z][A-Za-z]*)(,| \{.*)$/\1/p')
compare delegated-actions "$code_delegated"

code_suffixes=$(awk '/fn protected_name_suffixes/{on=1;next} on && /\]/{print; on=0} on' crates/wrasse-core/src/platform/mod.rs |
  grep -oE '"\.[a-z]+"' | tr -d '"' | sed 's/.*/`&`/' | paste -sd, - | sed 's/,/, /g')
compare library-suffixes "$code_suffixes"

if [[ $fail -ne 0 ]]; then
  exit 1
fi
echo "SECURITY_AUDIT.md sync check passed."
