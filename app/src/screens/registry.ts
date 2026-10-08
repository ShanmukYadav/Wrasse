import type { Icon } from "@phosphor-icons/react";
import {
  Archive,
  Brain,
  ChartPieSlice,
  Code,
  Files,
  Gear,
  House,
  Power,
  Sparkle,
  SquaresFour,
  Stethoscope,
} from "@phosphor-icons/react";
import type { Hue } from "../design/tokens";
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
  icon: Icon;
  hue: Hue;
  title: MessageKey;
  body: MessageKey;
  /** Build phase that delivers this screen (PLAN.md §11). */
  phase: number;
};

// Sidebar groups, in display order. Settings is pinned to the bottom.
export const screenGroups: ScreenDef[][] = [
  [
    { id: "home", icon: House, hue: "teal", title: "nav.home", body: "screen.home.body", phase: 5 },
    { id: "freeUpSpace", icon: Sparkle, hue: "emerald", title: "nav.freeUpSpace", body: "screen.freeUpSpace.body", phase: 3 },
    { id: "storage", icon: ChartPieSlice, hue: "indigo", title: "nav.storage", body: "screen.storage.body", phase: 1 },
  ],
  [
    { id: "apps", icon: SquaresFour, hue: "blue", title: "nav.apps", body: "screen.apps.body", phase: 3 },
    { id: "aiModels", icon: Brain, hue: "violet", title: "nav.aiModels", body: "screen.aiModels.body", phase: 4 },
    { id: "developer", icon: Code, hue: "cyan", title: "nav.developer", body: "screen.developer.body", phase: 4 },
    { id: "largeOld", icon: Files, hue: "ocean", title: "nav.largeOld", body: "screen.largeOld.body", phase: 3 },
    { id: "startup", icon: Power, hue: "amber", title: "nav.startup", body: "screen.startup.body", phase: 5 },
  ],
  [
    { id: "diagnostics", icon: Stethoscope, hue: "coral", title: "nav.diagnostics", body: "screen.diagnostics.body", phase: 4 },
    { id: "quarantine", icon: Archive, hue: "slate", title: "nav.quarantine", body: "screen.quarantine.body", phase: 2 },
  ],
];

export const settingsScreen: ScreenDef = {
  id: "settings",
  icon: Gear,
  hue: "slate",
  title: "nav.settings",
  body: "screen.settings.body",
  phase: 5,
};

export const allScreens: ScreenDef[] = [...screenGroups.flat(), settingsScreen];

export function screenById(id: ScreenId): ScreenDef {
  return allScreens.find((s) => s.id === id) ?? allScreens[0];
}
