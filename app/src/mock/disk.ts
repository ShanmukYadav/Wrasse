// Mock disk for Stage A, shaped like the D1 mockup: a 1 TB volume with 446 GB free, grouped into
// the six Reef Blocks categories plus a snapshot ghost. Deterministic. Nothing here touches the
// real file system; Stage B replaces it with the Rust scanner behind the same api/ interface.
import type { Category } from "../design/tokens";
import { GB, LENS_BIT, LENS_IDS, type AppSource, type Disk, type DiskNode, type ModelMeta, type NodeKind } from "../api/types";

const CAPACITY = 1000 * GB;

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

const WHY = {
  cache: "Cache files the app rebuilds automatically. Clearing them frees space; the app may start a little slower once.",
  logs: "Old log files used only for troubleshooting. Nothing depends on them.",
  trash: "Items you already moved to the Trash.",
  mail: "Copies of attachments you opened in Mail. The originals stay in your mailbox.",
  derived: "Xcode build products. Xcode regenerates them on the next build.",
  installer: "An installer you already used. The app it installed doesn't need it.",
  nodeModules: "Project dependencies. Run npm install to get them back.",
  docker: "Docker images and build cache. Docker pulls or rebuilds them when needed.",
  simulator: "iOS simulator runtimes and devices. Xcode can download them again.",
  pkgCache: "Downloaded packages kept by package managers. They download again when needed.",
  model: "A local AI model. You can download it again, but it's large, so it's your call.",
  duplicateModel: "Byte-identical to another copy of this model in a different tool. Keeping one copy is enough.",
  duplicate: "Byte-identical to another file on this Mac. Keeping one copy is enough.",
  app: "An installed app. Uninstalling removes it and the leftovers you choose.",
  media: "Your own media. Wrasse never removes it without asking.",
  personal: "Personal files. Wrasse never touches Documents, Desktop or Photos without asking.",
  source: "Your source code. Wrasse never removes it.",
  system: "Part of macOS. Shown for size only.",
  generic: "A file you created or downloaded. Check it before removing.",
  appData: "Data this app keeps for you, such as settings, documents and databases. Removing it resets the app.",
  prefs: "This app's settings. Removing it resets the app to its defaults.",
};

type Spec = Partial<Pick<DiskNode, "safety" | "why" | "lastOpenedDays" | "kind" | "lenses" | "category">> & { path?: string };

