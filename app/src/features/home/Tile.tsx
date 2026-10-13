import { useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { api, deletable, type Disk, type DiskNode } from "../../api";
import { useApp } from "../../store/app";
import { Icon } from "../../design/icons/Icon";
import { iconFor } from "../../design/icons/semantic";
import { categories, d1, type Scheme } from "../../design/tokens";
import { useTilePaint } from "../../design/tilePaint";
import { t } from "../../i18n";
import { bytesText, formatBytes, type UnitSystem } from "../../lib/format";
import type { Cell } from "./gridLayout";
import { modelCaption } from "../models/modelLine";
import { matchesScope } from "../browse/scope";

export type TileMetrics = { cellW: number; cellH: number };

type Props = {
  disk: Disk;
  node: DiskNode;
  cell: Cell;
  metrics: TileMetrics;
  parentSize: number;
  scheme: Scheme;
  units: UnitSystem;
  scopeBits: number;
  selected: boolean;
  deleting: boolean;
  pulseKey: number | null;
  drillable?: boolean;
  onOpen: (node: DiskNode) => void;
};

const UNIT: Partial<Record<DiskNode["category"], string>> = { files: "unit.folders", aiModels: "unit.models", apps: "unit.apps" };

// One D1 tile: badge + name + meta, big size, top items as rows, insight chips.
export function Tile({ disk, node, cell, metrics, parentSize, scheme, units, scopeBits, selected, deleting, pulseKey, drillable = node.children.length > 0, onOpen }: Props) {
  const c = categories[scheme][node.category];
  const cells = cell.colSpan * cell.rowSpan;
  const size = formatBytes(node.size, units);
  const sizeFont = cells >= 30 ? d1.tile.sizeXL : cells >= 12 ? d1.tile.sizeL : cells >= 4 ? d1.tile.sizeM : d1.tile.sizeS;
  const heightPx = cell.rowSpan * metrics.cellH + (cell.rowSpan - 1) * d1.grid.gap;
  const widthPx = cell.colSpan * metrics.cellW + (cell.colSpan - 1) * d1.grid.gap;
  // Small tiles degrade gracefully so the size is always fully visible:
  // short tiles become one line (name + size), medium tiles drop the chips.
  const short = heightPx < d1.tile.shortBelow;
  const chipsRoom = !short && heightPx >= d1.tile.chipsFrom && widthPx >= d1.tile.chipsMinWidth;
  const fittedSizeFont = Math.min(sizeFont, Math.max(d1.tile.sizeS, (heightPx - (chipsRoom ? 112 : 78)) * 0.9));
  const share = Math.round((node.size / Math.max(1, parentSize)) * 100);
  const shareText = share < 1 ? "<1" : share;
  // Category tiles count in their own unit (folders, models, apps). Junk and the rest count items.
  const unitKey = (node.depth === 1 ? UNIT[node.category] : undefined) ?? "unit.items";
  // Actionable items first; read-only libraries (Photos, Music) collapse into "+N more".
  const kids = node.children
    .map((k) => disk.nodes[k])
    .filter((k) => k.size > 0)
    .sort((a, b) => Number(!!a.readOnly) - Number(!!b.readOnly) || b.size - a.size);
  // The AI Models tile previews models, which sit under each tool's folder.
  const listed = node.depth === 1 && node.category === "aiModels" ? modelRows(disk, node.id) : kids;

  // Rows: XL tiles show five items plus "+N more"; L tiles show their top three.
  const rowRoom = Math.floor((heightPx - 160 - sizeFont) / (d1.row.height + d1.row.stackGap));
  const maxRows = sizeFont === d1.tile.sizeXL ? 6 : sizeFont === d1.tile.sizeL ? 3 : 0;
  // Single items (apps, models, files) show no rows: their parts aren't separate choices.
  const fitRows = drillable && widthPx >= 220 ? Math.max(0, Math.min(maxRows, rowRoom)) : 0;
  // Names are never cut short: they wrap instead, so a row can take two lines. When the tile then
  // runs out of room, it drops a row (into "+N more"), then its chips, until everything fits.
  const fitKey = `${node.id}:${widthPx}:${heightPx}:${listed.length}`;
  const [fit, setFit] = useState({ key: fitKey, drop: 0 });
  const drop = fit.key === fitKey ? fit.drop : 0;
  const rowCount = Math.max(0, fitRows - drop);
  const chipsFit = drop <= fitRows;
  let rows = listed.slice(0, rowCount);
  let more: { count: number; size: number } | null = null;
  if (rowCount >= 5 && listed.length > rowCount) {
    rows = listed.slice(0, rowCount - 1);
    const rest = listed.slice(rowCount - 1);
    more = { count: rest.length, size: rest.reduce((s, k) => s + k.size, 0) };
  }
  const maxKid = Math.max(1, ...listed.map((k) => k.size));
  const large = rows.length > 0;
  // Models are counted wherever they sit (a project folder can hold several), apps likewise.
  const count = node.depth === 1 && (node.category === "aiModels" || node.category === "apps") ? countKind(disk, node.id, node.category === "aiModels" ? "model" : "app") : kids.length;
  // Every top-level block shows its share, including Junk, which often has no rows.
  const meta = node.depth === 1
    ? t("home.metaCount", { share: shareText, count, unit: t(unitKey) })
    : large
      ? t("home.metaCount", { share: shareText, count, unit: t("unit.items") })
      : node.kind === "model" && node.model
        ? modelCaption(node)
      : node.kind === "app" && node.app && widthPx >= 340
        ? node.app.neverOpened
          ? t("app.neverOpened")
          : node.lastOpenedDays === 0
            ? t("app.openedToday")
            : t("app.opened", { when: t("detail.daysAgo", { count: node.lastOpenedDays }) })
        : "";
  const insights = api.insights(disk, node.id);
  const showChips = chipsRoom && chipsFit;
  // Ticks for bulk delete: shown on hover, and on every tile once something is ticked.
  const checked = useApp((s) => s.checked.includes(node.id));
  const selecting = useApp((s) => s.checked.length > 0);
  // Whole home categories (Your Files, Apps, …) and macOS itself have no tick: open them to choose.
  const tickable = deletable(disk, node.id) && widthPx >= 120;
  const dimmed = scopeBits !== 0 && !matchesScope(disk, node.id, scopeBits);

  // The fill and corner glow as images (see design/tilePaint.ts).
  const paintRef = useRef<HTMLButtonElement>(null);
  useTilePaint(paintRef, c.from, c.to, { color: c.glow, size: d1.tile.glowSize });
  useLayoutEffect(() => {
    const el = paintRef.current;
    if (el && el.scrollHeight > el.clientHeight + 1 && drop <= fitRows) setFit({ key: fitKey, drop: drop + 1 });
  });
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

  // A plain button. Gradients and shadows paint here. No transform on the tile: that would
  // give every block its own backing store for as long as the window is open.
  return (
    <button
      ref={paintRef}
      type="button"
      data-tile-id={node.id}
      className={`tile focus-ring ${selected || checked ? "is-selected" : ""} ${dimmed ? "is-dimmed" : ""} ${deleting ? "is-deleting" : ""} ${tickable ? "has-tick" : ""} ${tickable && (selecting || checked) ? "is-ticking" : ""}`}
      style={style}
      aria-label={`${node.name}, ${size.value} ${size.unit}`}
      onClick={() => onOpen(node)}
    >
      <span className={`tile-head ${short ? "is-short" : ""}`}>
        {/* Narrow tiles drop the badge so the name stays readable. */}
        {widthPx >= 120 && (
          <span className="tile-badge">
            <Icon name={iconFor(node)} tone="white" size={20} />
          </span>
        )}
        {/* Name and meta share a line when they fit; otherwise the meta moves under the name. */}
        <span className="tile-title">
          <span className="tile-name">{node.name}</span>
          {short
            ? node.depth === 1 && <span className="tile-meta">{t("home.metaShare", { share: shareText })}</span>
            : meta && <span className="tile-meta">{meta}</span>}
        </span>
        {short && (
          <span className="tile-size-inline">
            {size.value} {size.unit}
          </span>
        )}
      </span>
      {!short && (
        <span className="tile-size">
          <span className="tile-size-num" style={{ fontSize: fittedSizeFont }}>
            {size.value}
          </span>
          <span className="tile-size-unit">{size.unit}</span>
        </span>
      )}
      {large && (
        <span className="tile-rows">
          {rows.map((k) => (
            <Row key={k.id} node={k} max={maxKid} units={units} bar={widthPx >= d1.tile.rowBarMinWidth} dimmed={!dimmed && scopeBits !== 0 && !matchesScope(disk, k.id, scopeBits)} />
          ))}
          {more && (
            <span className="tile-row">
              <Icon name="folder" tone={node.category} size={16} />
              <span className="tile-row-name">
                {node.category === "files" ? t("home.moreFolders", { count: more.count }) : t("home.more", { count: more.count })}
              </span>
              {widthPx >= d1.tile.rowBarMinWidth && <SizeBar fraction={more.size / maxKid} />}
              <span className="tile-row-size">{bytesText(more.size, units)}</span>
            </span>
          )}
        </span>
      )}
      {!short && <span className="tile-spacer" />}
      {!short && (
        <span className="tile-foot">
          <span className="tile-chips">
            {showChips &&
              insights.map((i) => (
                <span key={i.text} className={`chip chip-${i.tone}`}>
                  {i.text}
                </span>
              ))}
          </span>
          {widthPx >= d1.tile.openMinWidth && (
            <span className="tile-open" aria-hidden>
              {drillable ? t("home.open") : t("home.details")}
            </span>
          )}
        </span>
      )}
      {selected && !checked && (
        <span className="tile-check" aria-hidden>
          <Icon name="check" tone="white" size={16} />
        </span>
      )}
      {tickable && (
        <span
          role="checkbox"
          aria-checked={checked}
          aria-label={t("select.tick", { name: node.name })}
          className={`tile-tick ${checked ? "is-on" : ""} ${selecting ? "is-visible" : ""}`}
          onClick={(e) => {
            e.stopPropagation();
            useApp.getState().toggleCheck(node.id);
          }}
        >
          {checked && <Icon name="check" tone="white" size={16} />}
        </span>
      )}
      {pulseKey !== null && <span key={pulseKey} className="tile-pulse" aria-hidden />}
    </button>
  );
}

function Row({ node, max, units, bar, dimmed }: { node: DiskNode; max: number; units: UnitSystem; bar: boolean; dimmed: boolean }) {
  // The quantization tag wraps with the name, and is left out when the name already says it.
  const quant = node.model?.quantization;
  const tag = quant && !node.name.toLowerCase().includes(quant.toLowerCase()) ? quant : null;
  return (
    <span className={`tile-row ${dimmed ? "is-dimmed" : ""}`}>
      <Icon name={iconFor(node)} tone={node.category} size={16} />
      <span className="tile-row-name">
        {node.name}
        {tag && <span className="tile-row-tag">{tag}</span>}
      </span>
      {bar && <SizeBar fraction={node.size / max} />}
      <span className="tile-row-size">{bytesText(node.size, units)}</span>
    </span>
  );
}

function SizeBar({ fraction }: { fraction: number }) {
  return (
    <span className="size-bar" aria-hidden>
      <span className="size-bar-fill" style={{ width: `${Math.max(2, Math.min(100, fraction * 100))}%` }} />
    </span>
  );
}

/** Model rows for the AI Models tile: the model itself, not the shard files inside it. */
function modelRows(disk: Disk, id: number): DiskNode[] {
  const out: DiskNode[] = [];
  const walk = (nid: number, underModel: boolean) => {
    for (const c of disk.nodes[nid].children) {
      const k = disk.nodes[c];
      if (k.size <= 0) continue;
      if (k.kind === "model" && !underModel) out.push(k);
      else walk(c, underModel || k.kind === "model");
    }
  };
  walk(id, false);
  out.sort((a, b) => b.size - a.size);
  return out;
}

/** Items of a kind anywhere under a node (models inside project folders, apps inside app folders). */
function countKind(disk: Disk, id: number, kind: DiskNode["kind"]): number {
  let n = 0;
  const stack = [...disk.nodes[id].children];
  while (stack.length) {
    const k = disk.nodes[stack.pop()!];
    if (k.size <= 0) continue;
    if (k.kind === kind) n++;
    else stack.push(...k.children);
  }
  return n;
}
