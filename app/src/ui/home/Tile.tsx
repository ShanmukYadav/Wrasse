import { motion } from "motion/react";
import type { CSSProperties } from "react";
import { api, type Disk, type DiskNode } from "../../api";
import { Icon } from "../../design/icons/Icon";
import { iconFor } from "../../design/icons/semantic";
import { categories, d1, type LensId, type Scheme } from "../../design/tokens";
import { t } from "../../i18n";
import { formatBytes, type UnitSystem } from "../../lib/format";
import { duration, ease, spring } from "../../motion/tokens";
import type { Cell } from "./gridLayout";
import { MiddleEllipsis } from "./MiddleEllipsis";

export type TileMetrics = { cellW: number; cellH: number };

type Props = {
  disk: Disk;
  node: DiskNode;
  cell: Cell;
  metrics: TileMetrics;
  parentSize: number;
  scheme: Scheme;
  units: UnitSystem;
  lens: LensId;
  selected: boolean;
  deleting: boolean;
  pulseKey: number | null;
  reveal: { key: number; delay: number } | null;
  drillable?: boolean;
  onOpen: (node: DiskNode) => void;
};

const UNIT: Partial<Record<DiskNode["category"], string>> = { files: "unit.folders", aiModels: "unit.models", apps: "unit.apps" };

export function lensMatch(n: DiskNode, lens: LensId): boolean {
  return lens === "all" || (n.matchBytes[lens] ?? 0) > 0;
}

