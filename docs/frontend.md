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
| **Home map** | `ui/home/Home.tsx`, `Tile.tsx`, `gridLayout.ts` | One level of the disk on a 12×8 grid, at most 12 tiles (the rest fold into "+N more"). Tile area follows the square root of size, the biggest is capped at 40% of the grid, and every tile gets a minimum (6 cells, fewer when there are many tiles); the grid is cut on whole cells, choosing the cuts that keep tiles closest to their fair size and to square. Tiles show badge, name, meta, big size, top rows, insight chips and "Open →" / "Details". Small tiles degrade to a one-line name + size. |
| **Drill-in** | `Home.tsx` (`Level`) | Opening a folder, category or app crossfades (0.18 s) to its level: the category-tinted frame with its children as tiles. No zoom, sweep or staggered reveal: the user found them distracting. Back, Esc and the breadcrumb go up; single-child folders are skipped. When everything in a level is removed, the map goes up to the nearest level that still has items. |
| **Apps drill-in** | `mock/disk.ts`, `DetailPanel.tsx` | An app opens into mini tiles: the `.app` bundle, Caches, Application Support, Containers, Logs, Saved State, Preferences, with a "Running" chip when the app is open. |
| **Onboarding** | `ui/Onboarding.tsx` | First run: what Wrasse does, no telemetry, Full Disk Access with the real status (re-checked when the window regains focus) and a button to System Settings, then "Scan this Mac". Command "Permissions" reopens it. |
| **Sonar scan** | `ui/home/Sonar.tsx` | CSS sweep: two rings, the Wrasse mark, 24 bubbles, plus live progress (items, bytes, current folder). Mounted only while a scan runs and repeats until it ends; reduced motion shows only a fade. Tiles re-reveal with one last sweep when results land. Scan becomes Stop while scanning; "Pause scan" / "Resume scan" in the command bar. On battery below 20% or in Low Power Mode the scan pauses by itself (checked every 10 s) and the readout says why, with Resume scan; resuming keeps it going for the rest of that scan. After the first scan, Scan re-reads only the folders FSEvents reports as changed (`summary.incremental`, `summary.changedFolders`). |
| **Lens bar** | `ui/LensBar.tsx`, `ui/lenses.ts` | All, Junk, Apps, AI Models, Dev, Large & Old, Grew. Category lenses open their category from anywhere; All goes home; Large & Old and Grew dim non-matching tiles. One navy pill slides to the active lens and follows the category you open. |
| **Command bar** | `ui/CommandBar.tsx`, `command/commands.ts` | ⌘K or type anywhere. Fuzzy commands: free N GB, where is <name>, lenses, scan, list/map view, zoom out, home, empty/restore tray, overlays, design system. |
| **Free Up Space** | `CommandBar.tsx` (`PlanMeter`), `ui/planReason.ts` | "free 30 GB" ranks candidates by size × unused time × safety and lists each one (rank, icon, name, where it lives, size, reason). Fill meter toward the target, "Add all N to tray", "Clear"; clicking a row shows it on the map. |
| **Detail panel** | `ui/DetailPanel.tsx` | 330 px glass panel: badge, name, size, label chip (Safe / Your call / Keep / Duplicate · Your call), location, last used, category, explanation. Actions by type: Uninstall / Remove app only (bundle), Remove (leftover), Move to quarantine, Delete now (safe items only), Open, Reveal in Finder. |
| **Select and delete** | `ui/home/Tile.tsx` (tick), `ui/SelectionBar.tsx`, `ui/CautionSheet.tsx`, `src/api/caution.ts` | Hovering a block shows a tick; once anything is ticked every block shows one and a bar replaces the lens bar ("N selected · size", Delete, Clear; Esc clears). Delete, there or in the detail panel, moves items to the tray (restorable). OS folders (Desktop, Documents, Downloads, Movies, Music, Pictures, Library, Applications on macOS; the profile folders, Program Files and Windows on Windows) and Keep data first show a caution sheet listing what goes, with an "I understand" tick before "Delete anyway". Whole home categories (Your Files, Apps, AI Models, Developer, Library & caches) have no tick and can't be deleted in one go ("Open it to choose what to delete"); macOS itself, System items, snapshots and app-managed libraries can't be ticked or deleted either. |
| **Uninstall flow** | `ui/UninstallSheet.tsx` | Checklist of the app bundle and its leftovers with sizes; "Quit app" first when it is running; confirm sends the checked parts to the tray. |
| **Quarantine tray** | `ui/QuarantineTray.tsx`, `ui/Flyers.tsx` | Removed tiles fly into the tray (bottom right). Shows count and size, Restore and Empty now. Hidden when empty. |
| **Snapshots ghost** | `ui/home/GhostTile.tsx` | Dashed tile for space held by local Time Machine snapshots, with "Free now". |
| **List view** | `ui/ListView.tsx` | The focused folder's direct children as rows; Back returns to the map of the same folder. |
| **History** | `ui/Overlays.tsx` (History), store `deleted` | Everything deleted this session, newest first: category-colored icon, name, kind (App, App data, AI model, Developer, Cache, Folder, File, Snapshots) in its category color, full path, size, time, and status (In tray · restorable with Restore, Restored, Deleted). Filters All / In tray / Deleted / Restored. Opens from the clock button in the header (after the first delete), the button on the tray, or the command bar ("history", "deleted"). |
| **Overlays** | `ui/Overlays.tsx` | Startup items, Settings (theme, motion), Audit log (every action with path and size), Quarantine history. |
| **Design page** | `ui/DesignPage.tsx` | Hidden (command "design system"): brand mark, icons, palette, chips and real tiles in both themes. |
| **Toast** | `ui/Toast.tsx` | Short confirmations ("Tray emptied", "Revealed in Finder"). |
| **Themes** | `design/apply.ts`, `lib/prefs.ts` | Light (day reef) is the default and approved direction. Dark (night) values exist but are provisional until light is approved. |

