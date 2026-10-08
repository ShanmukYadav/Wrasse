import { create } from "zustand";
import type { AppInfo } from "../lib/backend";
import type { ScreenId } from "../screens/registry";

export type MotionPreference = "system" | "reduce" | "full";

type AppState = {
  screen: ScreenId;
  info: AppInfo | null;
  motion: MotionPreference;
  setScreen: (screen: ScreenId) => void;
  setInfo: (info: AppInfo) => void;
  setMotion: (motion: MotionPreference) => void;
};

export const useApp = create<AppState>((set) => ({
  screen: "home",
  info: null,
  motion: "system",
  setScreen: (screen) => set({ screen }),
  setInfo: (info) => set({ info }),
  setMotion: (motion) => set({ motion }),
}));
