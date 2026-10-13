# Wrasse frontend (Stage A, frozen as `frontend-v1`)

The frontend is a React 19 + TypeScript app in `app/src`, styled in the locked **D1 "Reef Blocks"** design (day reef, light theme first). In the browser it runs on mock data; in the Tauri app it runs on the real scanner (Phase 1, read-only).

- Browser (mock data): `cd app && ../dev.sh npm run dev`, open http://localhost:1420.
- App (real data): `cd app && ../dev.sh npm run tauri dev`. Command bar › "Use mock data (dev)" switches the app to mock data for UI work (stored in `localStorage["wrasse.mock"]`); "Use real data (dev)" switches back.
- `WRASSE_AUTOSCAN=1` (dev builds) skips onboarding and scans at launch, for test runs.

## Rules

- **All data and actions go through `src/api/`.** Components and the store import `api` and the shared types from `src/api`; nothing outside `src/api` and `src/mock` imports `src/mock`.
- **Every visual value lives in `src/design/tokens.ts`** (colors, gradients, radii, shadows, sizes) or `src/motion/tokens.ts` (durations, easings, springs). `src/design/apply.ts` publishes them as CSS variables (`--c-*`, `--d1-*`, `--d-*`).
- **Animate only transform and opacity.** Respect reduced motion (system setting or Settings › Motion).
- **Component files export only components**, so hot updates stay local. Plain helpers live in `.ts` files.
- **The first scan runs once per page load** (`boot()` in `store/app.ts`), not per React mount.

## Screens and flows

