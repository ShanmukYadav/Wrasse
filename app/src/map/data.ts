// Mock disk for the Reef Map prototype. Deterministic, so every launch shows the same disk.
// Nothing here touches the real file system.
import type { Category, LensId, Safety } from "../design/tokens";

export type NodeKind = "volume" | "dir" | "file" | "app" | "model" | "ghost";

export const LENS_BIT: Record<Exclude<LensId, "all">, number> = {
  junk: 1,
  apps: 2,
  aiModels: 4,
  dev: 8,
  largeOld: 16,
  grew: 32,
};

export type DiskNode = {
  id: number;
  name: string;
  path: string;
  size: number;
  parent: number; // -1 for the root
  children: number[];
  depth: number;
  category: Category;
  kind: NodeKind;
  lastOpenedDays: number;
  safety: Safety;
  why: string;
  lenses: number; // bitmask of LENS_BIT
  deep: number; // lenses of this node and all descendants
  grewBy: number;
  shade: 0 | 1 | 2;
};

export type Disk = {
  nodes: DiskNode[];
  root: number;
  capacity: number;
  ghostId: number;
  leafCount: number;
};

export const GB = 1e9;
const CAPACITY = 1000 * GB;
const FILES_TOTAL = 567 * GB;
const GHOST_SIZE = 31 * GB;

// Small deterministic PRNG (mulberry32).
function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

type Attrs = Partial<Pick<DiskNode, "safety" | "why" | "lastOpenedDays" | "kind" | "lenses">>;

const WHY = {
  cache: "Cache files the app rebuilds automatically. Removing them frees space; the app may start a little slower once.",
  logs: "Old log files used only for troubleshooting. Nothing depends on them.",
  trash: "Items you already moved to the Trash.",
  derived: "Xcode build products. Xcode regenerates them on the next build.",
  installer: "An installer you already used. The app it installed doesn't need it.",
  nodeModules: "Dependencies for a project you haven't touched in a while. Run npm install to get them back.",
  buildOutput: "Build output that the toolchain regenerates on the next build.",
  pkgCache: "Downloaded packages kept by a package manager. They download again when needed.",
  model: "A local AI model. You can download it again, but it's large, so it's your call.",
  backup: "An old device backup. Make sure you have a newer one before removing it.",
  app: "An installed app. Uninstalling removes it and the leftovers you choose.",
  media: "Your own media. Wrasse never removes it without asking.",
  personal: "Personal files. Wrasse never touches Documents, Desktop or Photos.",
  system: "Part of the operating system. Shown for size only.",
  generic: "A file you created or downloaded. Check it before removing.",
};

