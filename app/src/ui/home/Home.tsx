import { AnimatePresence, motion } from "motion/react";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import type { DiskNode } from "../../api";
import { d1 } from "../../design/tokens";
import { useReducedMotion, useScheme } from "../../lib/prefs";
import { duration, ease, mapMotion } from "../../motion/tokens";
import { useApp } from "../../store/app";
import { GhostTile } from "./GhostTile";
import { gridLayout } from "./gridLayout";
import { Sonar } from "./Sonar";
import { Tile, type TileMetrics } from "./Tile";

// The home grid: the current folder's children as D1 tiles on a 12 × 8 grid.
export function Home() {
  const gridRef = useRef<HTMLDivElement>(null);
  const [metrics, setMetrics] = useState<TileMetrics>({ cellW: 80, cellH: 60 });
  const disk = useApp((s) => s.disk);
  useApp((s) => s.diskVersion);
  const focus = useApp((s) => s.focus);
  const lens = useApp((s) => s.lens);
  const selected = useApp((s) => s.selected);
  const deleting = useApp((s) => s.deleting);
  const pulse = useApp((s) => s.pulse);
  const scan = useApp((s) => s.scan);
  const scanKey = useApp((s) => s.scanKey);
  const visible = useApp((s) => s.viewMode === "map");
  const units = useApp((s) => s.info?.units ?? "decimal");
  const scheme = useScheme();
  const reduced = useReducedMotion();
  const { setFocus, select, freeGhost } = useApp.getState();

  // Cell size, so tiles can decide how many rows they fit.
  useLayoutEffect(() => {
    const el = gridRef.current!;
    const measure = () => {
      const style = getComputedStyle(el);
      const w = el.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      const h = el.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom);
      setMetrics({
        cellW: (w - (d1.grid.cols - 1) * d1.grid.gap) / d1.grid.cols,
        cellH: (h - (d1.grid.rows - 1) * d1.grid.gap) / d1.grid.rows,
      });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const focusNode = disk.nodes[focus];
  const kids = focusNode.children.map((k) => disk.nodes[k]).filter((k) => k.size > 0);
  const shown = kids.slice(0, d1.grid.maxTiles);
  const cells = useMemo(
    () => gridLayout(shown.map((k) => ({ id: k.id, size: k.size })), d1.grid.cols, d1.grid.rows, d1.grid.emphasis),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [focus, shown.map((k) => `${k.id}:${k.size}`).join(",")],
  );

  // Sonar reveal: each tile appears as the sweep passes its center.
  const revealDelay = (col: number, row: number, colSpan: number, rowSpan: number) => {
    const cx = col - 1 + colSpan / 2 - d1.grid.cols / 2;
    const cy = (row - 1 + rowSpan / 2 - d1.grid.rows / 2) * (metrics.cellH / Math.max(1, metrics.cellW));
    const max = Math.hypot(d1.grid.cols / 2, d1.grid.rows / 2);
    return reduced ? 0 : (Math.hypot(cx, cy) / max) * mapMotion.sonar * 0.6;
  };

  function open(n: DiskNode) {
    if (n.children.length > 0) setFocus(n.id);
    else select(n.id);
  }

  return (
    <motion.main
      className="home"
      aria-hidden={!visible}
      animate={{ opacity: visible ? 1 : 0 }}
      transition={{ duration: duration.base, ease: ease.inOut }}
      style={{ pointerEvents: visible ? "auto" : "none" }}
    >
      <div ref={gridRef} className="home-grid">
        <AnimatePresence mode="popLayout" initial={false}>
          {cells.map((cell) => {
            const n = disk.nodes[cell.id];
            if (n.kind === "ghost") {
              return <GhostTile key={n.id} node={n} cell={cell} units={units} onSelect={() => select(n.id)} onFree={freeGhost} />;
            }
            return (
              <Tile
                key={`${n.id}-${scanKey}`}
                disk={disk}
                node={n}
                cell={cell}
                metrics={metrics}
                parentSize={focusNode.size - (disk.nodes[disk.ghostId]?.parent === focus ? disk.nodes[disk.ghostId].size : 0)}
                scheme={scheme}
                units={units}
                lens={lens}
                selected={selected === n.id}
                deleting={deleting.includes(n.id)}
                pulseKey={pulse?.id === n.id ? pulse.key : null}
                reveal={scan === "scanning" ? { key: scanKey, delay: revealDelay(cell.col, cell.row, cell.colSpan, cell.rowSpan) } : null}
                onOpen={open}
              />
            );
          })}
        </AnimatePresence>
      </div>
      {scan === "scanning" && <Sonar key={scanKey} />}
    </motion.main>
  );
}
