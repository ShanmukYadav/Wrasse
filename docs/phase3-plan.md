# Phase 3 plan (macOS): Apps, Free Up Space, Large/Old/Duplicate files

Status: **checkpoint done, waiting for approval.** Phase 2 (macOS) was approved on 2026-10-09 after the manual tray test
(~/WrasseTest moved to the tray and restored, all 8 files byte-identical by SHA-256). The user took
the recommendation for all 11 decisions in §8. Windows stays deferred (PLAN.md status note); everything new in `wrasse-core` stays
platform-neutral, with macOS specifics in `platform/macos/`.

Facts checked on this Mac (2026-10-09): /Applications has 8 apps, 5 root-owned and 3 user-owned;
3 App Store apps (`Contents/_MASReceipt`); **Homebrew is not installed**; Spotlight answers
`kMDItemLastUsedDate` (Safari: today).

---

## 1. Steps (each one commit, prefixed "Phase 3:")

1. App model and discovery (read-only): sources, ownership, true size, last opened.
2. Uninstall flows: quit first, the three removal routes, leftovers to the tray.
3. Orphaned leftovers.
4. Ranker in `wrasse-core` and Free Up Space on real data.
5. Large and old files, then duplicates.
6. UI wiring into D1.
7. Checkpoint: tests plus a dummy-app check on your Mac, then stop and report.

## 2. App discovery

New module `crates/wrasse-core/src/apps/` (neutral model) plus `platform/macos/apps.rs` (Info.plist,
receipts, casks, Spotlight). The scan's breakdown uses it instead of its own `claim_apps`.

```
App { id (bundle id), name, bundle path, version,
      source: Folder | Cask { token } | AppStore | System,
      owner: User | Root, running, last_used: Option<days>,
      bundle_bytes, leftovers: Vec<Leftover>, true_bytes }
Leftover { path, kind, matched_by: BundleId | Name, shared_with: Vec<app id>, needs_admin }
```

- **Where:** `/Applications`, `~/Applications`, and one folder level inside each (e.g.
  "Adobe Creative Cloud/"), as today. `/System/Applications` is listed nowhere (part of macOS).
