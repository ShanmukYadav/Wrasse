import { useEffect, useRef, useState } from "react";
import { categoryShades, lensColors } from "../design/tokens";
import { t } from "../i18n";
import { formatBytes } from "../lib/format";
import { useReducedMotion, useScheme } from "../lib/prefs";
import { mapMotion } from "../motion/tokens";
import { setNodeColorResolver, useApp } from "../store/app";
import { setRenderer } from "./controller";
import { lensMatches } from "../api/types";
import { MapRenderer } from "./renderer";

export function MapView() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const rendererRef = useRef<MapRenderer | null>(null);
  const wheel = useRef(0);
  const [announce, setAnnounce] = useState("");

  const scheme = useScheme();
  const reduced = useReducedMotion();
  const focus = useApp((s) => s.focus);
  const lens = useApp((s) => s.lens);
  const plan = useApp((s) => s.plan);
  const selected = useApp((s) => s.selected);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const visible = useApp((s) => s.viewMode === "map");

  // Create the renderer once and keep it sized to the window.
  useEffect(() => {
    const canvas = canvasRef.current!;
    const r = new MapRenderer(canvas, useApp.getState().disk);
    rendererRef.current = r;
    setRenderer(r);
    const resize = () => r.resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
    resize();
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);
    void useApp.getState().scanDisk();
    return () => {
      ro.disconnect();
      r.destroy();
      setRenderer(null);
    };
  }, []);

  useEffect(() => {
    const r = rendererRef.current!;
    r.setScheme(scheme);
    setNodeColorResolver((n) => {
      const l = useApp.getState().lens;
      return l !== "all" && lensMatches(n, l) ? lensColors[scheme][l] : categoryShades[scheme][n.category][n.shade];
    });
  }, [scheme]);
  useEffect(() => void (rendererRef.current!.reduced = reduced), [reduced]);
  useEffect(() => rendererRef.current!.setUnits(units), [units]);
  useEffect(() => rendererRef.current!.setLens(plan ? "plan" : lens), [lens, plan]);
  useEffect(() => rendererRef.current!.setCandidates(plan ? plan.ids : null), [plan]);
  useEffect(() => rendererRef.current!.setSelected(selected), [selected]);
  useEffect(() => {
    const r = rendererRef.current!;
    if (r.view && r.view.focus !== focus) r.transitionTo(focus);
  }, [focus]);

  // Announce the selection for VoiceOver.
  useEffect(() => {
    if (selected === null) return;
    const n = useApp.getState().disk.nodes[selected];
    const f = formatBytes(n.size, units);
    setAnnounce(`${n.name}, ${f.value} ${f.unit}, ${t(`category.${n.category}`)}, ${t(`safety.${n.safety}`)}`);
  }, [selected, units]);

  function local(e: { clientX: number; clientY: number }) {
    const rect = canvasRef.current!.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  }

  function onPointerMove(e: React.PointerEvent) {
    const r = rendererRef.current!;
    const { x, y } = local(e);
    const hit = r.hitTest(x, y);
    r.setHover(hit ? hit.id : null);
    const tip = tooltipRef.current!;
    if (hit) {
      const n = useApp.getState().disk.nodes[hit.id];
      const f = formatBytes(n.size, units);
      tip.textContent = `${n.name}  ·  ${f.value} ${f.unit}`;
      tip.style.transform = `translate(${Math.min(x + 14, r.width - 260)}px, ${y + 18}px)`;
      tip.style.opacity = "1";
      canvasRef.current!.style.cursor = "pointer";
    } else {
      tip.style.opacity = "0";
      canvasRef.current!.style.cursor = "default";
    }
  }

  function onPointerLeave() {
    rendererRef.current!.setHover(null);
    tooltipRef.current!.style.opacity = "0";
  }

  // Click a folder to zoom into it; click a file (or Option/right-click anything) for details.
  function onClick(e: React.MouseEvent) {
    const hit = rendererRef.current!.hitTest(local(e).x, local(e).y);
    const app = useApp.getState();
    if (!hit) return app.select(null);
    const n = app.disk.nodes[hit.id];
    if (n.children.length > 0 && !e.altKey) return app.setFocus(hit.id);
    app.select(hit.id);
  }

  function onContextMenu(e: React.MouseEvent) {
    e.preventDefault();
    const hit = rendererRef.current!.hitTest(local(e).x, local(e).y);
    if (hit) useApp.getState().select(hit.id);
  }

  function drillAt(x: number, y: number) {
    const r = rendererRef.current!;
    const hit = r.hitTest(x, y);
    if (!hit) return;
    const child = r.childOfFocusContaining(hit.id);
    if (child !== null) useApp.getState().setFocus(child);
  }

  function onDoubleClick(e: React.MouseEvent) {
    const { x, y } = local(e);
    drillAt(x, y);
  }

  // Scroll or pinch: accumulate until a threshold, then take one zoom step.
  function onWheel(e: React.WheelEvent) {
    const r = rendererRef.current!;
    if (r.busy || r.scanning) return;
    wheel.current += e.deltaY;
    if (wheel.current <= -mapMotion.wheelThreshold) {
      wheel.current = 0;
      const { x, y } = local(e);
      drillAt(x, y);
    } else if (wheel.current >= mapMotion.wheelThreshold) {
      wheel.current = 0;
      useApp.getState().zoomOut();
    }
  }

  // Safari/WKWebView trackpad pinch.
  useEffect(() => {
    const canvas = canvasRef.current!;
    let last = { x: 0, y: 0 };
    const move = (e: MouseEvent) => (last = local(e));
    const change = (e: Event) => {
      e.preventDefault();
      const scale = (e as unknown as { scale: number }).scale;
      const r = rendererRef.current!;
      if (r.busy) return;
      if (scale > 1.15) drillAt(last.x, last.y);
      else if (scale < 0.87) useApp.getState().zoomOut();
    };
    canvas.addEventListener("mousemove", move);
    canvas.addEventListener("gesturechange", change);
    canvas.addEventListener("gesturestart", (e) => e.preventDefault());
    return () => {
      canvas.removeEventListener("mousemove", move);
      canvas.removeEventListener("gesturechange", change);
    };
  }, []);

  function onKeyDown(e: React.KeyboardEvent) {
    const app = useApp.getState();
    const r = rendererRef.current!;
    const dirs: Record<string, "left" | "right" | "up" | "down"> = {
      ArrowLeft: "left",
      ArrowRight: "right",
      ArrowUp: "up",
      ArrowDown: "down",
    };
    if (e.metaKey && e.key === "ArrowDown" && app.selected !== null) {
      e.preventDefault();
      const child = r.childOfFocusContaining(app.selected);
      if (child !== null) app.setFocus(child);
    } else if ((e.metaKey && e.key === "ArrowUp") || e.key === "Backspace" || e.key === "-") {
      e.preventDefault();
      app.zoomOut();
    } else if (dirs[e.key]) {
      e.preventDefault();
      const next = r.neighbor(app.selected, dirs[e.key]);
      if (next !== null) app.select(next, false);
    } else if (e.key === "Enter" && app.selected !== null) {
      e.preventDefault();
      // Enter zooms into a folder and opens details for a file; "i" always opens details.
      if (app.disk.nodes[app.selected].children.length > 0) app.setFocus(app.selected);
      else app.select(app.selected, true);
    } else if (e.key === "i" && app.selected !== null) {
      e.preventDefault();
      app.select(app.selected, true);
    } else if (e.key === " " && app.selected !== null) {
      e.preventDefault();
      app.addToTray([app.selected]);
    }
  }

  return (
    <div className="map-layer" aria-hidden={!visible} style={{ opacity: visible ? 1 : 0, pointerEvents: visible ? "auto" : "none" }}>
      <canvas
        ref={canvasRef}
        className="map-canvas focus-ring"
        tabIndex={0}
        role="application"
        aria-label={t("map.label")}
        aria-describedby="map-help"
        onPointerMove={onPointerMove}
        onPointerLeave={onPointerLeave}
        onClick={onClick}
        onDoubleClick={onDoubleClick}
        onContextMenu={onContextMenu}
        onWheel={onWheel}
        onKeyDown={onKeyDown}
      />
      <div ref={tooltipRef} className="map-tooltip tabular" role="presentation" />
      <p id="map-help" className="sr-only">
        {t("map.help")}
      </p>
      <p className="sr-only" aria-live="polite">
        {announce}
      </p>
    </div>
  );
}
