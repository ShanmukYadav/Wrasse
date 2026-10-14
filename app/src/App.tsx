import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { DevBadge } from "./shared/DevBadge";
import { applyTokens, onSystemSchemeChange } from "./design/apply";
import { useReducedMotion, useScheme } from "./lib/prefs";
import { api } from "./api";
import { t } from "./i18n";
import { useApp } from "./store/app";
import { boot } from "./store/boot";
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

  // Shortcuts (PLAN.md §6.15). Esc goes up a level when nothing else is open; ⌘Z undoes the last
  // move to the tray; ⌘R or F5 scans; ⌘, opens Settings; ⌘⌫ or Delete puts the ticked (or the
  // selected) items in the tray, asking first where the item needs it; Space shows Quick Look.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useApp.getState();
      const tag = (e.target as HTMLElement).tagName;
      const typing = tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
      const cmd = (e.metaKey || e.ctrlKey) && !e.shiftKey && !e.altKey;
      const sheet = !!s.overlay || s.uninstall !== null || !!s.confirmDelete || !!s.fileCaution || s.commandOpen;
      if (cmd && e.key.toLowerCase() === "z" && !typing) {
        e.preventDefault();
        s.undoLastMove();
        return;
      }
      if ((cmd && e.key.toLowerCase() === "r") || e.key === "F5") {
        e.preventDefault(); // In the app, ⌘R would otherwise reload the page.
        if (s.scan !== "scanning") s.startScan();
        return;
      }
      if (cmd && e.key === ",") {
        e.preventDefault();
        s.setOverlay("settings");
        return;
      }
      if (!typing && !sheet && ((cmd && e.key === "Backspace") || (e.key === "Delete" && !e.metaKey))) {
        const ids = s.checked.length ? s.checked : s.selected !== null ? [s.selected] : [];
        if (ids.length) {
          e.preventDefault();
          s.requestDelete(ids);
        }
        return;
      }
      if (e.key === " " && !typing && !sheet && s.selected !== null) {
        const n = s.disk.nodes[s.selected];
        if (n?.path) {
          e.preventDefault();
          void api.quickLook(n.path).catch(() => s.notify(t("shortcut.quickLookFailed")));
        }
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
