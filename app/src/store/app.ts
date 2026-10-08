import { create } from "zustand";
import { currentScheme } from "../design/apply";
import type { Scheme } from "../design/tokens";
import type { AppInfo, VolumeSpace } from "../lib/backend";
import type { ScreenId } from "../screens/registry";

export type MotionPreference = "system" | "reduce" | "full";

type AppState = {
  screen: ScreenId;
  info: AppInfo | null;
  volume: VolumeSpace | null;
  scheme: Scheme;
  motion: MotionPreference;
  setScreen: (screen: ScreenId) => void;
  setInfo: (info: AppInfo) => void;
  setVolume: (volume: VolumeSpace | null) => void;
  setScheme: (scheme: Scheme) => void;
  setMotion: (motion: MotionPreference) => void;
};

export const useApp = create<AppState>((set) => ({
  screen: "home",
  info: null,
  volume: null,
  scheme: currentScheme(),
  motion: "system",
  setScreen: (screen) => set({ screen }),
  setInfo: (info) => set({ info }),
  setVolume: (volume) => set({ volume }),
  setScheme: (scheme) => set({ scheme }),
  setMotion: (motion) => set({ motion }),
}));