**Not built yet in D1**: empty / error / "good shape" states, and a dedicated AI-model duplicates view. Duplicates show today as a chip and a detail label.

**Read-only phase:** while `appInfo().readOnly` is true (Phase 1), cleanup actions show a toast ("This version only scans…") instead of acting.

## Components

| Area | Files |
|---|---|
| App shell | `App.tsx` (backdrop, header, map, list, lens bar, panels, overlays, Esc handling), `main.tsx` |
| Home | `ui/home/Home.tsx` (Home, Level, MoreTile), `Tile.tsx` (Tile, Row, SizeBar), `GhostTile.tsx`, `Header.tsx` (traffic lights, breadcrumb/back, command bar, free space, Scan), `Sonar.tsx`, `MiddleEllipsis.tsx`, `gridLayout.ts`, `nodes.ts` (isDrillable, lensMatch) |
| Panels and bars | `ui/LensBar.tsx`, `ui/CommandBar.tsx` (CommandBar, PlanMeter), `ui/DetailPanel.tsx`, `ui/UninstallSheet.tsx`, `ui/QuarantineTray.tsx`, `ui/ListView.tsx`, `ui/Overlays.tsx`, `ui/Toast.tsx`, `ui/Flyers.tsx`, `ui/Caustics.tsx`, `ui/Safety.tsx`, `ui/DesignPage.tsx` |
| Shared | `components/Bytes.tsx` (animated byte count), `components/DevBadge.tsx`, `design/icons/Icon.tsx` + `nodes.ts` + `semantic.ts` (duotone icon set, iconFor/toneFor), `design/brand/BrandMark.tsx` |
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
| caustics drift | 48 s cycle |

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
| `quarantine(disk, ids)` | tree, node ids | `Promise<Disk>` | Moves items to the tray (recoverable). |
| `restore(disk, ids)` | tree, node ids | `Promise<Disk>` | Puts quarantined items back. |
| `emptyTray(ids)` | node ids | `Promise<void>` | Permanently removes quarantined items. |
| `deleteNow(disk, ids)` | tree, node ids | `Promise<Disk>` | Permanent delete, safe items only, skips the tray. |
| `thinSnapshots(disk)` | tree | `Promise<Disk>` | Frees space held by local Time Machine snapshots. |
| `quitApp(disk, appId)` | tree, app node id | `Promise<Disk>` | Asks a running app to quit before uninstalling. |

Actions take the UI's current tree and resolve to the updated tree. The mock edits it in place; the real API rejects them in Phase 1 (read-only) until Phase 2 implements them.

### Events

| Event | Payload |
|---|---|
| scan progress | `ScanProgress { phase: "walking" \| "done" \| "cancelled", items, bytes, current, paused?: "user" \| "battery" \| "lowPower" \| null }` |

### Types (`src/api/types.ts`)

- **AI Models (real scans)** include the known tool folders (Ollama, LM Studio, Hugging Face, Jan, GPT4All) and models found anywhere: Hugging Face hub caches (one tile per model, sized by the blobs its snapshots link to), Ollama stores, and weight files (.gguf, .safetensors, .ckpt, .pt, .onnx … from 50 MB), grouped by the project folder they sit in.
- **`Disk`**: `{ nodes: DiskNode[], root, capacity, ghostId, leafCount, summary? }`. Nodes are an arena indexed by id. `summary` (real scans) carries `capacity, available, osUsed, scanned, unaccounted, home, files, dirs, unreadableDirs, datalessItems, hardlinksDeduped, cloneSharedBytes, cancelled`. The real tree is pruned: per folder, items under 10 MB fold into "Smaller items".
- **`DiskNode`**: `id, name, path, size` (bytes on disk), `parent` (-1 for root), `children, depth, category` (files, aiModels, developer, apps, system, junk, other), `kind` (volume, dir, file, app, model, ghost), `lastOpenedDays, safety` (safe, yourCall, keep), `why` (one-line explanation), `lenses` / `deep` (lens bitmasks), `matchBytes` (bytes per lens), `grewBy, shade`. Optional: `duplicateOf, readOnly, rebuildable, running, appBundle, leftover` (caches, support, containers, logs, preferences, savedState), `note` ("In iCloud only", "Couldn't read", "Shares space with clones").
- **`Insight`**: `{ tone: "amber" | "green" | "violet" | "protected", text }`.
- **Helpers** (pure, no I/O): `lensMatches`, `ancestors`, `isAncestor`, `attached`, `lensForFocus`, `categoryForLens`, `throughSingles`, `emptyDisk`, constants `GB`, `LENS_BIT`, `LENS_IDS`.
