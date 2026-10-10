# Wrasse security audit

What Wrasse can and cannot touch on your computer, and how that's enforced. The lists between the
`AUDIT` markers are checked against the code by `scripts/check-security-audit.sh` (CI and the
pre-commit hook), so this file can't drift from what Wrasse actually does.

## How removal works

- **Two entrypoints only.** Every deletion goes through `safety::remove` (Wrasse moves or deletes
  files itself) or `safety::delegate` (the OS does it, e.g. emptying the Trash). Code outside
  `safety/`, `quarantine/` and `platform/` can't delete, rename or start programs;
  `scripts/check-forbidden-apis.sh` fails the build otherwise.
- **Preview, then act.** `safety::prepare` checks every item and reports what would happen, what
  would be skipped and why, and whether it can be undone. `safety::remove` re-runs every check
  right before acting (protected, Never touch list, open, iCloud-only, …) and confirms it's the
  same file (device and file id), not swapped for a link, same owner, and for "your call" items
  unchanged in size and date. Anything that changed is skipped and logged.
- **The tray first.** By default items go to the quarantine tray
  (`~/Library/Application Support/com.shanmukyadav.wrasse/Quarantine`, same disk, so a move is a
  rename). Each move is written to the batch's manifest before it happens and marked done after;
  a half-finished batch is repaired at the next launch. Items stay restorable for 7 days or until
  "Empty now".
- **Not undoable, said up front.** "Delete now" (only for items labeled Safe), "Empty tray" and
  "Empty Trash" always ask first and say "Not undoable".
- **Dry run.** Development builds start every launch in dry run: actions report what they'd do
  and change nothing. With dry run off the DEV badge turns into a red LIVE badge.
- **Audit log.** Every action, skip and failure is appended to
  `~/Library/Application Support/com.shanmukyadav.wrasse/logs/actions.log` (rotated at 10 MB).
- **One operation at a time**, with the Mac kept awake while it runs.

## Never touched

- Anything on a disk other than the startup disk (v1).
- Anything owned by another user, and other users' home folders.
- Items in iCloud only (dataless): there's nothing to free and removing them would remove them
  from iCloud.
- Paths on the user's Never touch list, in both entrypoints: `safety::delegate` refuses to empty
  the Trash while the Trash is on the list.
- Wrasse's own data folder (including the tray).
- App-managed libraries: any path inside a folder ending in
  <!-- AUDIT:library-suffixes -->
  `.photoslibrary`, `.musiclibrary`, `.tvlibrary`, `.imovielibrary`
  <!-- /AUDIT -->

## Protected locations (macOS)

`hard`: nothing at or under it is ever removed. `hand-pick`: the folder itself is never removed;
items inside only when the user picks them by hand, only to the tray (never "Delete now"), and
they're never pre-selected anywhere; if iCloud syncs the folder, the confirmation warns that the
item is removed from the user's other devices too. `folder`: the folder itself is never removed;
what's inside follows the normal rules. The home folder and `/` are `folder` as well, and every
other user's home folder is `hard`.

<!-- AUDIT:macos-protected -->
```
hard /System
hard /usr
hard /bin
hard /sbin
hard /private/var/db
hard /Library/Apple
hard /Users/Shared
hard ~/Library/Keychains
hard ~/Library/Mail
hard ~/Library/Messages
hard ~/Library/Accounts
hard ~/Library/Cookies
hard ~/Library/Group Containers/group.com.apple.notes
hand-pick ~/Documents
hand-pick ~/Desktop
hand-pick ~/Library/Mobile Documents
hand-pick ~/Library/CloudStorage
folder ~/Downloads
folder ~/Movies
folder ~/Music
folder ~/Pictures
folder ~/Public
folder ~/Library
folder ~/Applications
folder /Applications
folder /Library
folder /Users
folder /private
folder /opt
```
<!-- /AUDIT -->

## Junk rules

Bundled rules live in `rules/macos/junk.toml`; each has an explanation shown as "Why is this
safe?" and its own test (what it matches, and that it never matches a protected place). Rules
never offer an app-managed library, even with `**`. A rule is rejected if its schema version is unknown, a root is (or is inside,
or contains) a protected location, a root is a link pointing somewhere protected, a root uses `..`
or isn't absolute, a pattern could reach outside its root, it's marked `keep`, or it names an
unknown delegate. Custom rules (`<data>/rules/custom/*.toml`) are always "your call" and can't
delegate. Rules whose app is open wait until it's closed.

## Programs Wrasse runs (macOS)

Read-only, through `platform::macos::commands::run_readonly`, which refuses anything else:

<!-- AUDIT:read-only-programs -->
```
/usr/bin/pmset
/usr/sbin/lsof
/usr/bin/tmutil
```
<!-- /AUDIT -->

`tmutil` is allowed only as `tmutil isexcluded`. The others each have their own narrowly named
function, with arguments passed as argv (never through a shell):

- `tmutil addexclusion <Wrasse data folder>`: keeps Wrasse's own folder out of Time Machine
  (no admin rights; nothing of the user's is affected).
- `NSRunningApplication terminate` for a bundle id: asks an app to quit before it's uninstalled,
  like the Dock's Quit (the app may ask to save documents). The call returns once the request is
  sent; if it hasn't returned after 10 seconds, Wrasse reports "couldn't ask it to quit" and
  leaves the app running. Never `forceTerminate`, and never a signal to the app.

Delegated actions, reachable only through `safety::delegate`, which validates each before it runs
and logs it (dry run reports them without running):

- `EmptyTrash`: `osascript -e 'tell application "Finder" to empty trash'`. Not undoable. Refused
  while the Trash is on the Never touch list. If Finder hasn't answered after 2 minutes, only
  that `osascript` is stopped (Finder is not); Wrasse reports that it couldn't ask.
- `BrewUninstallCask`: `brew uninstall --cask -- <token>` (no `--zap`: the leftovers go to the tray
  instead). `brew` must be `/opt/homebrew/bin/brew` or `/usr/local/bin/brew` and owned by the
  user; the token must be well formed and that cask's metadata must list the app. Not undoable.
- `FinderMoveToTrash`: Finder moves a root-owned app (App Store, `.pkg`) to the user's Trash and
  asks for the password itself; Put Back works. Same 2-minute bound as emptying the Trash: only
  `osascript` is stopped, so a stuck Automation prompt can't hang Wrasse, and a password dialog
  still has time to be answered.

For both app actions the app must be an `.app` directly in (or one folder inside)
`/Applications` or `~/Applications`, on the startup disk, not protected, not on the Never touch
list, not part of macOS (`com.apple.*` without an App Store receipt), and not running.

<!-- AUDIT:delegated-actions -->
```
EmptyTrash
BrewUninstallCask
FinderMoveToTrash
```
<!-- /AUDIT -->

## Known limitations

- Hard links inside a folder re-read by an incremental rescan may be counted twice until the next
  full scan (sizes only; removal re-checks each item).
- The open-file check (`lsof`) runs only for "your call" files, not inside large cache folders.
- Windows: not implemented yet; the Windows protected list will be added here with its code.
