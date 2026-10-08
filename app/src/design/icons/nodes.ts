// The Wrasse icon set: every icon as SVG element data on a 24×24 grid with a 2px safe margin.
// Shapes start from lucide (ISC); AI chip, snapshot ghost, tray, node_modules and Python env are
// custom. Rendering (stroke, duotone fill, size) happens in Icon.tsx and canvas.ts, so the
// whole set is restyled consistently in one place.
import {
  AppWindow, Boxes, Broom, Check, ChevronDown, ChevronRight, CircleQuestionMark, Clock, Cloud, Code, Container,
  Disc, ExternalLink, File, FileArchive, FileText, Film, Folder, Hammer, Hourglass, Image, Info, Layers,
  LayoutGrid, List, ListChecks, Lock, MonitorCog, Music, Package, Power, Radar, Scale, ScrollText, Search,
  Settings, ShieldCheck, Sparkles, Trash, TrendingUp, TriangleAlert, Undo2, X,
} from "lucide";

export type IconElement = [tag: string, attrs: Record<string, string | number>];
export type IconNode = IconElement[];

const custom: Record<string, IconNode> = {
  // A chip with a spark: AI model.
  aiModel: [
    ["rect", { x: 5, y: 5, width: 14, height: 14, rx: 3 }],
    ["path", { d: "M9 2.5V5M15 2.5V5M9 19v2.5M15 19v2.5M2.5 9H5M2.5 15H5M19 9h2.5M19 15h2.5" }],
    ["path", { d: "M12 8.6l.9 2.5 2.5.9-2.5.9-.9 2.5-.9-2.5-2.5-.9 2.5-.9z" }],
  ],
  // A dashed block with clock hands: space held by a snapshot.
  ghost: [
    ["rect", { x: 3, y: 4, width: 18, height: 16, rx: 3, "stroke-dasharray": "3 2.6" }],
    ["path", { d: "M12 8.5V12l2.5 1.5" }],
  ],
  // The quarantine tray.
  tray: [
    ["path", { d: "M3 13.5 5.4 6.4A2 2 0 0 1 7.3 5h9.4a2 2 0 0 1 1.9 1.4l2.4 7.1V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" }],
    ["path", { d: "M3 13.5h4.5l1.5 2.5h6l1.5-2.5H21" }],
  ],
  // A hexagon with an N: node_modules.
  nodeModules: [
    ["path", { d: "M12 2.8l7.8 4.5v9.4L12 21.2l-7.8-4.5V7.3z" }],
    ["path", { d: "M9.5 15V9l5 6V9" }],
  ],
  // Two interlocking rounded squares: a Python environment.
  pythonEnv: [
    ["rect", { x: 3, y: 3, width: 11, height: 11, rx: 3 }],
    ["rect", { x: 10, y: 10, width: 11, height: 11, rx: 3 }],
    ["circle", { cx: 6.5, cy: 6.5, r: 0.9 }],
    ["circle", { cx: 17.5, cy: 17.5, r: 0.9 }],
  ],
};

const strip = (n: unknown) => n as IconNode;

export const ICONS = {
  // Item types
  folder: strip(Folder),
  file: strip(File),
  app: strip(AppWindow),
  video: strip(Film),
  photo: strip(Image),
  audio: strip(Music),
  document: strip(FileText),
  archive: strip(FileArchive),
  installer: strip(Package),
  diskImage: strip(Disc),
  aiModel: custom.aiModel,
  nodeModules: custom.nodeModules,
  docker: strip(Container),
  xcode: strip(Hammer),
  pythonEnv: custom.pythonEnv,
  packageCache: strip(Boxes),
  log: strip(ScrollText),
  cache: strip(Layers),
  system: strip(MonitorCog),
  cloud: strip(Cloud),
  ghost: custom.ghost,
  // App surfaces
  tray: custom.tray,
  startup: strip(Power),
  settings: strip(Settings),
  scan: strip(Radar),
  history: strip(Clock),
  audit: strip(ListChecks),
  // Safety and status
  warning: strip(TriangleAlert),
  safe: strip(ShieldCheck),
  yourCall: strip(Scale),
  keep: strip(Lock),
  help: strip(CircleQuestionMark),
  // Lenses
  all: strip(LayoutGrid),
  junk: strip(Broom),
  dev: strip(Code),
  largeOld: strip(Hourglass),
  grew: strip(TrendingUp),
  free: strip(Sparkles),
  // Controls
  search: strip(Search),
  list: strip(List),
  grid: strip(LayoutGrid),
  chevronRight: strip(ChevronRight),
  chevronDown: strip(ChevronDown),
  close: strip(X),
  trash: strip(Trash),
  restore: strip(Undo2),
  reveal: strip(ExternalLink),
  info: strip(Info),
  check: strip(Check),
} satisfies Record<string, IconNode>;

export type IconName = keyof typeof ICONS;

/** Closed shapes get the soft duotone fill; open strokes don't. */
export function isFillable([tag, attrs]: IconElement): boolean {
  if (tag === "rect" || tag === "circle" || tag === "ellipse" || tag === "polygon") return true;
  return tag === "path" && /z\s*$/i.test(String(attrs.d ?? ""));
}
