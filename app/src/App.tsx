import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { DevBadge } from "./components/DevBadge";
import { applyTokens, onSystemSchemeChange } from "./design/apply";
import { useReducedMotion, useScheme } from "./lib/prefs";
import { boot, useApp } from "./store/app";
import { DesignPage } from "./ui/DesignPage";
import { DetailPanel } from "./ui/DetailPanel";
import { Flyers } from "./ui/Flyers";
import { Header } from "./ui/home/Header";
import { Home } from "./ui/home/Home";
import { LensBar } from "./ui/LensBar";
import { ListView } from "./ui/ListView";
import { CautionSheet } from "./ui/CautionSheet";
import { Onboarding } from "./ui/Onboarding";
import { SelectionBar } from "./ui/SelectionBar";
import { Overlays } from "./ui/Overlays";
import { QuarantineTray } from "./ui/QuarantineTray";
import { Toast } from "./ui/Toast";
import { UninstallSheet } from "./ui/UninstallSheet";

export function App() {
  const scheme = useScheme();
  const reduced = useReducedMotion();
  const platform = useApp((s) => (s.info?.platform === "Windows" ? "windows" : "macos"));

  useEffect(() => {
    boot(); // once per page load, however often this effect runs
    return onSystemSchemeChange(useApp.getState().setSystemScheme);
  }, []);

  useEffect(() => applyTokens(scheme, platform), [scheme, platform]);

  // Esc goes up a level when nothing else is open. ⌘Z / Ctrl+Z undoes the last move to the tray.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useApp.getState();
      const typing = (e.target as HTMLElement).tagName === "INPUT";
      if ((e.metaKey || e.ctrlKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "z" && !typing) {
        e.preventDefault();
        s.undoLastMove();
        return;
      }
      if (e.key === "Escape" && !typing && !s.overlay && !s.detailOpen && !s.commandOpen && s.uninstall === null && !s.confirmDelete && !s.fileCaution) {
        if (s.checked.length) s.clearChecks();
        else s.zoomOut();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <MotionConfig reducedMotion={reduced ? "always" : "never"}>
      <div className="d1-frame">
        <Header />
        <Home />
        <ListView />
      </div>
      <LensBar />
      <SelectionBar />
      <DetailPanel />
      <QuarantineTray />
      <Flyers />
      <Toast />
      <Overlays />
      <Onboarding />
      <CautionSheet />
      <UninstallSheet />
      <DesignPage />
      <DevBadge />
    </MotionConfig>
  );
}
