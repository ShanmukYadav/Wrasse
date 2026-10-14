# Phase 6 plan: unsigned beta release (macOS)

Status: **proposed 2026-10-14, waiting for the owner's "start".** Release choice (owner): an
unsigned public beta first; Apple Developer signing later. Windows stays deferred.

## Exit criteria

1. A `v0.1.0-beta.1` tag builds a universal `.dmg` (Apple Silicon + Intel) on GitHub Actions and
   attaches it, with the updater files, to a GitHub Release published as a pre-release.
2. The README, repo files and issue templates below are in place.
3. You have run the eight irreversible live tests yourself, each passing.
4. A full test on a fresh macOS user account passes, from the downloaded `.dmg` to removing
   Wrasse.
5. All §8 budgets still pass on the release build (idle measured untouched and after use).

## 0. Idle after use (first, before anything else)

Idle after use is 114–140 MB (budget 100): WebKit's page process keeps memory after any
interaction, and nothing inside the page gives it back (see docs/status.md). The fix is at the
process level (D7), measured with `check-idle-memory.sh --after-use`.

## 1. Build and release (GitHub Actions)

- **Version:** the app stays `0.1.0` inside (macOS bundle versions must be numbers only); the
  tag and release are `v0.1.0-beta.1`, marked pre-release (D1).
- **Universal `.dmg`:** `tauri build --target universal-apple-darwin` on a macOS runner with both
  Rust targets (`aarch64-apple-darwin`, `x86_64-apple-darwin`). Expected about twice today's
  2.6 MB, inside the 15 MB budget; the bundle-size check runs on it.
- **Workflow `release.yml`:** on a pushed `v*` tag: the same checks as CI, then build, then
  `tauri-apps/tauri-action` creates a **draft** release with the `.dmg`, the updater archive and
  its `.sig`, and `latest.json`. You read it and press Publish as pre-release (D2).
- **CI** (`ci.yml`, every push): macOS runs every check and builds the `.dmg`; Windows checks
  that the core crates compile (parity deferred).

## 2. README and repo files

- **README:** what Wrasse does; a short GIF and screenshots of the D1 home map, the tray and
  Free Up Space, taken from the app in mock-data mode so none of your folder names appear (D3);
  install steps for an unsigned app, including the macOS "Open Anyway" step (System Settings ›
  Privacy & Security, after the first blocked open) with a screenshot; Full Disk Access setup;
  what Wrasse never touches (from SECURITY_AUDIT.md); the no-telemetry statement and the only
  network use (the opt-in update check, local Ollama); how to remove Wrasse completely (Settings
  › Remove all Wrasse data, then the app, then `~/Library/Application Support/com.shanmukyadav.wrasse`
  if anything is left).
- **New files:** `SECURITY.md` (how to report a vulnerability privately: GitHub's private
  advisories), `TRADEMARK.md` (the name and fish mark: forks rename), `CHANGELOG.md` (Keep a
  Changelog, starting with 0.1.0-beta.1).
- **Updated:** `LICENSE` (MIT, already there), `CONTRIBUTING.md`, `SECURITY_AUDIT.md` (final pass:
  every program and delegated action in the release, checked by the sync check).
- **Issue templates:** bug report; feature request; **"Wrasse removed something it shouldn't"**
  (marked urgent: what was removed, from where, whether it's still in the tray, and the matching
  audit-log lines, with a note on how to restore from the tray first).

## 3. Auto-updates (opt-in)

- **Off until you turn it on:** a Settings switch, "Check for updates", off by default; the
  first-run screen mentions it (D4). Wrasse makes no network request while it's off.
- **How:** Tauri's updater plugin checks `latest.json` on GitHub Releases at launch (when on) and
  offers the update; nothing installs without a click. Every download is checked against the
  public key built into the app (minisign signatures); a bad signature is refused.
- **Pre-releases:** beta builds follow the beta channel (`latest.json` of the newest
  pre-release); later stable builds only see stable releases (D5).
