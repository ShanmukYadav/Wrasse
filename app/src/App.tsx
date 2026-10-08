import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { DevBadge } from "./components/DevBadge";
import { ScreenHost } from "./components/ScreenHost";
import { Sidebar } from "./components/Sidebar";
import { getAppInfo } from "./lib/backend";
import { useApp, type MotionPreference } from "./store/app";

// "system" follows prefers-reduced-motion; reduced motion turns movement into opacity fades.
const motionSetting: Record<MotionPreference, "user" | "always" | "never"> = {
  system: "user",
  reduce: "always",
  full: "never",
};

export function App() {
  const motion = useApp((s) => s.motion);
  const setInfo = useApp((s) => s.setInfo);
  const setScreen = useApp((s) => s.setScreen);

  useEffect(() => {
    getAppInfo().then((info) => {
      setInfo(info);
      document.documentElement.dataset.platform = info.platform === "Windows" ? "windows" : "macos";
    });
  }, [setInfo]);

  // ⌘, / Ctrl+, opens Settings.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        setScreen("settings");
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setScreen]);

  return (
    <MotionConfig reducedMotion={motionSetting[motion]}>
      <div className="flex h-screen w-screen">
        <Sidebar />
        <ScreenHost />
      </div>
      <DevBadge />
    </MotionConfig>
  );
}
