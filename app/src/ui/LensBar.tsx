import { motion } from "motion/react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { api, LENS_BIT, type Disk } from "../api";
import type { LensId } from "../design/tokens";
import { t } from "../i18n";
import { formatBytes, type UnitSystem } from "../lib/format";
import { useReducedMotion } from "../lib/prefs";
import { spring } from "../motion/tokens";
import { useApp } from "../store/app";
import { LARGE_MIN_KEY } from "./LargeOld";
import { LENSES } from "./lenses";

// Floating lens bar: matching tiles stay in full color, the rest fade. One persistent navy pill
// slides to the active lens: it moves by transform and eases its width to the pill.
// The list button is the way into the results. With no lens it stays a plain pill; with a lens
// it fills in the Scan accent and names the live count.
export function LensBar() {
  const lens = useApp((s) => s.lens);
  const plan = useApp((s) => s.plan);
  const disk = useApp((s) => s.disk);
  const version = useApp((s) => s.diskVersion);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const setLens = useApp((s) => s.setLens);
  const reduced = useReducedMotion();
  const pills = useRef<Record<string, HTMLButtonElement | null>>({});
  const [box, setBox] = useState<{ left: number; width: number } | null>(null);
  const [large, setLarge] = useState<{ count: number; bytes: number } | null>(null);
  const [glowKey, setGlowKey] = useState(0);
  const prevLens = useRef<string | null>(null);
  const active = plan ? null : lens;
  const selecting = useApp((s) => s.checked.length > 0);
  const hot = active !== null && active !== "all";

  useEffect(() => {
    if (active !== "largeOld") return;
    let live = true;
    setLarge(null);
    let min = 500e6;
    try {
      min = Number(localStorage.getItem(LARGE_MIN_KEY)) || 500e6;
    } catch {
      /* the list's own default */
    }
    api
      .largeFiles(min)
      .then((files) => {
        if (!live) return;
        setLarge({ count: files.length, bytes: files.reduce((s, f) => s + f.bytes, 0) });
      })
      .catch(() => live && setLarge(null));
    return () => {
      live = false;
    };
  }, [active, version]);

  useEffect(() => {
    const id = active ?? "all";
    const before = prevLens.current;
    prevLens.current = id;
    if (before === null || before === id || id === "all" || reduced) return;
    setGlowKey((k) => k + 1);
  }, [active, reduced]);

  const summary = useMemo(() => {
    if (!hot || !active) return null;
    if (active === "largeOld") return large;
    return lensLeaves(disk, active);
  }, [hot, active, large, disk, version]);

  useLayoutEffect(() => {
    const measure = () => {
      const el = active ? pills.current[active] : null;
      setBox(el ? { left: el.offsetLeft, width: el.offsetWidth } : null);
    };
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [active]);

  if (selecting) return null; // the selection bar takes its place

  const label =
    hot && summary
      ? t(active === "largeOld" ? "lens.seeFiles" : "lens.seeItems", {
          count: summary.count,
          size: fmt(summary.bytes, units),
        })
      : t("largeOld.showList");

  const open = () => {
    const { setOverlay, setViewMode } = useApp.getState();
    if (!active || active === "all" || active === "largeOld") setOverlay("largeOld");
    else setViewMode("list");
  };

  return (
    <div className="lens-bar" role="radiogroup" aria-label={t("lens.label")}>
      {box && (
        <motion.span
          className="lens-indicator"
          initial={false}
          animate={{ x: box.left, width: box.width }}
          transition={spring.soft}
          aria-hidden
        />
      )}
      {LENSES.map(({ id }) => {
        const on = active === id;
        return (
          <button
            key={id}
            ref={(el) => {
              pills.current[id] = el;
            }}
            type="button"
            role="radio"
            aria-checked={on}
            className={`lens-pill focus-ring ${on ? "is-active" : ""}`}
            onClick={() => setLens(id)}
          >
            <span className="relative">{t(`lens.${id}`)}</span>
          </button>
        );
      })}
      <button type="button" className={`lens-pill lens-go focus-ring ${hot ? "is-hot" : ""}`} onClick={open}>
        {hot && glowKey > 0 && !reduced && <span key={glowKey} className="lens-glow" aria-hidden />}
        <span className="relative">{label}</span>
      </button>
    </div>
  );
}

function lensLeaves(disk: Disk, lens: Exclude<LensId, "all">): { count: number; bytes: number } {
  const bit = LENS_BIT[lens];
  let count = 0;
  const stack = [...disk.nodes[disk.root].children];
  while (stack.length) {
    const n = disk.nodes[stack.pop()!];
    if (!n || n.size <= 0 || n.kind === "ghost") continue;
    if ((n.lenses & bit) !== 0) count += 1;
    else stack.push(...n.children);
  }
  return { count, bytes: disk.nodes[disk.root].matchBytes[lens] ?? 0 };
}

function fmt(bytes: number, units: UnitSystem) {
  const f = formatBytes(bytes, units);
  return `${f.value} ${f.unit}`;
}