const APPS = [
  "Xcode", "Docker", "Android Studio", "Final Cut Pro", "Logic Pro", "Adobe Photoshop 2026", "Microsoft Word",
  "Microsoft Excel", "Microsoft PowerPoint", "Slack", "Zoom", "Figma", "Spotify", "Google Chrome", "Firefox",
  "Arc", "Brave Browser", "Visual Studio Code", "Cursor", "iMovie", "GarageBand", "Keynote", "Pages", "Numbers",
  "Notion", "Discord", "WhatsApp", "Telegram", "Steam", "Blender", "Unity Hub", "OBS", "VLC", "IINA",
  "Postman", "TablePlus", "Raycast", "1Password", "Obsidian", "Ollama", "LM Studio", "Bear", "Pixelmator Pro",
  "DaVinci Resolve", "Affinity Designer", "Sketch", "Transmit", "Tower", "Fork", "iTerm", "Warp", "Linear",
  "Signal", "Kindle", "Microsoft Teams", "Webex", "Parallels Desktop", "UTM", "Audacity", "HandBrake",
  "Lightroom Classic", "Premiere Pro", "After Effects", "Illustrator", "InDesign", "Calibre",
];
const CACHE_APPS = [
  "com.spotify.client", "com.google.Chrome", "com.apple.Safari", "org.mozilla.firefox", "company.thebrowser.Browser",
  "com.microsoft.VSCode", "com.tinyspeck.slackmacgap", "us.zoom.xos", "com.figma.Desktop", "com.docker.docker",
  "com.apple.dt.Xcode", "com.brave.Browser", "com.hnc.Discord", "com.valvesoftware.steam", "com.adobe.Photoshop",
  "com.microsoft.teams2", "notion.id", "com.apple.Music", "com.apple.Photos", "com.apple.mail",
];
const PROJECTS = [
  "wrasse", "portfolio-site", "ml-experiments", "chat-app", "rust-raytracer", "data-pipeline", "mobile-app",
  "blog", "game-jam-2025", "thesis", "api-gateway", "design-system", "scraper", "dotfiles", "leetcode",
  "home-automation", "budget-tracker", "llm-playground",
];
const NPM_PACKAGES = [
  "react", "react-dom", "typescript", "esbuild", "@babel", "webpack", "vite", "next", "lodash", "@types",
  "eslint", "prettier", "rxjs", "three", "@swc", "sharp", "puppeteer", "electron", "@tensorflow", "aws-sdk",
  "firebase", "playwright-core", "tailwindcss", "date-fns", "zod", "axios", "jest", "@storybook", "core-js",
  "caniuse-lite",
];
const MODELS = [
  ["ollama", "llama3.1:8b", 4.9], ["ollama", "qwen2.5-coder:14b", 9.0], ["ollama", "mistral:7b", 4.1],
  ["ollama", "deepseek-r1:32b", 19.9], ["ollama", "gemma3:12b", 8.1], ["ollama", "nomic-embed-text", 0.27],
  ["hf", "models--meta-llama--Llama-3.1-8B-Instruct", 16.1], ["hf", "models--mistralai--Mistral-7B-v0.3", 14.5],
  ["hf", "models--openai--whisper-large-v3", 3.1], ["hf", "models--stabilityai--sdxl-turbo", 6.9],
  ["lmstudio", "Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf", 4.9], ["lmstudio", "Qwen2.5-14B-Instruct-Q5_K_M.gguf", 10.5],
  ["lmstudio", "phi-4-Q6_K.gguf", 12.0],
] as const;
const DOC_WORDS = ["Report", "Invoice", "Notes", "Plan", "Resume", "Budget", "Slides", "Draft", "Scan", "Contract", "Letter", "Syllabus"];
const DOC_EXT = [".pdf", ".docx", ".key", ".pages", ".xlsx", ".md", ".txt"];
const MEDIA_WORDS = ["Trip", "Birthday", "Wedding", "Concert", "Screen Recording", "Lecture", "Vlog", "Drone", "Family", "Beach"];
const INSTALLERS = ["Docker", "Zoom", "Blender", "Unity", "Python", "Node", "Steam", "Figma", "Chrome", "OBS", "VSCode", "Postgres", "Android Studio", "Xcode_16", "Parallels", "Ubuntu-24.04", "Windows11_ARM"];
const INSTALLER_EXT = [".dmg", ".pkg", ".zip", ".iso"];

/**
 * `targetLeaves` mock files. `uniform` flattens the size distribution so most files stay visible
 * as separate blocks (used for the 20,000-block stress test).
 */
