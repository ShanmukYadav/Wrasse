import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { DevBadge } from "./components/DevBadge";
import { applyTokens, onSystemSchemeChange } from "./design/apply";
import { syncGlass } from "./glass/regions";
import { api } from "./api";
import { useReducedMotion, useScheme } from "./lib/prefs";
import { runBenchmark } from "./map/bench";
import { MapView } from "./map/MapView";
import { useApp } from "./store/app";
import { CommandBar } from "./ui/CommandBar";
import { DetailPanel } from "./ui/DetailPanel";
import { Flyers } from "./ui/Flyers";
import { FpsHud } from "./ui/FpsHud";
import { HeaderInfo } from "./ui/HeaderInfo";
import { IconCluster } from "./ui/IconCluster";
import { LensBar } from "./ui/LensBar";
import { ListView } from "./ui/ListView";
import { Overlays } from "./ui/Overlays";
import { QuarantineTray } from "./ui/QuarantineTray";
import { Toast } from "./ui/Toast";
import { TopLeft } from "./ui/TopLeft";
import { Caustics } from "./ui/Caustics";
import { DesignPage } from "./ui/DesignPage";

export function App() {
  const scheme = useScheme();
  const reduced = useReducedMotion();
  const platform = useApp((s) => (s.info?.platform === "Windows" ? "windows" : "macos"));
  const glass = useApp((s) => s.glass);
  const nativeGlass = useApp((s) => s.nativeGlass);
  const scan = useApp((s) => s.scan);
  const bench = useApp((s) => s.info?.bench ?? false);

  useEffect(() => {
    const { setInfo, setSystemScheme, setPrefs } = useApp.getState();
    // Stage A: CSS glass everywhere; native Liquid Glass returns in Stage B.
    api.appInfo().then(setInfo);
    setPrefs({ nativeGlass: false });
    return onSystemSchemeChange(setSystemScheme);
  }, []);

  useEffect(() => applyTokens(scheme, platform), [scheme, platform]);
  useEffect(() => {
    document.documentElement.dataset.glass = nativeGlass && glass === "native" ? "native" : "css";
    syncGlass();
  }, [glass, nativeGlass]);

  // WRASSE_BENCH=1 runs the fps benchmark once the first scan finishes.
  useEffect(() => {
    if (bench && scan === "done") void runBenchmark();
  }, [bench, scan]);

  const glassClass = nativeGlass && glass === "native" ? "glass-native" : "glass-css";

  return (
    <MotionConfig reducedMotion={reduced ? "always" : "never"}>
      <div className="app-backdrop" aria-hidden />
      <Caustics />
      <div data-tauri-drag-region className="drag-strip" />
      <MapView />
      <ListView />
      <header className="chrome-top">
        <TopLeft />
        <CommandBar glassClass={glassClass} />
        <div className="top-right">
          <IconCluster />
          <HeaderInfo />
        </div>
      </header>
      <DetailPanel />
      <footer className="chrome-bottom">
        <LensBar glassClass={glassClass} />
      </footer>
      <QuarantineTray />
      <Flyers />
      <Toast />
      <Overlays />
      <DesignPage />
      <FpsHud />
      <DevBadge />
    </MotionConfig>
  );
}