- **The signing key** is created once with `tauri signer generate` on your Mac. Where it lives:
  - the private key file and its password in your password manager (or the macOS Keychain),
  - a second copy on an encrypted USB drive kept offline,
  - and as two encrypted GitHub Actions secrets (`TAURI_SIGNING_PRIVATE_KEY`,
    `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) so the release build can sign.

  Never in the repo or the project folder; `.gitignore` gets a rule for `*.key` as a guard. If
  the key is lost, installed copies can't update; if it leaks, an attacker could sign an update,
  so it's rotated by shipping a new public key in a manual release.
- **Budgets:** the updater plugin adds a little to the app; the bundle-size and memory checks
  run with it in.

## 4. Live tests (irreversible), run by you

`docs/live-tests.md` becomes a step-by-step guide for each of the eight tests (Homebrew cask
uninstall, Finder Move to Trash for an all-users app, `docker system prune`, `brew cleanup -s`,
snapshot thinning, deleting unavailable simulators, `ollama rm`, Ollama and LM Studio
discovery). Each has: what to install first as a disposable target, the exact clicks in
Wrasse with dry run off, what to check before and after (commands to paste), the audit-log line
to expect, and how to clean up. Homebrew, Docker, Xcode and Ollama aren't on this Mac today, so
each test lists what it needs installed. Nothing in it is run by an agent.

## 5. Final full test on a fresh macOS user account

A checklist you follow on a new standard user account: download the `.dmg` from the release
page; Open Anyway; onboarding and Full Disk Access; first scan; clean a few disposable files
(made by a small script in the guide); tray and restore (checksums match); Free Up Space;
uninstall the dummy app; menu bar and close-to-menu-bar; export; Settings › Remove all Wrasse
data; delete the app; confirm nothing of Wrasse is left. I write the guide and the helper
script; you run it and tell me what you see.

## 6. Publish, then later

- **Publish:** v0.1.0-beta.1 as a GitHub pre-release with release notes (what's in it, that
  it's unsigned, the Open Anyway step, how to report problems).
- **Later, in order (D6):**
  1. Apple Developer ID signing and notarization (Apple Developer Program, $99 a year): no more
     Open Anyway; required before stable 1.0.
  2. Your own Homebrew tap (`brew install --cask shanmukyadav/tap/wrasse`), once signed.
  3. Windows parity (PLAN.md §11), then winget.

## Decisions

**D1. Version naming.** **Recommended:** app version `0.1.0`, tag and release
`v0.1.0-beta.1` (later betas `-beta.2` …). Alternative: put the pre-release in the app version
itself; macOS bundle fields don't accept it.

**D2. Release flow.** **Recommended:** the tag builds a draft; you publish it. Alternative: the
tag publishes the pre-release directly.

**D3. Screenshots.** **Recommended:** mock-data mode, so no real folder or app names from your Mac
appear. Alternative: your real disk.

**D4. Update check default.** **Recommended:** off, with a Settings switch and a line on the first
run screen. Alternative: ask once on the first run.

**D5. Update channels.** **Recommended:** a beta install follows pre-releases; stable installs
follow only stable releases. Alternative: one channel for everything.

**D6. After the beta.** **Recommended:** signing and notarization first, then the Homebrew tap,
then Windows. Alternative: Homebrew tap before signing (users then see the unsigned warning
through Homebrew too).

**D7. Idle after use (over budget today).**
- **Recommended: a quiet restart in the background.** When Wrasse has been in the background
  (another app in front) for 5 minutes and nothing is running (no scan, cleanup or open sheet),
  it hands over to a fresh copy of itself, the same way it hands over to the menu bar, and
  reopens the window in the same place on the home map. Memory goes back to the
  fresh-launch level (about 95 MB). What you'd notice: coming back, you're on the home map,
  not the folder or panel you left open.
- Recycle only WebKit's page process through an undocumented WebKit call: nothing visible
  changes, but it can break with any macOS update.
- Keep the current behavior and set a separate, higher after-use budget (needs your OK; budgets
  are hard limits today).
