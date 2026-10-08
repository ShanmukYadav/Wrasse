import type { LucideIcon } from "lucide-react";
import {
  ArchiveRestore,
  BrainCircuit,
  ChartPie,
  Code,
  FileStack,
  House,
  LayoutGrid,
  Power,
  Settings,
  Sparkles,
  Stethoscope,
} from "lucide-react";
import type { MessageKey } from "../i18n";

export type ScreenId =
  | "home"
  | "freeUpSpace"
  | "storage"
  | "apps"
  | "aiModels"
  | "developer"
  | "largeOld"
  | "startup"
  | "diagnostics"
  | "quarantine"
  | "settings";

export type ScreenDef = {
  id: ScreenId;
  icon: LucideIcon;
  title: MessageKey;
  body: MessageKey;
};

// Sidebar groups, in display order. Settings is pinned to the bottom.
export const screenGroups: ScreenDef[][] = [
  [
    { id: "home", icon: House, title: "nav.home", body: "screen.home.body" },
    { id: "freeUpSpace", icon: Sparkles, title: "nav.freeUpSpace", body: "screen.freeUpSpace.body" },
    { id: "storage", icon: ChartPie, title: "nav.storage", body: "screen.storage.body" },
  ],
  [
    { id: "apps", icon: LayoutGrid, title: "nav.apps", body: "screen.apps.body" },
    { id: "aiModels", icon: BrainCircuit, title: "nav.aiModels", body: "screen.aiModels.body" },
    { id: "developer", icon: Code, title: "nav.developer", body: "screen.developer.body" },
    { id: "largeOld", icon: FileStack, title: "nav.largeOld", body: "screen.largeOld.body" },
    { id: "startup", icon: Power, title: "nav.startup", body: "screen.startup.body" },
  ],
  [
    { id: "diagnostics", icon: Stethoscope, title: "nav.diagnostics", body: "screen.diagnostics.body" },
    { id: "quarantine", icon: ArchiveRestore, title: "nav.quarantine", body: "screen.quarantine.body" },
  ],
];

export const settingsScreen: ScreenDef = {
  id: "settings",
  icon: Settings,
  title: "nav.settings",
  body: "screen.settings.body",
};

export const allScreens: ScreenDef[] = [...screenGroups.flat(), settingsScreen];

export function screenById(id: ScreenId): ScreenDef {
  return allScreens.find((s) => s.id === id) ?? allScreens[0];
}