export function generateDisk(targetLeaves = 2000, uniform = false, seed = 7): Disk {
  const rand = rng(seed);
  const f = targetLeaves / 2000;
  const nodes: DiskNode[] = [];
  const pick = <T,>(arr: readonly T[]) => arr[Math.floor(rand() * arr.length)];
  const count = (base: number) => Math.max(1, Math.round(base * f * (0.8 + rand() * 0.4)));
  // Log-normal size around `median` bytes.
  const size = (median: number, spread = 1) => {
    if (uniform) spread *= 0.15;
    const g = Math.sqrt(-2 * Math.log(rand() + 1e-9)) * Math.cos(2 * Math.PI * rand());
    return Math.max(1024, median * Math.exp(g * spread));
  };
  const days = (min: number, max: number) => Math.round(min + rand() * (max - min));

  function add(parent: number, name: string, category: Category, kind: NodeKind, attrs: Attrs = {}, bytes = 0): number {
    const id = nodes.length;
    const p = parent >= 0 ? nodes[parent] : null;
    nodes.push({
      id,
      name,
      path: p ? (p.path === "/" ? `/${name}` : `${p.path}/${name}`) : "/",
      size: bytes,
      parent,
      children: [],
      depth: p ? p.depth + 1 : 0,
      category,
      kind,
      lastOpenedDays: attrs.lastOpenedDays ?? (p ? p.lastOpenedDays : 0),
      safety: attrs.safety ?? (p ? p.safety : "keep"),
      why: attrs.why ?? (p ? p.why : WHY.system),
      lenses: (attrs.lenses ?? 0) | (p ? p.lenses : 0),
      grewBy: 0,
      deep: 0,
      shade: Math.floor(rand() * 3) as 0 | 1 | 2,
    });
    if (p) p.children.push(id);
    return id;
  }
  const dir = (parent: number, name: string, attrs: Attrs = {}) =>
    add(parent, name, nodes[parent].category, "dir", attrs);
  const file = (parent: number, name: string, bytes: number, attrs: Attrs = {}) =>
    add(parent, name, nodes[parent].category, attrs.kind ?? "file", attrs, bytes);
  const top = (name: string, category: Category, attrs: Attrs = {}) => add(root, name, category, "dir", attrs);

  const root = add(-1, "Macintosh HD", "other", "volume", { safety: "keep", why: WHY.system, lastOpenedDays: 0 });

  // Applications
  const apps = top("Applications", "apps", { lenses: LENS_BIT.apps, safety: "yourCall", why: WHY.app });
  const appCount = Math.min(APPS.length, Math.round(48 * Math.sqrt(f)));
  for (let i = 0; i < appCount; i++) {
    const name = APPS[i];
    const big = name === "Xcode" ? 12 * GB : name.includes("Studio") || name.includes("Final Cut") || name.includes("Logic") ? 4 * GB : 0;
    const app = dir(apps, `${name}.app`, { kind: "app", lastOpenedDays: days(0, 500) });
    nodes[app].kind = "app";
    const parts = Math.max(1, Math.round(4 * f));
    file(app, "Contents/MacOS", big ? big * 0.2 : size(60e6, 0.8));
    file(app, "Contents/Frameworks", big ? big * 0.5 : size(180e6, 0.9));
    for (let k = 0; k < parts; k++) file(app, `Contents/Resources ${k + 1}`, big ? (big * 0.3) / parts : size(40e6, 1));
  }

  // Users
  const users = top("Users", "users", { safety: "keep", why: WHY.personal });
  const home = dir(users, "shanmuk");
  const downloads = dir(home, "Downloads", { safety: "yourCall", why: WHY.generic });
  for (let i = 0, n = count(60); i < n; i++) {
    const installer = rand() < 0.45;
    if (installer) {
      const age = days(5, 700);
      file(downloads, `${pick(INSTALLERS)}-${1 + Math.floor(rand() * 9)}.${Math.floor(rand() * 20)}${pick(INSTALLER_EXT)}`, size(700e6, 1.1), {
        lastOpenedDays: age,
        why: WHY.installer,
        lenses: age > 30 ? LENS_BIT.junk : 0,
      });
    } else {
      file(downloads, `${pick(DOC_WORDS)} ${2020 + Math.floor(rand() * 6)}-${i}${pick(DOC_EXT)}`, size(4e6, 1.4), { lastOpenedDays: days(1, 900) });
    }
  }
  const movies = dir(home, "Movies", { safety: "yourCall", why: WHY.media });
  for (let i = 0, n = count(26); i < n; i++) file(movies, `${pick(MEDIA_WORDS)} ${i + 1}.mov`, size(2.2e9, 0.9), { lastOpenedDays: days(10, 1400) });
  const music = dir(home, "Music", { safety: "keep", why: WHY.media });
  const musicLib = dir(music, "Music Library");
  for (let i = 0, n = count(140); i < n; i++) file(musicLib, `Track ${String(i + 1).padStart(3, "0")}.m4a`, size(9e6, 0.4), { lastOpenedDays: days(1, 600) });
  const pictures = dir(home, "Pictures", { safety: "keep", why: WHY.personal });
  const photos = dir(pictures, "Photos Library.photoslibrary");
  for (let i = 0, n = count(160); i < n; i++) file(photos, `IMG_${4000 + i}.HEIC`, size(3.5e6, 0.5), { lastOpenedDays: days(1, 1200) });
  const documents = dir(home, "Documents", { safety: "keep", why: WHY.personal });
  for (const sub of ["School", "Work", "Taxes", "Personal"]) {
    const d = dir(documents, sub);
    for (let i = 0, n = count(45); i < n; i++) file(d, `${pick(DOC_WORDS)} ${i + 1}${pick(DOC_EXT)}`, size(2.5e6, 1.3), { lastOpenedDays: days(1, 1500) });
  }
  const desktop = dir(home, "Desktop", { safety: "keep", why: WHY.personal });
  for (let i = 0, n = count(30); i < n; i++) file(desktop, `Screenshot 2026-0${1 + (i % 9)}-${10 + (i % 18)} at ${i}.png`, size(1.8e6, 0.6), { lastOpenedDays: days(1, 300) });
  const trash = dir(home, ".Trash", { safety: "safe", why: WHY.trash, lenses: LENS_BIT.junk });
  for (let i = 0, n = count(24); i < n; i++) file(trash, `${pick([...DOC_WORDS, ...MEDIA_WORDS])} ${i} copy${pick([".mov", ".zip", ".pdf", ".psd"])}`, size(80e6, 1.6), { lastOpenedDays: days(3, 90) });

  // Library
  const library = top("Library", "library", { safety: "yourCall", why: WHY.generic });
  const caches = dir(library, "Caches", { safety: "safe", why: WHY.cache, lenses: LENS_BIT.junk });
  for (const id of CACHE_APPS) {
    const c = dir(caches, id, { lastOpenedDays: days(0, 200) });
    for (let i = 0, n = count(14); i < n; i++) file(c, `Cache.db-${i}`, size(25e6, 1.3));
  }
  const logs = dir(library, "Logs", { safety: "safe", why: WHY.logs, lenses: LENS_BIT.junk });
  for (let i = 0, n = count(50); i < n; i++) file(logs, `${pick(CACHE_APPS)}-${i}.log`, size(3e6, 1.4), { lastOpenedDays: days(10, 400) });
  const support = dir(library, "Application Support", { safety: "yourCall", why: WHY.generic });
  for (const app of APPS.slice(0, Math.round(26 * Math.sqrt(f)))) {
    const s = dir(support, app, { lastOpenedDays: days(0, 600) });
    for (let i = 0, n = count(5); i < n; i++) file(s, `${["Data", "Storage", "IndexedDB", "Local Storage", "GPUCache"][i % 5]}${i >= 5 ? ` ${i}` : ""}`, size(30e6, 1.4));
  }
  const backups = dir(library, "Application Support/MobileSync/Backup", { safety: "yourCall", why: WHY.backup });
  for (const name of ["iPhone 15 Pro", "iPhone 12", "iPad Air"]) {
    const b = dir(backups, name, { lastOpenedDays: name === "iPhone 15 Pro" ? 12 : days(400, 900) });
    for (let i = 0, n = count(8); i < n; i++) file(b, `Manifest-${i}.db`, size(500e6, 0.6));
  }
  const mail = dir(library, "Containers/com.apple.mail/Mail Downloads", { safety: "safe", why: WHY.cache, lenses: LENS_BIT.junk });
  for (let i = 0, n = count(30); i < n; i++) file(mail, `Attachment ${i + 1}${pick(DOC_EXT)}`, size(2e6, 1.2), { lastOpenedDays: days(20, 800) });

  // Developer
  const devTop = top("Developer", "developer", { lenses: LENS_BIT.dev, safety: "yourCall", why: WHY.generic });
  for (const project of PROJECTS) {
    const active = project === "wrasse" || project === "llm-playground";
    const p = dir(devTop, project, { lastOpenedDays: active ? 0 : days(20, 500) });
    const src = dir(p, "src", { safety: "keep", why: WHY.personal });
    for (let i = 0, n = count(6); i < n; i++) file(src, `module_${i}.ts`, size(40e3, 1));
    if (rand() < 0.75 || active) {
      const nm = dir(p, "node_modules", { safety: "yourCall", why: WHY.nodeModules, lenses: nodes[p].lastOpenedDays > 90 ? LENS_BIT.junk : 0 });
      for (let i = 0, n = count(9); i < n; i++) file(nm, NPM_PACKAGES[(i + project.length) % NPM_PACKAGES.length], size(18e6, 1.2));
    }
    if (rand() < 0.4) {
      const t = dir(p, "target", { safety: "safe", why: WHY.buildOutput, lenses: LENS_BIT.junk });
      for (let i = 0, n = count(5); i < n; i++) file(t, ["debug", "release", "incremental", "deps", "build"][i % 5] + (i >= 5 ? i : ""), size(300e6, 0.8));
    }
    if (rand() < 0.35) {
      const v = dir(p, ".venv", { safety: "yourCall", why: WHY.nodeModules, lenses: nodes[p].lastOpenedDays > 90 ? LENS_BIT.junk : 0 });
      for (let i = 0, n = count(4); i < n; i++) file(v, `lib/site-packages ${i}`, size(90e6, 0.9));
    }
  }
  const derived = dir(devTop, "Xcode/DerivedData", { safety: "safe", why: WHY.derived, lenses: LENS_BIT.junk });
  for (let i = 0, n = count(12); i < n; i++) file(derived, `${pick(PROJECTS)}-${(Math.floor(rand() * 1e9)).toString(36)}`, size(600e6, 0.8), { lastOpenedDays: days(5, 300) });
  const sims = dir(devTop, "CoreSimulator/Devices", { safety: "yourCall", why: WHY.derived });
  for (let i = 0, n = count(6); i < n; i++) file(sims, `iPhone ${14 + (i % 4)} (iOS ${17 + (i % 3)}.${i % 4})`, size(1.6e9, 0.5), { lastOpenedDays: days(30, 600) });
  const pkg = dir(devTop, "Package caches", { safety: "safe", why: WHY.pkgCache, lenses: LENS_BIT.junk });
  for (const c of [".npm/_cacache", ".cargo/registry", "Library/Caches/pip", "Library/Caches/Homebrew", ".gradle/caches", "go/pkg/mod"]) {
    const d = dir(pkg, c);
    for (let i = 0, n = count(6); i < n; i++) file(d, `chunk-${i}`, size(120e6, 1));
  }

  // AI Models
  const ai = top("AI Models", "aiModels", { lenses: LENS_BIT.aiModels, safety: "yourCall", why: WHY.model });
  const tools: Record<string, number> = {
    ollama: dir(ai, ".ollama/models"),
    hf: dir(ai, ".cache/huggingface/hub"),
    lmstudio: dir(ai, ".lmstudio/models"),
  };
  for (const [tool, name, gb] of MODELS) {
    const m = dir(tools[tool], name, { lastOpenedDays: days(0, 260) });
    nodes[m].kind = "model";
    const shards = Math.max(1, Math.round(gb / 4));
    for (let i = 0; i < shards; i++) file(m, tool === "ollama" ? `sha256-${(Math.floor(rand() * 2 ** 40)).toString(16)}` : `model-0000${i + 1}-of-0000${shards}.safetensors`, (gb * GB) / shards, { kind: "model" });
  }

  // System
  const system = top("System", "system", { safety: "keep", why: WHY.system });
  file(system, "macOS", 14 * GB);
  file(system, "System Data", 6 * GB);
  file(system, "VM (swap)", 3 * GB);
  const sysLibs = dir(system, "Library");
  for (let i = 0, n = count(40); i < n; i++) file(sysLibs, `Framework ${i + 1}`, size(40e6, 1));

  // Other
  const other = top("Other", "other", { safety: "yourCall", why: WHY.generic });
  for (const sub of ["Shared", "opt/homebrew", "usr/local"]) {
    const d = dir(other, sub);
    for (let i = 0, n = count(30); i < n; i++) file(d, `item-${i}`, size(8e6, 1.5), { lastOpenedDays: days(1, 900) });
  }

  // Ghost: deleted, but still held by local snapshots.
  const ghostId = add(root, "Held by snapshots", "other", "ghost", { safety: "keep", why: "Space from files you already deleted. macOS keeps it in local Time Machine snapshots until they expire." }, GHOST_SIZE);

  // Normalize real files to the target total, keeping fixed-size system and model blocks.
  const leaves = nodes.filter((n) => n.children.length === 0 && n.kind !== "ghost" && n.kind !== "volume");
  const fixed = leaves.filter((n) => n.category === "system" && n.depth === 2 || n.kind === "model");
  const fixedTotal = fixed.reduce((s, n) => s + n.size, 0);
  const flexible = leaves.filter((n) => !fixed.includes(n));
  const flexTotal = flexible.reduce((s, n) => s + n.size, 0);
  const scale = Math.max(0.05, (FILES_TOTAL - fixedTotal) / flexTotal);
  for (const n of flexible) n.size = Math.round(n.size * scale);

  // Sum directory sizes bottom-up (children always have larger ids than parents).
  for (let i = nodes.length - 1; i >= 0; i--) {
    const n = nodes[i];
    if (n.children.length) n.size = n.children.reduce((s, c) => s + nodes[c].size, 0);
  }
  for (const n of nodes) n.children.sort((a, b) => nodes[b].size - nodes[a].size);

  // Lens tags that depend on final sizes.
  for (const n of nodes) {
    if (n.kind === "volume" || n.kind === "ghost") continue;
    if (n.size >= 1 * GB && n.lastOpenedDays > 365 && n.safety !== "keep") n.lenses |= LENS_BIT.largeOld;
  }
  const growers = ["com.google.Chrome", "llama3.1:8b", "deepseek-r1:32b", "Movies", "DerivedData", "wrasse"];
  for (const n of nodes) {
    if (n.children.length === 0 && n.kind !== "ghost" && (growers.some((g) => n.path.includes(g)) || rand() < 0.03)) {
      n.grewBy = Math.round(n.size * (0.15 + rand() * 0.6));
      n.lenses |= LENS_BIT.grew;
    }
  }
  // A directory matches a lens when any descendant does, so lenses also apply to collapsed blocks.
  for (const n of nodes) n.deep = n.lenses;
  for (let i = nodes.length - 1; i > 0; i--) {
    const n = nodes[i];
    nodes[n.parent].grewBy += n.grewBy;
    nodes[n.parent].deep |= n.deep;
  }

  return { nodes, root, capacity: CAPACITY, ghostId, leafCount: leaves.length };
}

export function lensMatches(node: DiskNode, lens: LensId): boolean {
  return lens === "all" || (node.deep & LENS_BIT[lens]) !== 0;
}

export function ancestors(disk: Disk, id: number): number[] {
  const out: number[] = [];
  for (let n = disk.nodes[id]; n.parent >= 0; n = disk.nodes[n.parent]) out.unshift(n.parent);
  return out;
}

export function isAncestor(disk: Disk, ancestor: number, id: number): boolean {
  for (let n = disk.nodes[id]; n.parent >= 0; n = disk.nodes[n.parent]) if (n.parent === ancestor) return true;
  return false;
}