export function generateDisk(seed = 7): Disk {
  const rand = rng(seed);
  const nodes: DiskNode[] = [];
  const days = (min: number, max: number) => Math.round(min + rand() * (max - min));

  function add(parent: number, name: string, kind: NodeKind, spec: Spec = {}, bytes = 0): number {
    const id = nodes.length;
    const p = parent >= 0 ? nodes[parent] : null;
    const base = p ? (p.path === "/" ? "" : p.path) : "";
    nodes.push({
      id,
      name,
      path: spec.path ?? (p ? `${base}/${name}` : "/"),
      size: bytes,
      parent,
      children: [],
      depth: p ? p.depth + 1 : 0,
      category: spec.category ?? (p ? p.category : "other"),
      kind,
      lastOpenedDays: spec.lastOpenedDays ?? (p ? p.lastOpenedDays : 0),
      safety: spec.safety ?? (p ? p.safety : "keep"),
      why: spec.why ?? (p ? p.why : WHY.system),
      lenses: (spec.lenses ?? 0) | (p && kind !== "ghost" ? p.lenses : 0),
      deep: 0,
      matchBytes: {},
      grewBy: 0,
      shade: 0,
      duplicateOf: undefined,
      // Mock file and model ages are scan dates, the same fallback a real walk uses.
      approx: kind === "file" || kind === "model" ? true : undefined,
    });
    if (p) p.children.push(id);
    return id;
  }
  const dir = (parent: number, name: string, spec: Spec = {}) => add(parent, name, "dir", spec);
  const file = (parent: number, name: string, bytes: number, spec: Spec = {}) => add(parent, name, spec.kind ?? "file", spec, bytes);

  /** Splits `total` bytes into `n` uneven parts (largest first). */
  function split(total: number, n: number, skew = 1.6): number[] {
    const w = Array.from({ length: n }, (_, i) => Math.pow(1 / (i + 1), skew) * (0.75 + rand() * 0.5));
    const s = w.reduce((a, b) => a + b, 0);
    return w.map((x) => (x / s) * total);
  }
  function fill(parent: number, total: number, names: string[], spec: Spec = {}, skew = 1.2) {
    split(total, names.length, skew).forEach((b, i) => file(parent, names[i], Math.round(b), spec));
  }
  const top = (name: string, category: Category, spec: Spec = {}) => dir(root, name, { ...spec, category });

  const root = add(-1, "Macintosh HD", "volume", { safety: "keep", why: WHY.system, lastOpenedDays: 0, category: "other" });

  // ---- Your Files: 197 GB ---------------------------------------------------------------
  const files = top("Your Files", "files", { path: "~", safety: "keep", why: WHY.personal });
  const movies = dir(files, "Movies", { path: "~/Movies", safety: "yourCall", why: WHY.media });
  // [name, GB, days since opened]; the two old ones add up to the 8.4 GB "old videos" insight.
  const movieList: [string, number, number][] = [
    ["Iceland Trip 2023.mov", 14.2, 40], ["Wedding Highlights.mov", 11.6, 90], ["Drone — Big Sur.mp4", 9.1, 120], ["Concert 4K.mov", 7.4, 60],
    ["Family 2019.mov", 5.1, 900], ["Lecture Recordings.mp4", 4.9, 150], ["Beach Day.mov", 4.2, 200], ["Birthday 2020.mov", 3.3, 1100],
    ["Screen Recording 2024-03-02.mov", 2.4, 30], ["Vlog Draft 3.mov", 1.8, 12],
  ];
  for (const [name, gb, age] of movieList) file(movies, name, Math.round(gb * GB), { lastOpenedDays: age });
  const downloads = dir(files, "Downloads", { path: "~/Downloads", safety: "yourCall", why: WHY.generic });
  const dl = [
    ["Xcode_16.1.xip", 11.2, 420, WHY.installer], ["Windows11_ARM.iso", 6.1, 610, WHY.installer], ["Ubuntu-24.04-arm64.iso", 3.4, 300, WHY.installer],
    ["Iceland Trip 2023 (1).mov", 4.6, 220, WHY.duplicate], ["Docker.dmg", 0.9, 220, WHY.installer], ["Blender-4.2.dmg", 0.4, 180, WHY.installer],
    ["Tax Return 2023.pdf", 0.002, 200, WHY.generic], ["Photos export.zip", 5.8, 330, WHY.generic], ["Beach Day copy.mov", 2.9, 190, WHY.duplicate],
    ["Postgres-16.pkg", 0.35, 260, WHY.installer], ["Thesis draft.docx", 0.004, 90, WHY.generic], ["dataset.csv", 3.9, 45, WHY.generic],
  ] as const;
  let dlTotal = 0;
  for (const [, gb] of dl) dlTotal += gb;
  for (const [name, gb, age, why] of dl) {
    const installer = why === WHY.installer;
    file(downloads, name, Math.round((gb / dlTotal) * 42 * GB), { lastOpenedDays: age, why, lenses: installer && age > 30 ? LENS_BIT.junk : 0 });
  }
  file(files, "VSCode-1.8.zip", 17 * GB, { path: "~/VSCode-1.8.zip", lastOpenedDays: 700, safety: "yourCall", why: WHY.installer, lenses: LENS_BIT.junk });
  const desktop = dir(files, "Desktop", { path: "~/Desktop", safety: "keep", why: WHY.personal });
  fill(desktop, 14 * GB, ["Screen Recordings", "Design exports", "Invoices", "Old project.zip", "Notes.md", "Wallpapers"], { lastOpenedDays: 60 });
  const documents = dir(files, "Documents", { path: "~/Documents", safety: "keep", why: WHY.personal });
  fill(documents, 11 * GB, ["Work", "School", "Taxes", "Personal", "Scans", "Contracts", "Resume.pdf"], { lastOpenedDays: 120 });
  const pictures = dir(files, "Pictures", { path: "~/Pictures", safety: "keep", why: WHY.media });
  nodes[pictures].readOnly = true;
  fill(pictures, 31 * GB, ["Photos Library.photoslibrary", "Lightroom Catalog", "Screenshots", "Exports"], { lastOpenedDays: 20 });
  const music = dir(files, "Music", { path: "~/Music", safety: "keep", why: WHY.media });
  nodes[music].readOnly = true;
  fill(music, 18 * GB, ["Music Library", "GarageBand Projects", "Podcasts", "Samples"], { lastOpenedDays: 35 });

  // ---- AI Models: 114 GB ------------------------------------------------------------------
  const ai = top("AI Models", "aiModels", { path: "~", lenses: LENS_BIT.aiModels, safety: "yourCall", why: WHY.model });
  const model = (name: string, gb: number, path: string, age: number) => {
    const m = add(ai, name, "model", { path, lastOpenedDays: age }, 0);
    const shards = Math.max(1, Math.round(gb / 10));
    for (let i = 0; i < shards; i++) file(m, `model-0000${i + 1}-of-0000${shards}.safetensors`, Math.round((gb * GB) / shards), { kind: "model" });
    return m;
  };
  const llamaOllama = model("Llama 3.1 70B · Ollama", 39, "~/.ollama/models/llama3.1-70b", 6);
  const llamaLms = model("Llama 3.1 70B · LM Studio", 39, "~/.lmstudio/models/Meta-Llama-3.1-70B-Q4_K_M.gguf", 140);
  nodes[llamaLms].duplicateOf = llamaOllama;
  nodes[llamaLms].why = WHY.duplicateModel;
  const qwen = model("Qwen2.5 Coder 32B", 19.8, "~/.ollama/models/qwen2.5-coder-32b", 2);
  const mistral = model("Mistral 7B · Ollama", 8.1, "~/.ollama/models/mistral-7b", 75);
  const gemma = model("Gemma 2 9B · Hugging Face", 8.1, "~/.cache/huggingface/hub/models--google--gemma-2-9b", 230);
  const meta = (tool: ModelMeta["tool"], removal: ModelMeta["removal"], quant: string, extra: Partial<ModelMeta> = {}): ModelMeta => ({
    tool, removal, quantization: quant, ...extra,
  });
  nodes[llamaOllama].model = meta("ollama", "ollama", "Q4_K_M", { ollamaName: "llama3.1:70b", weight: nodes[llamaOllama].path });
  nodes[llamaLms].model = meta("lmStudio", "quarantine", "Q4_K_M", { dupeLevel: "full", weight: nodes[llamaLms].path, partnerWeight: nodes[llamaOllama].path });
  nodes[qwen].model = meta("ollama", "ollama", "Q4_K_M", { ollamaName: "qwen2.5-coder:32b", weight: nodes[qwen].path });
  nodes[mistral].model = meta("ollama", "ollama", "Q4_0", { ollamaName: "mistral:7b", weight: nodes[mistral].path });
  nodes[gemma].model = meta("huggingFace", "huggingFace", "4-bit", { weight: nodes[gemma].path });

  // ---- Developer: 83 GB -------------------------------------------------------------------
  const devTop = top("Developer", "developer", { path: "~/Developer", lenses: LENS_BIT.dev, safety: "yourCall", why: WHY.generic });
  const nm = dir(devTop, "node_modules ×14", { path: "~/Developer/*/node_modules", safety: "safe", why: WHY.nodeModules, lenses: LENS_BIT.junk });
  nodes[nm].rebuildable = true;
  const projects = ["wrasse", "portfolio-site", "chat-app", "design-system", "api-gateway", "blog", "game-jam-2025", "mobile-app", "data-pipeline", "scraper", "budget-tracker", "llm-playground", "home-automation", "thesis"];
  split(21 * GB, projects.length, 0.8).forEach((b, i) => file(nm, `${projects[i]}/node_modules`, Math.round(b), { path: `~/Developer/${projects[i]}/node_modules`, lastOpenedDays: i < 2 ? 1 : days(60, 500) }));
  const docker = dir(devTop, "Docker images", { path: "~/Library/Containers/com.docker.docker", safety: "safe", why: WHY.docker, lenses: LENS_BIT.junk });
  fill(docker, 18 * GB, ["postgres:16", "node:22", "python:3.12", "redis:7", "build cache", "ubuntu:24.04"], { lastOpenedDays: 40 });
  const derived = dir(devTop, "Xcode DerivedData", { path: "~/Library/Developer/Xcode/DerivedData", safety: "safe", why: WHY.derived, lenses: LENS_BIT.junk });
  nodes[derived].rebuildable = true;
  fill(derived, 9.8 * GB, ["Wrasse-abc", "Portfolio-def", "ChatApp-ghi", "Widgets-jkl", "ModuleCache"], { lastOpenedDays: 12 });
  const sims = dir(devTop, "iOS Simulators", { path: "~/Library/Developer/CoreSimulator", safety: "yourCall", why: WHY.simulator });
  fill(sims, 9.5 * GB, ["iOS 18.1 runtime", "iOS 17.5 runtime", "iPhone 16 Pro", "iPad Air", "iPhone SE"], { lastOpenedDays: 90 });
  const pkg = dir(devTop, "Package caches", { path: "~/Library/Caches", safety: "safe", why: WHY.pkgCache, lenses: LENS_BIT.junk });
  fill(pkg, 9.4 * GB, ["npm", "cargo registry", "pip", "Homebrew", "Gradle"], { lastOpenedDays: 8 });
  const android = dir(devTop, "Android SDK", { path: "~/Library/Android/sdk", safety: "yourCall", why: WHY.simulator });
  fill(android, 8.3 * GB, ["system-images", "platforms", "build-tools", "emulator"], { lastOpenedDays: 260 });
  const src = dir(devTop, "Projects", { path: "~/Developer", safety: "keep", why: WHY.source });
  fill(src, 7 * GB, projects.slice(0, 8), { lastOpenedDays: 3 });

  // ---- Apps: 71 GB, 63 apps ---------------------------------------------------------------
  const apps = top("Apps", "apps", { path: "/Applications", lenses: LENS_BIT.apps, safety: "yourCall", why: WHY.app });
  const appNames = [
    "Xcode", "Final Cut Pro", "Logic Pro", "Adobe Photoshop 2026", "Android Studio", "Docker", "Microsoft Word", "Microsoft Excel",
    "DaVinci Resolve", "Blender", "Unity Hub", "Microsoft PowerPoint", "Lightroom Classic", "Premiere Pro", "Steam", "Google Chrome",
    "Figma", "Slack", "Zoom", "Spotify", "Visual Studio Code", "Cursor", "Notion", "Discord", "Firefox", "Arc", "Brave Browser",
    "iMovie", "GarageBand", "Keynote", "Pages", "Numbers", "Ollama", "LM Studio", "Bear", "Pixelmator Pro", "Affinity Designer",
    "Sketch", "OBS", "VLC", "IINA", "Postman", "TablePlus", "Raycast", "1Password", "Obsidian", "Telegram", "WhatsApp", "Signal",
    "Transmit", "Tower", "Fork", "iTerm", "Warp", "Linear", "Kindle", "Microsoft Teams", "Webex", "Parallels Desktop", "UTM",
    "Audacity", "HandBrake", "Calibre",
  ];
  const unused = new Set(["Unity Hub", "Steam", "Webex", "Parallels Desktop"]);
  const running = new Set(["Xcode", "Google Chrome", "Slack", "Spotify", "Visual Studio Code", "Figma", "Raycast", "1Password", "Docker"]);
  const CASKS = new Set(["Firefox", "Visual Studio Code", "iTerm", "Raycast", "Obsidian", "VLC", "IINA", "Discord", "Postman"]);
  const STORE = new Set(["Keynote", "Pages", "Numbers", "iMovie", "GarageBand", "WhatsApp", "Kindle", "Pixelmator Pro", "Telegram"]);
  const ROOT_OWNED = new Set(["Zoom", "Microsoft Teams", "Parallels Desktop", "Microsoft PowerPoint"]);
  const bundleId = (name: string) => `com.${name.toLowerCase().replace(/[^a-z0-9]+/g, "")}`;
  // Each app holds its bundle plus the leftovers it keeps in ~/Library (PLAN.md §6.4: true app size).
  const LEFTOVERS: [NonNullable<DiskNode["leftover"]>, string, (id: string) => string, number, string][] = [
    ["caches", "Caches", (id) => `~/Library/Caches/${id}`, 0.12, WHY.cache],
    ["support", "Application Support", (id) => `~/Library/Application Support/${id}`, 0.1, WHY.appData],
    ["containers", "Containers", (id) => `~/Library/Containers/${id}`, 0.05, WHY.appData],
    ["logs", "Logs", (id) => `~/Library/Logs/${id}`, 0.02, WHY.logs],
    ["savedState", "Saved Application State", (id) => `~/Library/Saved Application State/${id}.savedState`, 0.003, WHY.appData],
    ["preferences", "Preferences", (id) => `~/Library/Preferences/${id}.plist`, 0.0002, WHY.prefs],
  ];
  split(71 * GB, appNames.length, 1.1).forEach((b, i) => {
    const name = appNames[i];
    const id = bundleId(name);
    const age = unused.has(name) ? days(200, 500) : days(0, 60);
    const a = add(apps, name, "app", { path: `/Applications/${name}.app`, lastOpenedDays: age }, 0);
    nodes[a].running = running.has(name) && !unused.has(name);
    const source: AppSource = CASKS.has(name)
      ? { kind: "cask", token: name.toLowerCase().replace(/[^a-z0-9]+/g, "-") }
      : STORE.has(name)
        ? { kind: "appStore" }
        : { kind: "folder" };
    const owner = source.kind === "appStore" || ROOT_OWNED.has(name) ? "root" : "user";
    nodes[a].app = {
      source,
      owner,
      bundleId: id,
      version: "1.0",
      neverOpened: unused.has(name) && name === "Webex",
      lastUsedKnown: true,
      route: source.kind === "cask" ? { kind: "homebrew", token: source.token } : owner === "root" ? { kind: "finderTrash" } : { kind: "tray" },
      adminLeftovers: ROOT_OWNED.has(name) ? [`/Library/LaunchDaemons/${id}.helper.plist`] : [],
    };
    const share = LEFTOVERS.reduce((s, l) => s + l[3], 0);
    const bundle = file(a, `${name}.app`, Math.round(b * (1 - share)), { path: `/Applications/${name}.app`, why: WHY.app });
    nodes[bundle].appBundle = true;
    for (const [kind, label, path, frac, why] of LEFTOVERS) {
      // Application Support is matched by the app's name here, so it starts unticked.
      const byName = kind === "support";
      const f = file(a, label, Math.max(4096, Math.round(b * frac * (0.6 + rand() * 0.8))), {
        path: byName ? `~/Library/Application Support/${name}` : path(id),
        why: byName ? `${why} Matched by the app's name, so check it's really ${name}'s.` : why,
        safety: kind === "caches" || kind === "logs" ? "safe" : "yourCall",
      });
      nodes[f].leftover = kind;
      nodes[f].leftoverMeta = { matchedBy: byName ? "name" : "bundleId", sharedWith: [], defaultOff: byName };
      nodes[f].lastOpenedDays = age;
    }
  });

  // Leftovers from removed apps (orphans): their apps aren't installed anywhere.
  const gone = add(apps, "Leftovers from removed apps", "dir", { path: "~/Library", why: "Data apps left behind after they were removed. No installed app uses these folders." }, 0);
  nodes[gone].orphan = true;
  for (const [oid, size, untouched] of [["com.adobe.Photoshop2022", 2.1 * GB, 410], ["com.skype.skype", 0.9 * GB, 640], ["com.evernote.Evernote", 0.4 * GB, 300]] as const) {
    const o = add(gone, oid, "dir", { path: `~/Library/Application Support/${oid}`, lastOpenedDays: untouched, why: `Left by an app with the id ${oid}, which isn't installed on this Mac. Nothing here changed in ${untouched} days.` }, 0);
    nodes[o].orphan = true;
    for (const [kind, label, frac] of [["support", "Application Support", 0.7], ["caches", "Caches", 0.3]] as const) {
      const f = file(o, label, Math.round(size * frac), { path: `~/Library/${label}/${oid}`, why: `${label} of an app that's no longer installed (${oid}).`, lastOpenedDays: untouched });
      nodes[f].leftover = kind;
      nodes[f].orphan = true;
      nodes[f].leftoverMeta = { matchedBy: "bundleId", sharedWith: [], defaultOff: true };
    }
  }

  // ---- System: 52 GB ----------------------------------------------------------------------
  const sys = top("System", "system", { path: "/System", safety: "keep", why: WHY.system });
  file(sys, "macOS", 28 * GB, { path: "/System" });
  file(sys, "System Data", 16 * GB, { path: "/System/Volumes/Data/private" });
  file(sys, "Virtual memory", 8 * GB, { path: "/System/Volumes/VM" });

  // ---- Junk: 31 GB, all safe --------------------------------------------------------------
  const junk = top("Junk", "junk", { path: "~/Library", safety: "safe", why: WHY.cache, lenses: LENS_BIT.junk });
  const caches = dir(junk, "App caches", { path: "~/Library/Caches", why: WHY.cache });
  fill(caches, 18 * GB, ["Google Chrome", "Spotify", "Slack", "Figma", "Xcode", "Arc", "Zoom", "Discord", "Photos", "Music", "Safari"], { lastOpenedDays: 5 });
  const logs = dir(junk, "Logs", { path: "~/Library/Logs", why: WHY.logs });
  fill(logs, 4 * GB, ["DiagnosticReports", "Docker", "Xcode", "CoreSimulator", "install.log"], { lastOpenedDays: 200 });
  // Junk rules cover these (as the core's annotate() does on real scans).
  for (const [parent, rule] of [[caches, "app-caches"], [logs, "app-logs"]] as const) for (const c of nodes[parent].children) nodes[c].ruleId = rule;
  const trash = dir(junk, "Trash", { path: "~/.Trash", why: WHY.trash });
  fill(trash, 6 * GB, ["Old renders", "Screen Recording 2023.mov", "archive.zip", "Untitled.psd"], { lastOpenedDays: 30 });
  const mail = dir(junk, "Mail Downloads", { path: "~/Library/Containers/com.apple.mail/Data/Library/Mail Downloads", why: WHY.mail });
  fill(mail, 3 * GB, ["Attachments 2024", "Attachments 2023", "Attachments 2022"], { lastOpenedDays: 300 });
  for (const c of nodes[mail].children) nodes[c].ruleId = "mail-downloads";

  // ---- Snapshot ghost ---------------------------------------------------------------------
  const ghostId = add(root, "Held by snapshots", "ghost", { path: "/", safety: "keep", category: "other", why: "Space from files you already deleted. macOS keeps it in local Time Machine snapshots until they expire." }, Math.round(6.1 * GB));

  // Byte-identical copies in Your Files.
  const byName = (name: string) => nodes.find((n) => n.name === name)!.id;
  nodes[byName("Iceland Trip 2023 (1).mov")].duplicateOf = byName("Iceland Trip 2023.mov");
  nodes[byName("Beach Day copy.mov")].duplicateOf = byName("Beach Day.mov");
  nodes[byName("Old project.zip")].duplicateOf = byName("Photos export.zip");

  // Sum directory sizes bottom-up (children always have larger ids than parents).
  for (let i = nodes.length - 1; i >= 0; i--) {
    const n = nodes[i];
    if (n.children.length) n.size = n.children.reduce((s, c) => s + nodes[c].size, 0);
  }
  for (const n of nodes) n.children.sort((a, b) => nodes[b].size - nodes[a].size);

  // Lens tags that depend on final sizes and ages.
  for (const n of nodes) {
    if (n.kind === "volume" || n.kind === "ghost") continue;
    if (n.size >= 1 * GB && n.lastOpenedDays > 365 && n.safety !== "keep") n.lenses |= LENS_BIT.largeOld;
  }
  const growers = ["Llama 3.1 70B · Ollama", "Qwen2.5 Coder 32B", "Google Chrome", "Iceland Trip 2023.mov", "Wrasse-abc", "wrasse/node_modules"];
  for (const n of nodes) {
    if (n.children.length === 0 && growers.some((g) => n.path.includes(g) || n.name.includes(g))) {
      n.grewBy = Math.round(n.size * (0.2 + rand() * 0.5));
      n.lenses |= LENS_BIT.grew;
    }
  }
  for (const n of nodes) {
    n.deep = n.lenses;
    if (n.children.length === 0) {
      for (const lens of LENS_IDS) if (n.lenses & LENS_BIT[lens]) n.matchBytes[lens] = lens === "grew" ? n.grewBy : n.size;
    }
  }
  for (let i = nodes.length - 1; i > 0; i--) {
    const n = nodes[i];
    const parent = nodes[n.parent];
    parent.grewBy += n.grewBy;
    parent.deep |= n.deep;
    for (const lens of LENS_IDS) {
      const bytes = n.matchBytes[lens];
      if (bytes) parent.matchBytes[lens] = (parent.matchBytes[lens] ?? 0) + bytes;
    }
  }

  return { nodes, root, capacity: CAPACITY, ghostId, leafCount: nodes.filter((n) => n.children.length === 0).length };
}
