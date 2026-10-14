# Security policy

Wrasse deletes files, so security reports matter more than usual. Thank you for sending them.

## Reporting a vulnerability

Please report privately, not in a public issue: use **Report a vulnerability** on the repository's
[Security tab](https://github.com/ShanmukYadav/Wrasse/security/advisories/new) (GitHub private
advisories). Include what you found, the version (Settings › About, or the release name), the
macOS version, and steps to reproduce.

Especially in scope:
- anything that lets Wrasse remove, move or change a file it shouldn't (see
  [docs/security-audit.md](docs/security-audit.md) for what it may touch);
- a way around the protected locations, the Never touch list, the tray or the "Not undoable"
  confirmations;
- a way to make Wrasse run a program, or run one with arguments, that isn't in docs/security-audit.md;
- the update check accepting an update that isn't signed with Wrasse's key.

## If Wrasse removed something it shouldn't

That's a bug report, not a vulnerability: open an issue with the **"Wrasse removed something it
shouldn't"** template. First check the tray (bottom right): most removals can be put back from
there.

## Supported versions

Only the latest release gets fixes during the beta.
