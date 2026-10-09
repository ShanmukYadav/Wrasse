import { AnimatePresence, LayoutGroup, motion } from "motion/react";
import { useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { DiskNode } from "../../api";
import { categories, d1 } from "../../design/tokens";
import { t } from "../../i18n";
import { formatBytes } from "../../lib/format";
import { useReducedMotion, useScheme } from "../../lib/prefs";
import { duration, ease, mapMotion } from "../../motion/tokens";
import { useApp } from "../../store/app";
import { GhostTile } from "./GhostTile";
import { gridLayout, type Cell } from "./gridLayout";
import { Sonar } from "./Sonar";
import { Icon } from "../../design/icons/Icon";
import { isDrillable } from "./nodes";
import { Tile, type TileMetrics } from "./Tile";

const MORE_ID = -2;

// Home: one level of the disk at a time. Moving between levels is a short crossfade: no zoom,
// no sweep, no staggered reveal (the user found those distracting).
export function Home() {
  const focus = useApp((s) => s.focus);
  const disk = useApp((s) => s.disk);
  const scan = useApp((s) => s.scan);
  const scanKey = useApp((s) => s.scanKey);
  const visible = useApp((s) => s.viewMode === "map");
  return (
    <motion.main
      className="home"
      aria-hidden={!visible}
      animate={{ opacity: visible ? 1 : 0 }}
      transition={{ duration: duration.base, ease: ease.inOut }}
      style={{ pointerEvents: visible ? "auto" : "none" }}
    >
      <LayoutGroup>
        <AnimatePresence initial={false}>
          <Level key={focus} focus={focus} root={focus === disk.root} />
        </AnimatePresence>
      </LayoutGroup>
      {scan === "scanning" && <Sonar key={scanKey} />}
    </motion.main>
  );
}

function Level({ focus, root }: { focus: number; root: boolean }) {
  const gridRef = useRef<HTMLDivElement>(null);
  const [metrics, setMetrics] = useState<TileMetrics>({ cellW: 80, cellH: 60 });
  const disk = useApp((s) => s.disk);
  useApp((s) => s.diskVersion);
  const lens = useApp((s) => s.lens);
  const selected = useApp((s) => s.selected);
  const deleting = useApp((s) => s.deleting);
  const pulse = useApp((s) => s.pulse);
  const scan = useApp((s) => s.scan);
  const scanKey = useApp((s) => s.scanKey);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const scheme = useScheme();
  const reduced = useReducedMotion();
  const { setFocus, select, freeGhost, setViewMode } = useApp.getState();

  // Cell size, so tiles can decide how many rows they fit.
  useLayoutEffect(() => {
    const el = gridRef.current!;
    const measure = () => {
      const style = getComputedStyle(el);
      const w = el.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      const h = el.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom);
      const cellW = (w - (d1.grid.cols - 1) * d1.grid.gap) / d1.grid.cols;
      const cellH = (h - (d1.grid.rows - 1) * d1.grid.gap) / d1.grid.rows;
      // A level that is fading out may already be detached: keep its last good size.
      if (Number.isFinite(cellW) && Number.isFinite(cellH) && cellW > 0 && cellH > 0) setMetrics({ cellW, cellH });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const focusNode = disk.nodes[focus];
  const kids = focusNode.children.map((k) => disk.nodes[k]).filter((k) => k.size > 0);
  // At most 12 tiles; the rest collapse into a "+N more" tile.
  const overflow = kids.length > d1.grid.maxTiles;
  const shown = overflow ? kids.slice(0, d1.grid.maxTiles - 1) : kids;
  const rest = overflow ? kids.slice(d1.grid.maxTiles - 1) : [];
  const restSize = rest.reduce((s, k) => s + k.size, 0);
  const cells = useMemo(
    () =>
      gridLayout(
        // "+N more" always lands last (bottom-right): its weight is capped at the smallest shown tile.
        [...shown.map((k) => ({ id: k.id, size: k.size })), ...(overflow ? [{ id: MORE_ID, size: Math.min(restSize, shown[shown.length - 1].size * 0.99) }] : [])],
        d1.grid.cols,
        d1.grid.rows,
        d1.grid.emphasis,
        d1.grid.minCells,
        d1.grid.maxShare,
      ),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [focus, shown.map((k) => `${k.id}:${k.size}`).join(","), restSize],
  );
  const parentSize = focusNode.size - (disk.nodes[disk.ghostId]?.parent === focus ? disk.nodes[disk.ghostId].size : 0);

  // Sonar reveal (home only): each tile appears as the sweep passes its center.
  const revealDelay = (c: Cell) => {
    const cx = c.col - 1 + c.colSpan / 2 - d1.grid.cols / 2;
    const cy = (c.row - 1 + c.rowSpan / 2 - d1.grid.rows / 2) * (metrics.cellH / Math.max(1, metrics.cellW));
    const max = Math.hypot(d1.grid.cols / 2, d1.grid.rows / 2);
    return reduced ? 0 : (Math.hypot(cx, cy) / max) * mapMotion.sonar * 0.6;
  };

  function open(n: DiskNode) {
    if (isDrillable(n)) setFocus(n.id);
    else select(n.id);
  }

  const tiles = cells.map((cell) => {
    if (cell.id === MORE_ID) {
      return <MoreTile key="more" cell={cell} focus={focusNode} count={rest.length} size={restSize} onOpen={() => setViewMode("list")} />;
    }
    const n = disk.nodes[cell.id];
    if (n.kind === "ghost") {
      return <GhostTile key={n.id} node={n} cell={cell} units={units} onSelect={() => select(n.id)} onFree={freeGhost} />;
    }
    return (
      <Tile
        key={`${n.id}-${root ? scanKey : 0}`}
        disk={disk}
        node={n}
        cell={cell}
        metrics={metrics}
        parentSize={parentSize}
        scheme={scheme}
        units={units}
        lens={lens}
        selected={selected === n.id}
        deleting={deleting.includes(n.id)}
        pulseKey={pulse?.id === n.id ? pulse.key : null}
        reveal={root && scan === "scanning" ? { key: scanKey, delay: revealDelay(cell) } : null}
        drillable={isDrillable(n)}
        onOpen={open}
      />
    );
  });

  // The whole level fades in and out as one piece.
  const fade = {
    initial: { opacity: 0 },
    animate: { opacity: 1, transition: { duration: duration.fast, ease: ease.out } },
    exit: { opacity: 0, transition: { duration: duration.fast, ease: ease.inOut } },
  };
  if (root) {
    return (
      <motion.div className="level" {...fade}>
        <div ref={gridRef} className="home-grid">
          {tiles}
        </div>
      </motion.div>
    );
  }

  // A drilled-in level: the opened tile's frame fills the grid area, children sit inside it.
  const c = categories[scheme][focusNode.category];
  const frameStyle = { "--from": c.from, "--to": c.to, "--tint": c.border, "--glow": c.glow } as CSSProperties;
  return (
    <motion.div className="level" {...fade}>
      <div className="level-frame" style={frameStyle} />
      <div ref={gridRef} className="home-grid level-grid">
        {tiles}
      </div>
    </motion.div>
  );
}

function MoreTile({ cell, focus, count, size, onOpen }: { cell: Cell; focus: DiskNode; count: number; size: number; onOpen: () => void }) {
  const scheme = useScheme();
  const units = useApp((s) => s.info?.units ?? "decimal");
  const c = categories[scheme][focus.category];
  const f = formatBytes(size, units);
  const style = {
    gridColumn: `${cell.col} / span ${cell.colSpan}`,
    gridRow: `${cell.row} / span ${cell.rowSpan}`,
    "--acc": c.accent,
    "--ink": c.ink,
    "--from": c.from,
    "--to": c.to,
    "--tint": c.border,
    "--glow": c.glow,
  } as CSSProperties;
  return (
    <motion.button
      type="button"
      className="tile tile-more focus-ring"
      style={style}
      onClick={onOpen}
    >
      <span className="tile-head">
        <span className="tile-badge">
          <Icon name="list" tone="white" size={20} />
        </span>
        <span className="tile-name tile-name-keep">{t("home.more", { count })}</span>
        <span className="tile-meta">
          {f.value} {f.unit}
        </span>
      </span>
      <span className="tile-spacer" />
      <span className="tile-open">{t("home.showAll")}</span>
    </motion.button>
  );
}