// One D1 tile: badge + name + meta, big size, top items as rows, insight chips.
export function Tile({ disk, node, cell, metrics, parentSize, scheme, units, lens, selected, deleting, pulseKey, reveal, drillable = node.children.length > 0, onOpen }: Props) {
  const c = categories[scheme][node.category];
  const cells = cell.colSpan * cell.rowSpan;
  const size = formatBytes(node.size, units);
  const sizeFont = cells >= 30 ? d1.tile.sizeXL : cells >= 12 ? d1.tile.sizeL : cells >= 4 ? d1.tile.sizeM : d1.tile.sizeS;
  const heightPx = cell.rowSpan * metrics.cellH + (cell.rowSpan - 1) * d1.grid.gap;
  const widthPx = cell.colSpan * metrics.cellW + (cell.colSpan - 1) * d1.grid.gap;
  const share = Math.round((node.size / Math.max(1, parentSize)) * 100);
  // Category tiles count in their own unit (folders, models, apps); deeper tiles count items.
  const unitKey = node.depth === 1 ? UNIT[node.category] : "unit.items";
  // Actionable items first; read-only libraries (Photos, Music) collapse into "+N more".
  const kids = node.children
    .map((k) => disk.nodes[k])
    .filter((k) => k.size > 0)
    .sort((a, b) => Number(!!a.readOnly) - Number(!!b.readOnly) || b.size - a.size);

  // Rows: XL tiles show five items plus "+N more"; L tiles show their top three.
  const rowRoom = Math.floor((heightPx - 160 - sizeFont) / (d1.row.height + d1.row.stackGap));
  const maxRows = sizeFont === d1.tile.sizeXL ? 6 : sizeFont === d1.tile.sizeL ? 3 : 0;
  // Single items (apps, models, files) show no rows: their parts aren't separate choices.
  const rowCount = drillable && widthPx >= 220 ? Math.max(0, Math.min(maxRows, rowRoom)) : 0;
  let rows = kids.slice(0, rowCount);
  let more: { count: number; size: number } | null = null;
  if (rowCount >= 5 && kids.length > rowCount) {
    rows = kids.slice(0, rowCount - 1);
    const rest = kids.slice(rowCount - 1);
    more = { count: rest.length, size: rest.reduce((s, k) => s + k.size, 0) };
  }
  const maxKid = Math.max(1, ...kids.map((k) => k.size));
  const large = rows.length > 0;
  const meta = large
    ? unitKey
      ? t("home.metaCount", { share: share < 1 ? "<1" : share, count: kids.length, unit: t(unitKey) })
      : t("home.metaShare", { share: share < 1 ? "<1" : share })
    : node.category === "apps" && node.depth === 1
      ? t("home.apps", { count: kids.length })
      : "";
  const insights = api.insights(disk, node.id);
  const dimmed = !lensMatch(node, lens);

  const style = {
    gridColumn: `${cell.col} / span ${cell.colSpan}`,
    gridRow: `${cell.row} / span ${cell.rowSpan}`,
    "--acc": c.accent,
    "--ink": c.ink,
    "--from": c.from,
    "--to": c.to,
    "--tint": c.border,
    "--glow": c.glow,
    "--badge-shadow": c.shadow,
  } as CSSProperties;

  return (
    <motion.button
      type="button"
      layout
      layoutId={`tile-${node.id}`}
      data-tile-id={node.id}
      className={`tile focus-ring ${selected ? "is-selected" : ""} ${dimmed ? "is-dimmed" : ""}`}
      style={style}
      aria-label={`${node.name}, ${size.value} ${size.unit}`}
      onClick={() => onOpen(node)}
      initial={reveal ? { opacity: 0, scale: 0.94 } : false}
      animate={deleting ? { opacity: 0, scale: 0.85 } : { opacity: 1, scale: 1 }}
      transition={
        deleting
          ? { duration: duration.base, ease: ease.inOut }
          : reveal
            ? { ...spring.soft, delay: reveal.delay, opacity: { duration: duration.slow, ease: ease.out, delay: reveal.delay } }
            : spring.soft
      }
    >
      <span className="tile-glow" aria-hidden />
      <span className="tile-head">
        {/* Narrow tiles drop the badge so the name stays readable. */}
        {widthPx >= 120 && (
          <span className="tile-badge">
            <Icon name={iconFor(node)} tone="white" size={20} />
          </span>
        )}
        <span className="tile-name">{node.name}</span>
        {meta && <span className="tile-meta">{meta}</span>}
      </span>
      <span className="tile-size">
        <span className="tile-size-num" style={{ fontSize: sizeFont }}>
          {size.value}
        </span>
        <span className="tile-size-unit">{size.unit}</span>
      </span>
      {large && (
        <span className="tile-rows">
          {rows.map((k) => (
            <Row key={k.id} node={k} max={maxKid} units={units} dimmed={!lensMatch(k, lens)} />
          ))}
          {more && (
            <span className="tile-row">
              <Icon name="folder" tone={node.category} size={16} />
              <span className="tile-row-name">
                {node.category === "files" ? t("home.moreFolders", { count: more.count }) : t("home.more", { count: more.count })}
              </span>
              <SizeBar fraction={more.size / maxKid} />
              <span className="tile-row-size">{fmt(more.size, units)}</span>
            </span>
          )}
        </span>
      )}
      <span className="tile-spacer" />
      <span className="tile-foot">
        <span className="tile-chips">
          {insights.map((i) => (
            <span key={i.text} className={`chip chip-${i.tone}`}>
              {i.text}
            </span>
          ))}
        </span>
        <span className="tile-open" aria-hidden>
          {drillable ? t("home.open") : t("home.details")}
        </span>
      </span>
      {selected && (
        <span className="tile-check" aria-hidden>
          <Icon name="check" tone="white" size={16} />
        </span>
      )}
      {pulseKey !== null && <span key={pulseKey} className="tile-pulse" aria-hidden />}
    </motion.button>
  );
}

function Row({ node, max, units, dimmed }: { node: DiskNode; max: number; units: UnitSystem; dimmed: boolean }) {
  return (
    <span className={`tile-row ${dimmed ? "is-dimmed" : ""}`}>
      <Icon name={iconFor(node)} tone={node.category} size={16} />
      <MiddleEllipsisInline text={node.name} />
      <SizeBar fraction={node.size / max} />
      <span className="tile-row-size">{fmt(node.size, units)}</span>
    </span>
  );
}

function MiddleEllipsisInline({ text }: { text: string }) {
  return <MiddleEllipsis text={text} className="tile-row-name" />;
}

function SizeBar({ fraction }: { fraction: number }) {
  return (
    <span className="size-bar" aria-hidden>
      <span className="size-bar-fill" style={{ transform: `scaleX(${Math.max(0.02, Math.min(1, fraction))})` }} />
    </span>
  );
}

function fmt(bytes: number, units: UnitSystem) {
  const f = formatBytes(bytes, units);
  return `${f.value} ${f.unit}`;
}
