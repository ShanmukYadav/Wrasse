import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { api } from "./api";
import { DevBadge } from "./components/DevBadge";
import { applyTokens, onSystemSchemeChange } from "./design/apply";
import { useReducedMotion, useScheme } from "./lib/prefs";
import { useApp } from "./store/app";
import { Caustics } from "./ui/Caustics";
import { DesignPage } from "./ui/DesignPage";
import { DetailPanel } from "./ui/DetailPanel";
import { Flyers } from "./ui/Flyers";
import { Header } from "./ui/home/Header";
import { Home } from "./ui/home/Home";
import { LensBar } from "./ui/LensBar";
import { ListView } from "./ui/ListView";
import { Overlays } from "./ui/Overlays";
import { QuarantineTray } from "./ui/QuarantineTray";
import { Toast } from "./ui/Toast";

export function App() {
  const scheme = useScheme();
  const reduced = useReducedMotion();
  const platform = useApp((s) => (s.info?.platform === "Windows" ? "windows" : "macos"));

  useEffect(() => {
    const { setInfo, setSystemScheme, scanDisk } = useApp.getState();
    api.appInfo().then(setInfo);
    void scanDisk();
    return onSystemSchemeChange(setSystemScheme);
  }, []);

  useEffect(() => applyTokens(scheme, platform), [scheme, platform]);

  // Esc goes up a level when nothing else is open.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useApp.getState();
      const typing = (e.target as HTMLElement).tagName === "INPUT";
      if (e.key === "Escape" && !typing && !s.overlay && !s.detailOpen && !s.commandOpen) s.zoomOut();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <MotionConfig reducedMotion={reduced ? "always" : "never"}>
      <div className="app-backdrop" aria-hidden />
      <Caustics />
      <div className="d1-frame">
        <Header />
        <Home />
        <ListView />
      </div>
      <LensBar />
      <DetailPanel />
      <QuarantineTray />
      <Flyers />
      <Toast />
      <Overlays />
      <DesignPage />
      <DevBadge />
    </MotionConfig>
  );
}
