// When deleting needs a second look. Pure rules over the tree, shared by every delete entry point.
import type { Disk, DiskNode } from "./types";

export type Caution =
  /** Can't be deleted from Wrasse at all (macOS itself, snapshots, app-managed libraries). */
  | { kind: "blocked" }
  /** A whole category on the home screen (Your Files, Apps, AI Models, …): open it to choose. */
  | { kind: "category" }
  /** A folder the OS creates for every user (Documents, Desktop, …). */
  | { kind: "osFolder" }
  /** Personal or app data marked Keep (Mail, Keychains, iCloud Drive, …). */
  | { kind: "protected" }
  | { kind: "none" };

// macOS: the folders in every home, and the top-level system folders.
const MAC_HOME = /^(~|\/Users\/[^/]+)(\/(Desktop|Documents|Downloads|Movies|Music|Pictures|Public|Library|Applications))?$/;
const MAC_TOP = /^\/(Applications|Library|System|Users|private|usr|opt|bin|sbin|cores|Volumes)$/;
// Windows: the folders in every profile, and the system folders on the system drive.
const WIN_HOME = /^[A-Za-z]:\\Users\\[^\\]+(\\(Desktop|Documents|Downloads|Pictures|Videos|Music|AppData|OneDrive|Favorites|Contacts|Links|Saved Games|Searches))?$/;
const WIN_TOP = /^[A-Za-z]:\\(Windows|Program Files|Program Files \(x86\)|ProgramData|Users)$/;

/** A folder the operating system creates and apps expect to exist. */
export function isOsFolder(n: DiskNode): boolean {
  return MAC_HOME.test(n.path) || MAC_TOP.test(n.path) || WIN_HOME.test(n.path) || WIN_TOP.test(n.path);
}

/** Documents, Desktop and iCloud Drive: only items the user picks by hand, only to the tray,
 *  never pre-selected (Free Up Space, Large & Old). The folders themselves are OS folders. */
const HAND_PICK = /^(~|\/Users\/[^/]+)\/(Documents|Desktop|Library\/Mobile Documents|Library\/CloudStorage)\/.+/;
export function handPickOnly(n: DiskNode): boolean {
  return HAND_PICK.test(n.path);
}

export function cautionFor(disk: Disk, id: number): Caution {
  const n = disk.nodes[id];
  if (!n || n.parent < 0 || n.kind === "ghost" || n.kind === "volume" || n.category === "system" || n.readOnly) return { kind: "blocked" };
  // "Smaller items" carries its folder's path: removing it would remove the whole folder.
  // Older saved scans lack the flag: a leaf with its parent's path can only be such an aggregate.
  if (n.aggregate || (n.children.length === 0 && n.path === disk.nodes[n.parent]?.path)) return { kind: "blocked" };
  // In iCloud only (0 bytes here): nothing to free, and removing it would remove it from iCloud.
  if (n.note === "In iCloud only") return { kind: "blocked" };
  // A whole category can't go in one click: open it and choose what to delete inside.
  if (n.depth === 1) return { kind: "category" };
  if (isOsFolder(n)) return { kind: "osFolder" };
  if (n.safety === "keep") return { kind: "protected" };
  return { kind: "none" };
}

/** Whether an item can be ticked and deleted at all. */
export function deletable(disk: Disk, id: number): boolean {
  const k = cautionFor(disk, id).kind;
  return k !== "blocked" && k !== "category";
}

/** The strongest caution among several items (undeletable items are left out by the caller). */
export function strongest(disk: Disk, ids: number[]): Caution["kind"] {
  const kinds = ids.map((id) => cautionFor(disk, id).kind);
  return kinds.includes("osFolder") ? "osFolder" : kinds.includes("protected") ? "protected" : "none";
}
