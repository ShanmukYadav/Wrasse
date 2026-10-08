import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { Backdrop } from "./components/Backdrop";
import { DevBadge } from "./components/DevBadge";
import { ScreenHost } from "./components/ScreenHost";
import { Sidebar } from "./components/Sidebar";
import { applyTokens, onSchemeChange } from "./design/apply";
import { getAppInfo, getSystemVolume } from "./lib/backend";
import { useApp, type MotionPreference } from "./store/app";

// "system" follows prefers-reduced-motion; reduced motion turns movement into opacity fades.
const motionSetting: Record<MotionPreference, "user" | "always" | "never"> = {
  system: "user",
  reduce: "always",
  full: "never",
};

export function App() {
  const motion = useApp((s) => s.motion);
  const scheme = useApp((s) => s.scheme);
  const info = useApp((s) => s.info);
  const { setInfo, setVolume, setScheme, setScreen } = useApp.getState();

  useEffect(() => {
    getAppInfo().then(setInfo);
    getSystemVolume().then(setVolume);
    return onSchemeChange(setScheme);
  }, [setInfo, setVolume, setScheme]);

  const platform = info?.platform === "Windows" ? "windows" : "macos";
  useEffect(() => applyTokens(scheme, platform), [scheme, platform]);

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
      <Backdrop />
      <div className="relative flex h-screen w-screen">
        <Sidebar />
        <ScreenHost />
      </div>
      <DevBadge />
    </MotionConfig>
  );
}
