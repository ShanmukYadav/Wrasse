// One mapping from a disk item to its icon and tone, used by the map, lists, the detail panel
// and the command bar, so the same thing always looks the same.
import type { DiskNode } from "../../api/types";
import type { Tone } from "../tokens";
import type { IconName } from "./nodes";

const EXT: [RegExp, IconName][] = [
  [/\.(mov|mp4|m4v|mkv|avi)$/i, "video"],
  [/\.(heic|jpe?g|png|gif|psd|raw|tiff?)$/i, "photo"],
  [/\.(m4a|mp3|wav|aiff?|flac)$/i, "audio"],
  [/\.(dmg|iso|img)$/i, "diskImage"],
  [/\.(pkg|msi|exe)$/i, "installer"],
  [/\.(zip|tar|gz|rar|7z)$/i, "archive"],
  [/\.(gguf|safetensors|bin)$/i, "aiModel"],
  [/\.log$/i, "log"],
  [/\.(pdf|docx?|pages|key|xlsx?|numbers|md|txt|ts|rs)$/i, "document"],
];

export function iconFor(n: DiskNode): IconName {
  const name = n.name;
  const path = n.path;
  if (n.kind === "ghost") return "ghost";
  if (n.kind === "volume") return "system";
  // Top-level categories use the D1 category icons.
  if (n.depth === 1) {
    const byCategory: Partial<Record<DiskNode["category"], IconName>> = {
      files: "person", aiModels: "aiModel", developer: "dev", apps: "all", system: "monitor", junk: "junk",
    };
    const icon = byCategory[n.category];
    if (icon) return icon;
  }
  if (n.leftover) {
    const byLeftover: Record<NonNullable<DiskNode["leftover"]>, IconName> = {
      caches: "cache", logs: "log", preferences: "settings", savedState: "history", support: "folder", containers: "folder",
      groupContainers: "folder", httpStorages: "cache", webKit: "cache", launchAgents: "settings", launchDaemons: "settings", helperTools: "settings",
    };
    return byLeftover[n.leftover];
  }
  if (n.kind === "app" || name.endsWith(".app")) return "app";
  if (/\.app\//.test(path)) return n.children.length ? "folder" : "file"; // inside an app bundle
  if (n.kind === "model" || n.category === "aiModels" && n.depth >= 3) return "aiModel";
  if (name === "node_modules" || path.includes("/node_modules/")) return "nodeModules";
  if (name === ".venv" || path.includes("/.venv/") || path.includes("site-packages")) return "pythonEnv";
  if (/com\.docker|\/Docker(\/|$)/i.test(path)) return "docker";
  if (/\/Xcode(\/|$)|DerivedData|CoreSimulator/.test(path)) return "xcode";
  if (/Package caches|\/\.npm(\/|$)|\/\.cargo(\/|$)|\/\.gradle(\/|$)|Caches\/pip|Caches\/Homebrew|\/go\/pkg/.test(path)) return "packageCache";
  if (/\/Logs(\/|$)/.test(path)) return "log";
  if (/\/Caches(\/|$)|Cache\.db/.test(path)) return "cache";
  if (/Mobile Documents|iCloud/.test(path)) return "cloud";
  if (n.category === "system") return "system";
  if (n.category === "aiModels") return "aiModel";
  if (name === "Photos Library.photoslibrary") return "photo";
  if (name === "Movies") return "video";
  if (name === "Music" || name === "Music Library") return "audio";
  for (const [re, icon] of EXT) if (re.test(name)) return icon;
  if (n.children.length > 0) return "folder";
  return "file";
}

export function toneFor(n: DiskNode): Tone {
  if (n.kind === "ghost") return "ghost";
  return n.category;
}
