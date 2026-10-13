import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { DevBadge } from "./shared/DevBadge";
import { applyTokens, onSystemSchemeChange } from "./design/apply";
import { useReducedMotion, useScheme } from "./lib/prefs";
import { boot, useApp } from "./store/app";
import { Flyers } from "./features/cleanup/Flyers";
import { Header } from "./features/home/Header";
import { Home } from "./features/home/Home";
import { LensBar } from "./features/browse/LensBar";
import { ListView } from "./features/browse/ListView";
import { SelectionBar } from "./features/cleanup/SelectionBar";
import { QuarantineTray } from "./features/cleanup/QuarantineTray";
import { Toast } from "./shared/Toast";
import { deferred } from "./shared/Deferred";

// Closed at launch: loaded on first use (see ui/Deferred.tsx).
const DesignPage = deferred(() => import("./features/design-page/DesignPage").then((m) => m.DesignPage), (s) => s.overlay === "design");
const DetailPanel = deferred(() => import("./features/browse/DetailPanel").then((m) => m.DetailPanel), (s) => s.detailOpen || s.selected !== null);
const CautionSheet = deferred(() => import("./features/cleanup/CautionSheet").then((m) => m.CautionSheet), (s) => s.confirmDelete !== null || s.fileCaution !== null);
const Onboarding = deferred(() => import("./features/onboarding/Onboarding").then((m) => m.Onboarding), (s) => s.overlay === "onboarding");
const Overlays = deferred(() => import("./features/settings/Overlays").then((m) => m.Overlays), (s) => s.overlay !== null && s.overlay !== "design" && s.overlay !== "onboarding");
const UninstallSheet = deferred(() => import("./features/apps/UninstallSheet").then((m) => m.UninstallSheet), (s) => s.uninstall !== null);

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