| Screen / flow | Where | What it does |
|---|---|---|
| **Home map** | `features/home/Home.tsx`, `Tile.tsx`, `gridLayout.ts` | One level of the disk on a 12×8 grid, at most 12 tiles (the rest fold into "+N more"). Tile area follows the square root of size, the biggest is capped at 40% of the grid, and every tile gets a minimum (6 cells, fewer when there are many tiles); the grid is cut on whole cells, choosing the cuts that keep tiles closest to their fair size and to square. Tiles show badge, name, meta, big size, top rows, insight chips and "Open →" / "Details". Every top-level block, including Junk, shows its share of the disk at the top right ("18% · 4 items"); a short block shows the percent beside the size. Small tiles degrade to a one-line name + size. |
| **Drill-in** | `Home.tsx` (`Level`) | Opening a folder, category or app crossfades (0.18 s) to its level: the category-tinted frame with its children as tiles. No zoom, sweep or staggered reveal: the user found them distracting. Back, Esc and the breadcrumb go up; single-child folders are skipped. When everything in a level is removed, the map goes up to the nearest level that still has items. |
| **Apps drill-in** | `mock/disk.ts`, `DetailPanel.tsx` | An app opens into mini tiles: the `.app` bundle, Caches, Application Support, Containers, Logs, Saved State, Preferences, with a "Running" chip when the app is open. |
| **Onboarding** | `features/onboarding/Onboarding.tsx` | First run: what Wrasse does, no telemetry, Full Disk Access with the real status (re-checked when the window regains focus) and a button to System Settings, then "Scan this Mac". Command "Permissions" reopens it. |
| **Sonar scan** | `features/home/Sonar.tsx` | Two rings sweep by growing from 8vmax to 160vmax (the border fades) inside the scan area, plus the Wrasse mark, 24 bubbles, and live progress (items, bytes, current folder). The field clips the rings; they are not scaled. Mounted only while a scan runs and repeats until it ends; reduced motion shows only a fade. Tiles re-reveal with one last sweep when results land. Scan becomes Stop while scanning; "Pause scan" / "Resume scan" in the command bar. On battery below 20% or in Low Power Mode the scan pauses by itself (checked every 10 s) and the readout says why, with Resume scan; resuming keeps it going for the rest of that scan. After the first scan, Scan re-reads only the folders FSEvents reports as changed (`summary.incremental`, `summary.changedFolders`). |
| **Lens bar** | `features/browse/LensBar.tsx`, `features/browse/lenses.ts`, `features/browse/scope.ts` | All, Junk, Apps, AI Models and Dev are categories: one is selected, and choosing one opens that category (All goes home). A filter button sits after a divider. Large & Old and Grew show only while that button is open. Each toggles on its own (outline off, accent fill on) and both can be on together. The button fills with the accent color while a filter is on, and while that menu is closed it reads "Filters · 1" (or "Filters · 2"). In dark mode the bar, the filter menu and the selection bar use the dark glass, so All, Junk, Apps, AI Models and Dev stay readable. Escape or a click elsewhere closes the menu. Switching category keeps the filters, and turning a filter on stays on the current folder. Active filters show in the breadcrumb as removable chips. The list button stays on the bar: "List & duplicates →" while no filter is on, and a filled accent button "See N items · size →" while a filter is on. The count and size are the direct children of the current folder that match the category and the filters, the same rows the list shows (`visibleRows` / `buttonCount`). When that scope is empty the button reads "Nothing here" and stays disabled, and the folder shows a short empty state with "Show all of Macintosh HD". That link keeps the active filters and only widens the view to the whole disk, so Grew stays on and shows where things grew. Turning a filter on plays the button glow once when the scope has rows, unless Reduce Motion is on. The button opens the Large & Old overlay when Large & Old is on or no filter is on, and opens the list when only Grew is on. One navy pill slides among the categories and follows the category you open. |
| **Command bar** | `features/command/CommandBar.tsx`, `features/command/commands.ts` | ⌘K or type anywhere. Fuzzy commands: free N GB, where is <name>, why isn't this free, lenses, scan, list/map view, zoom out, home, empty/restore tray, overlays, design system. |
| **Free Up Space** | `CommandBar.tsx` (`PlanMeter`), `features/cleanup/planReason.ts`, core `rank/` (mock: `mock/rank.ts`) | "free 30 GB" asks the core ranker (size × unused time × safety over the last saved scan, minus what was removed this session). It never offers Documents/Desktop/iCloud Drive, Never touch, Keep, "Smaller items", open apps or anything used in the last 7 days. A file or model starts unticked only when its date is approximate (`approx`: file dates, not a known last-used date) and over a year (`lastOpenedDays` > 365). A known date over a year stays ticked. Select all still ticks an unticked row. Apps count whole (bundle plus the leftovers ticked by default); apps Homebrew or Finder remove show "Uninstall…" instead of a tick. Homebrew cleanup, Docker prune and unavailable simulators show "Clean up" and confirm on their own; they are not moved to the tray. The meter shows freed for good (solid), pending in the tray (striped) and picked (light); "Empty tray now · Not undoable" appears when the target depends on the tray. When the target can't be reached it says how much is missing and how much sits in things used this week or open apps. |
| **Detail panel** | `features/browse/DetailPanel.tsx` | 330 px glass panel (dark glass at night, the same surface as the other panels): badge, name, size, label chip (Safe / Your call / Keep / Duplicate · Your call), location, last used, category, explanation. Actions by type: Uninstall / Remove app only (bundle), Remove (leftover), Move to quarantine, Delete now (safe items only), Open, Reveal in Finder. |
| **Select and delete** | `features/home/Tile.tsx` (tick), `features/cleanup/SelectionBar.tsx`, `features/cleanup/CautionSheet.tsx`, `src/api/caution.ts` | Hovering a block shows a tick; once anything is ticked every block shows one and a bar replaces the lens bar ("N selected · size", Delete, Clear; Esc clears). Delete, there or in the detail panel, moves items to the tray (restorable). OS folders (Desktop, Documents, Downloads, Movies, Music, Pictures, Library, Applications on macOS; the profile folders, Program Files and Windows on Windows) and Keep data first show a caution sheet listing what goes, with an "I understand" tick before "Delete anyway". Whole home categories (Your Files, Apps, AI Models, Developer, Library & caches) have no tick and can't be deleted in one go ("Open it to choose what to delete"); macOS itself, System items, snapshots and app-managed libraries can't be ticked or deleted either. |
| **Uninstall flow** | `features/apps/UninstallSheet.tsx`, `features/browse/DetailPanel.tsx` (`AppActions`) | The detail panel shows where the app came from (copied in, Homebrew, App Store, part of macOS), its version and how it goes. The sheet lists the bundle and its leftovers with sizes: leftovers tied to the bundle id start ticked; ones matched only by the app's name, or shared with another app, start unticked and say so; "Needs admin" items in the system Library are listed greyed. "Quit {app}" asks the app to quit (never forced; the request gives up after 10 s, then Wrasse waits up to 10 s for it to close) and nothing else can be confirmed while it runs. The button follows the route: "Uninstall" (bundle to the tray), "Uninstall with Homebrew" (Not undoable), "Move to Trash" (Finder asks for the password; Put Back works). Apps that are part of macOS have no Uninstall. Toasts say what happened; the tray route offers Undo. |
| **Leftovers from removed apps** | `crates/wrasse-core/src/platform/macos/apps/mod.rs` (`orphans`), Apps category | A tile inside Apps with data folders whose app is gone (bundle-id names, no app of that vendor installed or known anywhere, untouched 30+ days, never Apple's). Each is "your call", chip "App removed". |
| **Large & Old** | `features/large-old/LargeOld.tsx`, `features/cleanup/CautionSheet.tsx`, core `files/` | Overlay (⌘K "large files", "duplicates", or the lens bar). Large files: the biggest files in the home folder of at least 100 MB to 5 GB (remembered), with last use from Spotlight (or "approx" from file dates), "Not opened in a year" and type filters. Nothing is ticked for you, including a row marked "approx". A ticked row shows a check on a teal box and a light teal tint, and the note changes to how many you ticked. The same check is used in Free Up Space, System Junk and uninstall leftovers; in Duplicates the copy you keep uses it. Keyboard focus outlines the box, and Space toggles it. Files in Documents or Desktop open the full caution sheet (list, iCloud warning, "I understand") before the move; a selection that is only in iCloud Drive keeps the one-line note. Each move re-checks the scan's size and modification time and skips a file that changed. Ticked files go to the tray with Undo. Duplicates: files of 10 MB or more grouped by size, a sampled hash, then a full BLAKE3 hash, with progress and Stop. Hard links aren't copies; APFS clones say "frees nothing"; personal-folder copies are marked. You pick the copy to keep (nothing preselected); the others go to the tray after every copy is hashed again, with the one-line note when a copy is in a personal folder. The tray itself has no row checkbox: each item has Restore. |
| **Quarantine tray** | `features/cleanup/QuarantineTray.tsx`, `features/cleanup/Flyers.tsx`, `shared/Toast.tsx` | Always in the bottom-right corner (glass button): "Tray empty", or "N items" and the size. Removed tiles fly into it. Clicking it opens the tray panel above it: every batch (when it was moved, item count, size, days left before it empties by itself), its items with Restore each, Restore all per batch and for the whole tray, History, and Empty now (marked Not undoable). Esc or a click elsewhere closes it. Every move to the tray shows a toast "Moved N items (X GB) to the tray · Undo"; Undo, ⌘Z / Ctrl+Z and ⌘K "undo" put back the last move. ⌘K "tray", "quarantine" or "restore" opens the panel, and History has "Open the tray". In dry run the toast says nothing moved and offers "Turn off dry run". On windows narrower than 1120 px the lens bar shifts left so it never overlaps the tray. |
| **Snapshots ghost** | `features/home/GhostTile.tsx`, `features/home/SpaceReport.tsx` | Dashed tile when local Time Machine snapshots are holding space, with "Free now" (confirms first; macOS asks for a password only if the unprivileged thin fails). After a cleanup, and from the command "Why isn't this free?", a dashed card shows expected versus actual free space, purgeable space, the Trash and the tray, each with its action. |
| **List view** | `features/browse/ListView.tsx` | The focused folder's direct children as rows, narrowed to the active category and filters. When that scope is empty, a short note and "Show all of Macintosh HD", which keeps the filters and widens to the whole disk. Back returns to the map of the same folder. |
| **History** | `features/settings/Overlays.tsx` (History), store `deleted` | Everything deleted this session, newest first: category-colored icon, name, kind (App, App data, AI model, Developer, Cache, Folder, File, Snapshots) in its category color, full path, size, time, and status (In tray · restorable with Restore, Restored, Deleted). Filters All / In tray / Deleted / Restored. Opens from the clock button in the header (after the first delete), the button on the tray, or the command bar ("history", "deleted"). |
| **Overlays** | `features/settings/Overlays.tsx` | Startup items, Settings (theme, motion), Audit log (every action with path and size), Quarantine history. |
| **Design page** | `features/design-page/DesignPage.tsx` | Hidden (command "design system"): brand mark, icons, palette, chips and real tiles in both themes. |
| **Toast** | `shared/Toast.tsx` | Short confirmations ("Tray emptied", "Revealed in Finder"). |
| **Themes** | `design/apply.ts`, `lib/prefs.ts` | Light (day reef) is the default and approved direction. Dark (night) values exist but are provisional until light is approved. |

**Not built yet in D1**: empty / error / "good shape" states, and a dedicated AI-model duplicates view. Duplicates show today as a chip and a detail label.

**Read-only phase:** while `appInfo().readOnly` is true (Phase 1), cleanup actions show a toast ("This version only scans…") instead of acting.

## Components

| Area | Files |
|---|---|
| App shell | `App.tsx` (backdrop, header, map, list, lens bar, panels, overlays, Esc handling), `main.tsx` |
| Home | `features/home/Home.tsx` (Home, Level, MoreTile), `Tile.tsx` (Tile, Row, SizeBar), `GhostTile.tsx`, `Header.tsx` (traffic lights, breadcrumb/back, command bar, free space, Scan), `Sonar.tsx`, `MiddleEllipsis.tsx`, `gridLayout.ts`, `nodes.ts` (isDrillable, lensMatch) |
| Panels and bars | `features/browse/LensBar.tsx`, `features/command/CommandBar.tsx` (CommandBar, PlanMeter), `features/browse/DetailPanel.tsx`, `features/apps/UninstallSheet.tsx`, `features/cleanup/QuarantineTray.tsx`, `features/browse/ListView.tsx`, `features/settings/Overlays.tsx`, `shared/Toast.tsx`, `features/cleanup/Flyers.tsx`, `features/cleanup/Safety.tsx`, `features/design-page/DesignPage.tsx` |
| Shared | `shared/Bytes.tsx` (animated byte count), `shared/DevBadge.tsx`, `design/icons/Icon.tsx` + `nodes.ts` + `semantic.ts` (duotone icon set, iconFor/toneFor), `design/brand/BrandMark.tsx` |
| State | `store/app.ts` (zustand: focus, lens, selection, tray, batches, audit, plan, flights, scan, prefs, uninstall; all actions) |
| Text | `i18n/en.ts`, `i18n/index.ts` (`t(key, vars)`, plurals with `one|other`) |

## Design tokens (D1 "Reef Blocks")

Source of truth: `app/src/design/tokens.ts` and `app/src/motion/tokens.ts`. Light values below; dark values are in the same file.

### Colors

| Token | Light |
|---|---|
| canvas | `#EEF5FA` |
| backdrop | `linear-gradient(160deg, #F2FBFA 0%, #EAF4FB 60%, #EEF0FB 100%)` |
| text / textMuted | `#0B1A2E` / `#5B6B7E` |
| accent / accentHover | `#0F766E` / `#0B5A54` |
| navy (active lens pill) | `#0B1A2E` |
| safe / yourCall / keep | `#166534` / `#8A4B07` / `#334155` |
| focus ring | `#2F5BD3` |
| glass / glassStrong / glassBorder | `rgba(255,255,255,.75)` / `rgba(255,255,255,.88)` / `rgba(11,26,46,.08)` |

**Categories** (accent · ink · gradient from → to):

| Category | Accent | Ink | Gradient |
|---|---|---|---|
| Your Files | `#0F766E` | `#0B4F4A` | `#E3F6F3 → #CDEDE8` |
| AI Models | `#6D4FD8` | `#3F2A9E` | `#F0ECFE → #E2DAFC` |
| Developer | `#0E7490` | `#0B4C5E` | `#E4F4FA → #CFEAF4` |
| Apps | `#2F5BD3` | `#1E3F99` | `#E8EFFE → #D6E2FC` |
| System | `#475569` | `#2B3646` | `#EEF1F5 → #E1E6EE` |
| Junk | `#B45309` | `#7A3A06` | `#FEF6E7 → #FCE9C6` |
| Other | `#475569` | `#2B3646` | `#F1F4F8 → #E6EBF1` |

Each category also has `border`, `glow` (top-right circle) and badge `shadow` values.

**Chips** (bg / text): amber `#FCE7C3 / #8A4B07` (attention), green `#D3F2DE / #166534` (safe), violet `#E2DAFC / #4C2DB0` (AI), protected `#E1E6EE / #334155`.

**Lens colors:** junk `#B45309`, apps `#2F5BD3`, aiModels `#6D4FD8`, dev `#0E7490`, largeOld `#C2410C`, grew `#15803D`.

### Radii

| Element | Radius |
|---|---|
| Tile | 20 px |
| Opened-tile frame | 14 px |
| Ghost tile | 16 px |
| Lens bar / lens pill | 18 px / 12 px |
| Command bar, buttons | 12 px |
| Row | 10 px |
| Icon badge | 10 px |
| General (macOS) | sm 8, md 12, lg 20, bar 18, pill 999 |

### Shadows and blur

- Tile: `0 1px 0 rgba(255,255,255,.9) inset, 0 10px 30px rgba(11,26,46,.07)`; hover: `… 0 18px 40px rgba(11,26,46,.13)` and a 3 px lift.
- Lens bar / glass panels: `0 10px 30px rgba(11,26,46,.12)`. Command bar: `0 4px 18px rgba(11,26,46,.06)`.
- Pill: `0 4px 12px rgba(11,26,46,.10)`. Flyer: `0 10px 26px rgba(11,26,46,.22)`.
- Glass blur: `saturate(160%) blur(20px)`, on glass elements only.
- Dimmed tiles (lens miss): opacity 0.35.

### Layout and sizes

- Header 72 px; grid 12 columns × 8 rows, gap 12, side padding 24, bottom padding 92 (room for the lens bar); detail panel 330 px.
- Tile size number: XL 44, L 38, M 32, S 24 px; tiles under 100 px tall go one-line; chips need 150 px height and width.
- Rows: 38 px tall, size bar 54 × 5 px.

### Type

- Font: `-apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable", "Segoe UI", sans-serif`.
- Scale (px): caption 12, label 13, body 15, heading 17, title 22, display 28, hero 40, giant 64.
- Weights: regular 400, names 500, sizes 600, strong 700. Hints at 70% opacity.
- Icons: duotone, 24-px grid, stroke 1.75, fill opacity 0.2, sizes 16 / 20 / 24 / 32.

### Motion

| Token | Value |
|---|---|
| ease.out (entering) | `[0.22, 1, 0.36, 1]` |
| ease.inOut (exits, crossfades) | `[0.65, 0, 0.35, 1]` |
| duration fast / base / slow | 0.18 s / 0.26 s / 0.4 s |
| spring.soft | stiffness 260, damping 30, mass 0.9 |
| spring.snappy | stiffness 420, damping 34 |
| spring.gentle | stiffness 140, damping 22 |
| stagger | 0.035 s |
| sonar sweep | 2.5 s |
| bubble rise | 1.8 s |
| "where is" pulse | 1.5 s |
| tray flight | 0.7 s |
| candidate stagger | 0.045 s |

## API (`src/api/`)

`src/api/index.ts` exports `api: WrasseApi` and re-exports everything in `src/api/types.ts`. In the Tauri app `api` is `tauriApi` (`src/api/tauri.ts`, commands in `app/src-tauri/src/scanning.rs`); in the browser, or with mock mode on, it is `mockApi` (`src/mock/api.ts`). Insight chips (`src/api/insights.ts`) are shared.

### Functions

| Function | Input | Output | Notes |
|---|---|---|---|
| `appInfo()` | none | `Promise<AppInfo>` | `{ version, platform, fileManager, units: "decimal" \| "binary", dev, bench, readOnly, autoScan? }` |
| `scan()` | none | `Promise<Disk>` | Whole system volume as a tree. Emits `ScanProgress` while it runs. A cancelled scan resolves with what was read (`summary.cancelled`). |
| `cachedScan()` | none | `Promise<Disk \| null>` | The last finished scan, saved in `~/Library/Application Support/com.shanmukyadav.wrasse/scan-cache.json`; shown at launch. |
| `cancelScan()` | none | `Promise<void>` | Stops the running scan. |
| `pauseScan(paused)` | boolean | `Promise<void>` | Pauses or resumes the running scan. |
| `fullDiskAccess()` | none | `Promise<boolean>` | macOS Full Disk Access (reads a TCC-protected folder). |
| `openFullDiskAccessSettings()` | none | `Promise<void>` | Opens System Settings › Privacy & Security › Full Disk Access. |
| `onScanProgress(listener)` | `(p: ScanProgress) => void` | unsubscribe `() => void` | Event stream for the scan (Stage B: at most 10 per second). |
| `insights(disk, id)` | tree, node id | `Insight[]` | Chips for a tile, e.g. "1 duplicate · 39 GB", "31 GB rebuildable", "Running", "All safe". At most 2. |
| `preview(disk, ids, opts)` | tree, node ids, `{ handPicked, deleteNow? }` | `Promise<RemovePreview>` | What removing would do: bytes, undoable, iCloud-synced items, skips with reasons. Touches nothing. |
| `remove(disk, ids, opts)` | tree, node ids, `{ handPicked, deleteNow? }` | `Promise<RemoveOutcome>` | To the tray (or Delete now for Safe items) through the core's safety layer. Returns the updated tree, what was done (with tray entry ids), what was skipped and why, and `dryRun`. |
| `removeJunk(items)` | junk matches | counts, bytes, `dryRun` | System Junk: rule items to the tray by path; the map refreshes by rescan. |
| `restore(disk, items)` | tree, tray items | `{ disk, rescan }` | Puts tray items back (works after a relaunch; `rescan` asks for a quick rescan). |
| `emptyTray(items)` | tray items | `{ bytes, dryRun }` | Deletes tray items for good. Not undoable. |
| `emptyTrash(disk, id)` | tree, Trash node id | `RemoveOutcome` | Empties the Trash through the OS. Not undoable. |
| `cleanupStatus()` | none | `{ dryRun, dev, tray }` | Dry-run state and the tray (it survives restarts). |
| `setDryRun(on)` | boolean | none | Dev builds start every launch in dry run. |
| `junk()` | none | `JunkReport` | Rule matches, rules waiting for an app to close, rule errors. |
| `neverTouch(path, on)` | path, boolean | the list | The user's Never touch list. |
| `thinSnapshots(disk)` | tree | `Promise<Disk>` | Confirms, then asks macOS to thin local snapshots. Dev builds stay in dry run. |
| `spaceReport(before, freedNow)` | free space before, bytes just freed | `SpaceReport` | Expected versus actual free space, purgeable space, snapshots, Trash and the tray. |
| `delegateAction(action)` | `thinSnapshots`, `brewCleanup`, `dockerPrune`, `deleteUnavailableSimulators` or `emptyTrash` | `{ bytes, dryRun }` | Runs that one delegated cleanup. The snapshot size is measured in the app, not sent by the UI. |
| `quitApp(disk, appId)` | tree, app node id | `Promise<Disk>` | Asks a running app to quit before uninstalling. |

Actions take the UI's current tree and resolve to the updated tree. The mock edits it in place; the real API rejects them in Phase 1 (read-only) until Phase 2 implements them.

### Events

| Event | Payload |
|---|---|
| scan progress | `ScanProgress { phase: "walking" \| "done" \| "cancelled", items, bytes, current, paused?: "user" \| "battery" \| "lowPower" \| null }` |

### Types (`src/api/types.ts`)

- **AI Models (real scans)** include the known tool folders (Ollama, LM Studio, Hugging Face, Jan, GPT4All) and models found anywhere: Hugging Face hub caches (one tile per model, sized by the blobs its snapshots link to), Ollama stores, and weight files (.gguf, .safetensors, .ckpt, .pt, .onnx … from 50 MB), grouped by the project folder they sit in. A file is not a model from its extension alone. `.gguf` and `.safetensors` still have to carry their header. `.bin` and the other ambiguous extensions need a known header (GGUF, safetensors, PyTorch zip or pickle, ONNX) or a `config.json` in the same folder whose JSON object has `model_type` or `architectures`. A config without those keys does not count. Large & Old uses the same rule for the model chip.
- **`Disk`**: `{ nodes: DiskNode[], root, capacity, ghostId, leafCount, summary? }`. Nodes are an arena indexed by id. `summary` (real scans) carries `capacity, available, osUsed, scanned, unaccounted, home, files, dirs, unreadableDirs, datalessItems, hardlinksDeduped, cloneSharedBytes, cancelled`. The real tree is pruned: per folder, items under 10 MB fold into "Smaller items".
- **`DiskNode`**: `id, name, path, size` (bytes on disk), `parent` (-1 for root), `children, depth, category` (files, aiModels, developer, apps, system, junk, other), `kind` (volume, dir, file, app, model, ghost), `lastOpenedDays, safety` (safe, yourCall, keep), `why` (one-line explanation), `lenses` / `deep` (lens bitmasks), `matchBytes` (bytes per lens), `grewBy, shade`. Optional: `duplicateOf, readOnly, rebuildable, running, appBundle, leftover` (caches, support, containers, logs, preferences, savedState), `note` ("In iCloud only", "Couldn't read", "Shares space with clones").
- **`Insight`**: `{ tone: "amber" | "green" | "violet" | "protected", text }`.
- **Helpers** (pure, no I/O): `lensMatches`, `ancestors`, `isAncestor`, `attached`, `lensForFocus`, `categoryForLens`, `throughSingles`, `emptyDisk`, constants `GB`, `LENS_BIT`, `LENS_IDS`.

## Performance notes (Phase 1, macOS)

- Idle memory with the window open (release build, after a scan): about 131 MB in total: app 47 MB, WebKit page 61 MB, WebKit GPU 18 MB, WebKit network 5 MB. The app process is ~44 MB before any scan, so most of it is the Tauri/WebKit baseline.
- Keep full-window layers to a minimum: no `position: fixed` full-window boxes, no animated full-window layers, overlays mounted only while shown. Each full-window layer costs ~16 MB at Retina size. The reef light is painted into the page background for this reason.