- **Homebrew casks:** read, never run `brew`, for discovery. Look in `/opt/homebrew/Caskroom` and
  `/usr/local/Caskroom`. For each `<token>/<version>`, the cask's saved metadata (`.metadata/…/
  Casks/<token>.json` or `.rb`) names its `.app` artifacts, and the matching bundle is tagged
  `Cask { token }`.
- **App Store:** `Contents/_MASReceipt/receipt` exists → `AppStore`.
- **Part of macOS:** `com.apple.*` without an App Store receipt (Safari etc.) → `System`, shown but
  never removable.
- **Owner:** owner uid of the bundle: yours → `User`, root → `Root` (App Store and `.pkg` installs).
- **True size:** the bundle's scanned size (clones and hard links already handled) plus leftovers.
  Leftover places (PLAN.md §6.4) under `~/Library`: Application Support, Caches, Containers,
  Group Containers, Preferences, Saved Application State, Logs, HTTPStorages, WebKit, LaunchAgents.
  - Matched by **bundle id** (exact name, `<id>.plist`, `<id>.savedState`, group containers named
    `group.<id>` or `<TEAMID>.<id>`): counted in the true size.
  - Matched by **app name** only (e.g. `Application Support/Spotify`): counted, but marked "your
    call". If two installed apps match the same folder (say a vendor folder like "Google"), it is
    marked "Shared with …" and never ticked.
  - `/Library/{Application Support, Caches, LaunchAgents, LaunchDaemons, PrivilegedHelperTools}`
    matches: shown as "Needs admin", display only, not in the true size.
- **Last opened:** Spotlight `kMDItemLastUsedDate` through CoreServices (`MDItemCreate` +
  `MDItemCopyAttribute`) in `platform/macos`: one in-process call per app, no program started.
  When Spotlight has no date: "Never opened" if the app was added more than 30 days ago, else
  "Unknown". See decision 1.
- **Running:** by executable path (as today) and bundle id.

## 3. Uninstaller

**Quit first.** If the app runs, the sheet shows "Quit <App>" and nothing else can be confirmed.
Quitting is a polite request (`NSRunningApplication terminate` for that bundle id, the same quit
event as the Dock, via `platform/macos/commands.rs`). It returns once the request is sent. A stuck
call is given up after 10 s ("couldn't ask it to quit"). Never `forceTerminate`, never a signal.
Wrasse then waits up to 10 s and, if the app is still open (unsaved work prompt), says so and stops.
`safety` re-checks at the last moment that no process runs from the bundle, so an app reopened in
between is skipped (`Reason::AppRunning`). Finder's AppleScripts (empty Trash, Move to Trash) have
their own 2-minute bound; only the `osascript` process is stopped. A stop at that limit is a
failure, never success. Emptying the Trash says "Emptying the Trash stopped after 2 minutes and
may be partly done." Finder Move to Trash says "Moving it to the Trash stopped after 2 minutes
and may be partly done."

**Three routes, chosen by source and owner:**

| App | Bundle goes | Undo | How |
|---|---|---|---|
| Homebrew cask | removed by Homebrew | Not undoable | `safety::delegate(BrewUninstallCask { token })` |
| Yours (dragged in) | the tray | Restore | `safety::remove` (quarantine) |
| Root-owned (App Store, `.pkg`) | your Trash | Put Back from Trash | `safety::delegate(FinderMoveToTrash { path })`; Finder asks for your password |
| Part of macOS | stays | n/a | no Uninstall button |

- **Brew:** `brew` must be `/opt/homebrew/bin/brew` or `/usr/local/bin/brew` and owned by you. The
  token must be a folder in the Caskroom and match `[a-z0-9][a-z0-9@._+-]*`. It runs as
  `brew uninstall --cask <token>` (argv, no shell) with `HOMEBREW_NO_AUTO_UPDATE=1`, and **without
  `--zap`**: zap deletes leftovers for good, while Wrasse sends them to the tray itself (decision 2).
- **Finder Move to Trash:** the path must be a `.app` directly in (or one folder inside)
  `/Applications` or `~/Applications`, not `System`, and the same file that was planned.
- **Leftovers** (every route): the ticked ones go to the tray through `safety::remove`. By default,
  bundle-id matches are ticked and name-only or shared matches aren't (decision 4); "Needs admin"
  rows are greyed.
- `Delegated` gains the two variants. Bundled rules still may only delegate `empty-trash`, and
  custom rules still can't delegate. SECURITY_AUDIT.md lists the new commands, and the allowlist
  tests cover both variants plus injection attempts (`foo; rm`, `../x`, `--zap`, paths outside the
  app folders). Tests can't reach a real delegated action (`cfg(test)` stop, as for the Trash).

**Orphaned leftovers** (step 3): folders in the `~/Library` leftover places whose name is
reverse-DNS (3+ parts) and that meet all of these:
- no discovered app has that bundle id;
- Spotlight finds no app with it anywhere (apps outside /Applications, external disks, Setapp);
- not `com.apple.*`;
- not modified for 30+ days.

They're shown as one "Leftovers from removed apps" tile inside Apps, each item "your call" and never
ticked (decision 5).

## 4. Free Up Space on real data

The ranker moves from `store/app.ts` into `crates/wrasse-core/src/ranker.rs`, so the app, the CLI
(`wrasse free 10GB --dry-run`) and the tests share it. It's source-agnostic: each source hands it
candidates.

```
Candidate { path(s), bytes (reclaimable), last_used_days, safety, source, label, reason }
score = reclaimable_bytes × unused_factor × safety_weight
unused_factor: 0 days → 0.1, 30 → 0.5, 180+ → 1.0 (linear between)
safety_weight: safe 1.0, your call 0.7; keep excluded
```

- **Sources in Phase 3:** junk rule matches (caches use the owning app's last use), whole apps (the
  bundle plus ticked leftovers as one candidate; last opened from Spotlight), large/old files, and
  orphaned leftovers. Duplicates come later as "your call".
- **Reclaimable bytes** exclude clone-shared and iCloud-only bytes (the scan already knows both).
- **Never offered:**
  - Documents/Desktop/iCloud Drive items (hand-pick only);
  - Keep items, protected or never-touch paths, and held rules (app open);
  - running apps and anything used in the last 7 days (decision 8).
- **Not pre-ticked:** a file or model whose age on the map is a scan date older than a year
  (`last_opened_days` > 365) stays in the plan and starts unticked. Select all still ticks it.
  Apps, orphans, junk and anything used within the year are unchanged.
- **Shortfall:** if the target can't be reached without things used this month, the meter says so
  and shows how much is missing instead of offering them.
- **Two numbers**, as PLAN.md §6.5 asks:
  - **Freed now:** bytes actually gone (Delete now, delegated removals, emptied tray or Trash).
  - **Pending in tray:** bytes moved to the tray, which free up only when it's emptied.

  When the target depends on the tray, an "Empty tray now" button appears, marked Not undoable.
  The disk's real free space is shown beside them (decision 9).

## 5. Large, old and duplicate files

**Large and old** come from the scan tree, with no extra walk:
- **Large:** files of 500 MB or more (Settings: 100 MB to 5 GB).
- **Old:** not opened in over a year, by Spotlight last-used, else the newest of mtime and atime,
  labeled "approx".

The scan tree stores each file's modification time in seconds (`WRSTREE3`; older tree caches are
ignored until the next scan). Last-used day stays the newest of mtime and atime. Files of 1 MB or
more are their own nodes; smaller files stay folded into their folder. Before each move, Wrasse
compares the live allocated size and modification time with that scan record and skips a file
that changed ("changed since the scan"). Nothing in the list starts ticked, including a row whose
date says "approx". Documents and Desktop open the full caution sheet (the list, the iCloud
warning, "I understand") for the whole selection. A selection that is only in iCloud Drive keeps
the one-line note. Type filters: video, archive, disk image, model, other (by extension and UTI).
The model type is not the extension alone: `.gguf` and `.safetensors` must carry their header, and
`.bin` and the other ambiguous extensions need a known header or a `config.json` beside the file.

**Duplicates** use size → partial hash → full hash:
1. Group files of **10 MB or more** (decision 6) by exact size from the tree.
2. Partial BLAKE3 over the first, middle and last 64 KB.
3. Full streaming BLAKE3, only for groups that still match.

It runs on the utility-QoS pool, cancellable and power-paused like the scan, and the reads are
throttled. Skipped:
- same `(dev, ino)` (a hard link isn't a copy);
- APFS clones (they share blocks, so removing one frees nothing; shown as "Clone · frees nothing");
- iCloud-only files;
- the insides of packages (`.app`, `.photoslibrary`, …).

**You pick which copy to keep.** Each group is a card listing every copy with its place and dates;
nothing is ticked until you choose. Copies in Documents, Desktop, iCloud Drive or other protected
places are never suggested for removal. If every copy is in such a place, the card only shows them.
Right before removing, Wrasse re-hashes the kept copy; if it changed or is gone, nothing in that
group is removed. Removal goes to the tray.

## 6. How it fits D1 (Reef Blocks)

No new transitions: crossfade only, and removed items fly to the tray as today.

- **Apps tile / lens** (exists): app tiles show size and a meta line like "Opened 3 months ago ·
  Homebrew", with chips for Running, App Store and "Part of macOS" (greyed, no tick). Drill-in mini
  tiles (bundle, Caches, Support, …) are now the real leftovers, and "Needs admin" ones are dashed.
  A "Leftovers from removed apps" tile sits inside Apps.
- **Detail panel:** Uninstall (with the route in plain words: "Removed with Homebrew · Not
  undoable", "Finder will ask for your password · Goes to your Trash", "Goes to the tray"),
  Remove leftover, Reveal in Finder.
- **Uninstall sheet** (exists): the Quit button first, the checklist with defaults from decision 4,
  greyed Needs-admin rows, a Not undoable chip for Homebrew, and the dry-run note in dev builds.
- **Free Up Space** (command bar PlanMeter, exists): fed by the core ranker. The meter has two
  segments, freed now (solid) and pending in tray (striped), plus the shortfall line and "Empty
  tray now".
- **Large & Old lens** (exists, dims tiles): adds a list overlay with the size slider, type chips
  and age, plus a Duplicates section of group cards. The duplicate chip on tiles stays.
- **Caution sheet:** reused for Delete now, OS folders and Keep data. Large & Old files in
  Documents or Desktop use that same sheet. iCloud Drive files, and duplicate copies, keep the
  one-line note.

## 7. Checkpoint tests (dummy apps only, never real apps)

Fixtures in `wrasse-fixtures` build everything in a temp folder with an injected apps directory.
Tests never read or write the real `/Applications`, `~/Applications` or Homebrew, and the
`remove_test_*` guards stay in place.

- `mac_apps(home, apps_dir)`: fake `.app` bundles (Info.plist with `com.wrasse-test.<name>`, a stub
  executable), an App Store one (`_MASReceipt`), a "system" one (`com.apple.*`), a fake Caskroom
  with metadata, and leftovers matched by id, by name, and shared.
- **Discovery:** each source and owner is detected; true size = bundle + id/name leftovers;
  shared and Needs-admin leftovers are handled as above.
- **Uninstall, yours:** bundle and ticked leftovers go to the tray; restore is byte-identical.
- **Uninstall, cask:** routes to `BrewUninstallCask` with the exact argv; bad tokens are refused;
  never executed.
- **Uninstall, root-owned:** ownership is injected; routes to `FinderMoveToTrash`; never executed.
- **Running:** a process started from the dummy bundle's executable blocks uninstall at preview and
  at the last moment; quitting goes through a fake.
- **Orphans:** a leftover with no app is found; with an app, it isn't; `com.apple.*` and recently
  modified ones are never found.
- **Ranker:** on a synthetic tree it reaches a 10 GB target. It never offers hand-pick-only,
  running, keep or protected items, orders by score, reports the shortfall, and splits freed now
  from pending.
- **Duplicates:** identical files are grouped; same size with a difference outside the sampled
  ranges passes the partial hash but fails the full hash; hard links, clones and iCloud-only files
  aren't duplicates; a kept copy that changed blocks its group.
- **On your Mac** (like the Phase 2 dummy check): a user-owned `~/Applications/WrasseDummy.app`
  with leftovers in `~/Library/Caches/com.wrasse-test.dummy` etc.
  - Uninstall in the app; everything goes to the tray; restore is byte-identical.
  - The Homebrew and Finder routes are shown in dry-run only (decision 11).

## 8. Decisions needed

Each has a recommendation; reply with the numbers you want changed, or "start" to take them all.

1. **Last opened from:** Spotlight through CoreServices, in-process. *Recommended:* fast (one call
   per app), no programs started. Alternatives: `mdls` per app (adds a read-only program,
   ~20 ms × apps), or bundle access time (unreliable).
2. **Homebrew leftovers:** `brew uninstall --cask` without `--zap`, and Wrasse sends leftovers to
   the tray. *Recommended:* leftovers stay restorable. Alternative: `--zap` (permanent).
3. **Root-owned apps:** Finder "Move to Trash" (Finder asks for the password; Put Back works).
   *Recommended.* Alternative: show them as "Needs admin" only.
4. **Uninstall default ticks:** the bundle and bundle-id leftovers are ticked; name-only and shared
   ones aren't. *Recommended.* Alternative: tick everything found.
5. **Orphans:** strict detection (reverse-DNS name, no app anywhere per Spotlight, 30+ days
   untouched, never `com.apple.*`) and also offered in Free Up Space as "your call".
   *Recommended.* Alternative: show only, never in Free Up Space.
6. **Duplicates:** minimum size 10 MB, hashed with BLAKE3 (new dependency `blake3`, pure Rust).
   *Recommended:* 10 MB keeps the hashing quick and covers where the space is. Alternatives: 1 MB;
   SHA-256 (already on every Mac, slower).
7. **"Old":** not opened in over a year by Spotlight, else mtime/atime labeled "approx".
   *Recommended.*
8. **Free Up Space leaves out** running apps and anything used in the last 7 days. *Recommended.*
   Alternative: include running apps with "Quit first".
9. **"Freed now":** counted from what Wrasse actually removed, with the disk's live free space
   beside it. *Recommended:* the live number alone moves with other apps' activity.
10. **Quitting:** a polite quit, a 10 s wait, never force. *Recommended.*
11. **Homebrew on this Mac:** it isn't installed, so the cask route is checked by tests and dry-run
    only. *Recommended:* don't install Homebrew for this. Alternative: you install it and a dummy
    cask from a local tap for a live check.

## 9. Release checklist

Before release, live-test the Homebrew cask uninstall route and the root-owned Finder Move to
Trash route. Both are unit-tested only. Homebrew is not installed on this Mac; the Finder route
needs an administrator password.
