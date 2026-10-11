import { AnimatePresence, LayoutGroup, motion } from "motion/react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { DiskNode } from "../../api";
import { categories, d1 } from "../../design/tokens";
import { t } from "../../i18n";
import { formatBytes } from "../../lib/format";
import { useScheme } from "../../lib/prefs";
import { duration, ease } from "../../motion/tokens";
import { useApp } from "../../store/app";
import { GhostTile } from "./GhostTile";
import { gridLayout, type Cell } from "./gridLayout";
import { Sonar } from "./Sonar";
import { Icon } from "../../design/icons/Icon";
import { isDrillable } from "./nodes";
import { ScopeEmpty } from "../ScopeEmpty";
import { filtersOn, scopeBits, visibleRows } from "../scope";
import { Tile, type TileMetrics } from "./Tile";

const MORE_ID = -2;

// Turns on `ms` after `on` becomes true, and off again as soon as `on` is false.
function useDelayed(on: boolean, ms: number): boolean {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    if (!on) {
      setReady(false);
      return;
    }
    const id = window.setTimeout(() => setReady(true), ms);
    return () => window.clearTimeout(id);
  }, [on, ms]);
  return ready;
}

// Home: one level of the disk at a time. Moving between levels is a short crossfade: no zoom,
// no sweep, no staggered reveal (the user found those distracting).
export function Home() {
  const focus = useApp((s) => s.focus);
  const disk = useApp((s) => s.disk);
  const scan = useApp((s) => s.scan);
  const scanKey = useApp((s) => s.scanKey);
  const visible = useApp((s) => s.viewMode === "map");
  // Sonar mounts after the first paint, so its rings are not in the opening composite.
  const showSonar = useDelayed(scan === "scanning", 400);
  return (
    <main className={`home ${visible ? "" : "is-hidden"}`} aria-hidden={!visible}>
      <LayoutGroup>
        <AnimatePresence initial={false}>
          <Level key={focus} focus={focus} root={focus === disk.root} />
        </AnimatePresence>
      </LayoutGroup>
      {showSonar && <Sonar key={scanKey} />}
    </main>
  );
}

function Level({ focus, root }: { focus: number; root: boolean }) {
  const gridRef = useRef<HTMLDivElement>(null);
  const [metrics, setMetrics] = useState<TileMetrics>({ cellW: 80, cellH: 60 });
  const disk = useApp((s) => s.disk);
  useApp((s) => s.diskVersion);
  const lens = useApp((s) => s.lens);
  useEffect(() => {
    if (lens === "aiModels") void useApp.getState().refreshLoaded();
  }, [lens]);
  const filters = useApp((s) => s.filters);
  const plan = useApp((s) => s.plan);
  const selected = useApp((s) => s.selected);
  const deleting = useApp((s) => s.deleting);
  const pulse = useApp((s) => s.pulse);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const scheme = useScheme();
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
      if (Number.isFinite(cellW) && Number.isFinite(cellH) && cellW > 0 && cellH > 0) {
        // Same size must not store a new object: that re-render was allocating a fresh set of
        // tile layers on every resize observation, and the web view's memory climbed for minutes.
        setMetrics((m) => (Math.abs(m.cellW - cellW) < 0.5 && Math.abs(m.cellH - cellH) < 0.5 ? m : { cellW, cellH }));
      }
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

  function open(n: DiskNode) {
    if (isDrillable(n)) setFocus(n.id);
    else select(n.id);
  }

  const bits = scopeBits(lens, filters);
  const showBanner = !plan && (lens !== "all" || filtersOn(filters)) && visibleRows(disk, focus, lens, filters).length === 0;
  const banner = showBanner ? <ScopeEmpty banner /> : null;

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
        key={n.id}
        disk={disk}
        node={n}
        cell={cell}
        metrics={metrics}
        parentSize={parentSize}
        scheme={scheme}
        units={units}
        scopeBits={bits}
        selected={selected === n.id}
        deleting={deleting.includes(n.id)}
        pulseKey={pulse?.id === n.id ? pulse.key : null}
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
  // The home level is a plain div. Fading it (or each tile) promotes a backing store the
  // size of the grid, and the launch stays under the memory budget only without that store.
  if (root) {
    return (
      <div className="level">
        {banner}
        <div ref={gridRef} className="home-grid">
          {tiles}
        </div>
      </div>
    );
  }

  // A drilled-in level: the opened tile's frame fills the grid area, children sit inside it.
  const c = categories[scheme][focusNode.category];
  const frameStyle = { "--from": c.from, "--to": c.to, "--tint": c.border, "--glow": c.glow } as CSSProperties;
  return (
    <motion.div className="level" {...fade}>
      {banner}
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
    <button
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
    </button>
  );
}

