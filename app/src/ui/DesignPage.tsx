// Hidden "Design" page (command: "design system"): every icon in every tone and size, logo
// variants, color tokens, the type scale and sample blocks of each item type in both themes.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useRef } from "react";
import type { Disk } from "../api/types";
import { BrandMark } from "../design/brand/BrandMark";
import { Icon } from "../design/icons/Icon";
import { ICONS, type IconName } from "../design/icons/nodes";
import { iconFor } from "../design/icons/semantic";
import {
  brand, categoryShades, icon as iconTokens, lensColors, map as mapTokens, palette, tones, type as typeScale,
  type Category, type LensId, type Scheme, type Tone,
} from "../design/tokens";
import { t } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import { MapRenderer } from "../map/renderer";
import { useApp } from "../store/app";

const TONES = Object.keys(tones.dark) as Tone[];
const ICON_NAMES = Object.keys(ICONS) as IconName[];
const SCHEMES: Scheme[] = ["dark", "light"];

export function DesignPage() {
  const open = useApp((s) => s.overlay === "design");
  const setOverlay = useApp((s) => s.setOverlay);
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOverlay(null);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, setOverlay]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="design-page"
          role="dialog"
          aria-modal="true"
          aria-label={t("design.title")}
          initial={{ opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1, transition: spring.soft }}
          exit={{ opacity: 0, scale: 0.98, transition: { duration: duration.fast, ease: ease.inOut } }}
        >
          <div className="design-inner">
            <header className="design-header">
              <BrandMark size={32} />
              <div className="flex flex-col">
                <h1 className="text-title">{t("design.title")}</h1>
                <span className="text-label text-muted">{t("design.subtitle")}</span>
              </div>
              <button type="button" className="icon-btn focus-ring ml-auto" aria-label={t("detail.close")} onClick={() => setOverlay(null)}>
                <Icon name="close" size={20} />
              </button>
            </header>
            <Logos />
            <IconSizes />
            <IconTones />
            <Colors />
            <TypeScale />
            <SampleBlocks />
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Section({ title, note, children }: { title: string; note?: string; children: React.ReactNode }) {
  return (
    <section className="design-section">
      <h2 className="design-h2">{title}</h2>
      {note && <p className="text-label text-muted">{note}</p>}
      {children}
    </section>
  );
}

function Logos() {
  return (
    <Section title={t("design.logo")} note={t("design.logoNote")}>
      <div className="design-row">
        <figure className="design-figure">
          <BrandMark variant="appIcon" size={160} />
          <figcaption>{t("design.appIcon")}</figcaption>
        </figure>
        <figure className="design-figure">
          <div className="flex items-end gap-4">
            {[64, 32, 16].map((s) => (
              <BrandMark key={s} variant="appIcon" size={s} />
            ))}
          </div>
          <figcaption>64 · 32 · 16</figcaption>
        </figure>
        <figure className="design-figure">
          <div className="flex items-end gap-4">
            {[64, 32, 24, 16].map((s) => (
              <BrandMark key={s} size={s} />
            ))}
          </div>
          <figcaption>{t("design.markDuotone")}</figcaption>
        </figure>
        <figure className="design-figure">
          <div className="flex gap-3">
            <div className="tray-sample" style={{ background: brand.trayPreview.light }}>
              {[32, 22, 16].map((s) => (
                <BrandMark key={s} variant="solid" ink="light" size={s} />
              ))}
            </div>
            <div className="tray-sample" style={{ background: brand.trayPreview.dark }}>
              {[32, 22, 16].map((s) => (
                <BrandMark key={s} variant="solid" ink="dark" size={s} />
              ))}
            </div>
          </div>
          <figcaption>{t("design.tray")}</figcaption>
        </figure>
      </div>
    </Section>
  );
}

function IconSizes() {
  return (
    <Section title={t("design.icons")} note={t("design.iconsNote", { stroke: iconTokens.stroke, fill: iconTokens.fillOpacity * 100 })}>
      <div className="icon-grid">
        {ICON_NAMES.map((name) => (
          <div key={name} className="icon-card">
            <div className="flex items-end gap-3">
              {iconTokens.sizes.map((s) => (
                <Icon key={s} name={name} size={s} tone="neutral" />
              ))}
            </div>
            <span className="text-caption text-muted">{name}</span>
          </div>
        ))}
      </div>
    </Section>
  );
}

function IconTones() {
  return (
    <Section title={t("design.tones")}>
      <div className="tone-table" role="table">
        <div className="tone-row" role="row">
          <span className="tone-name" />
          {TONES.map((tone) => (
            <span key={tone} className="tone-head text-caption text-muted" role="columnheader">
              {tone}
            </span>
          ))}
        </div>
        {ICON_NAMES.map((name) => (
          <div key={name} className="tone-row" role="row">
            <span className="tone-name text-caption text-muted">{name}</span>
            {TONES.map((tone) => (
              <span key={tone} className="tone-cell" role="cell">
                <Icon name={name} tone={tone} size={20} />
              </span>
            ))}
          </div>
        ))}
      </div>
    </Section>
  );
}

function Swatch({ color, name }: { color: string; name: string }) {
  return (
    <div className="swatch">
      <span className="swatch-chip" style={{ background: color }} />
      <span className="text-caption">{name}</span>
      <span className="text-caption text-muted">{color.length > 24 ? "" : color}</span>
    </div>
  );
}

function Colors() {
  return (
    <Section title={t("design.colors")}>
      {SCHEMES.map((scheme) => (
        <div key={scheme} className="swatch-board" style={{ background: palette[scheme].canvas, color: palette[scheme].text }}>
          <h3 className="design-h3">{t(scheme === "dark" ? "settings.night" : "settings.day")}</h3>
          <div className="swatch-grid">
            {Object.entries(palette[scheme]).filter(([, v]) => !v.startsWith("linear")).map(([k, v]) => (
              <Swatch key={k} name={k} color={v} />
            ))}
          </div>
          <h3 className="design-h3">{t("design.categories")}</h3>
          <div className="swatch-grid">
            {(Object.keys(categoryShades[scheme]) as Category[]).map((c) => (
              <div key={c} className="swatch">
                <span className="flex">
                  {categoryShades[scheme][c].map((v) => (
                    <span key={v} className="swatch-chip swatch-chip-sm" style={{ background: v }} />
                  ))}
                </span>
                <span className="text-caption">{c}</span>
              </div>
            ))}
          </div>
          <h3 className="design-h3">{t("design.lensColors")}</h3>
          <div className="swatch-grid">
            {(Object.entries(lensColors[scheme]) as [LensId, string][]).map(([k, v]) => (
              <Swatch key={k} name={k} color={v} />
            ))}
          </div>
          <h3 className="design-h3">{t("design.iconTones")}</h3>
          <div className="swatch-grid">
            {Object.entries(tones[scheme]).map(([k, v]) => (
              <Swatch key={k} name={k} color={v} />
            ))}
          </div>
        </div>
      ))}
      <div className="swatch-grid">
        <Swatch name="brand.iconFrom" color={brand.iconFrom} />
        <Swatch name="brand.iconTo" color={brand.iconTo} />
        <Swatch name="brand.stripe" color={brand.stripe} />
      </div>
    </Section>
  );
}

function TypeScale() {
  return (
    <Section title={t("design.type")} note={t("design.typeNote")}>
      <div className="flex flex-col gap-3">
        {Object.entries(typeScale).map(([k, v]) => (
          <div key={k} className="type-row">
            <span className="text-caption text-muted type-key">
              {k} · {v}
            </span>
            <span style={{ fontSize: v }} className="type-sample">
              <span className="type-name">Macintosh HD</span> <span className="type-size">402 GB</span>{" "}
              <span className="type-hint">{t("header.free")}</span>
            </span>
          </div>
        ))}
      </div>
    </Section>
  );
}

const SAMPLE_TYPES: IconName[] = [
  "folder", "app", "aiModel", "video", "photo", "audio", "document", "installer", "diskImage", "archive",
  "nodeModules", "pythonEnv", "xcode", "packageCache", "log", "cache", "system", "ghost",
];

function SampleBlocks() {
  const disk = useApp((s) => s.disk);
  const samples = useMemo(() => pickSamples(disk), [disk]);
  // A top-level folder that holds some junk, to show a lens fill.
  const junkSample = useMemo(() => disk.nodes.find((n) => n.depth === 1 && (n.matchBytes.junk ?? 0) > 0)?.id, [disk]);
  return (
    <Section title={t("design.blocks")} note={t("design.blocksNote")}>
      {SCHEMES.map((scheme) => (
        <div key={scheme} className="sample-board" style={{ background: palette[scheme].canvas, color: palette[scheme].text }}>
          <h3 className="design-h3">{t(scheme === "dark" ? "settings.night" : "settings.day")}</h3>
          <div className="sample-grid">
            {samples.map(({ type, id }) => (
              <SampleTile key={`${type}-${scheme}`} disk={disk} id={id} scheme={scheme} />
            ))}
            {junkSample !== undefined && <SampleTile disk={disk} id={junkSample} scheme={scheme} lens="junk" />}
          </div>
        </div>
      ))}
    </Section>
  );
}

function pickSamples(disk: Disk): { type: IconName; id: number }[] {
  const out: { type: IconName; id: number }[] = [];
  for (const type of SAMPLE_TYPES) {
    const n = disk.nodes.find((n) => n.parent >= 0 && n.size > 0 && iconFor(n) === type && (type !== "folder" || n.depth === 1));
    if (n) out.push({ type, id: n.id });
  }
  return out;
}

const SAMPLE = { w: 220, h: 140 };

function SampleTile({ disk, id, scheme, lens = "all" }: { disk: Disk; id: number; scheme: Scheme; lens?: LensId }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const units = useApp((s) => s.info?.units ?? "decimal");
  useEffect(() => {
    const canvas = ref.current!;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = SAMPLE.w * dpr;
    canvas.height = SAMPLE.h * dpr;
    const r = new MapRenderer(document.createElement("canvas"), disk);
    r.scheme = scheme;
    r.units = units;
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = mapTokens[scheme].background;
    ctx.fillRect(0, 0, SAMPLE.w, SAMPLE.h);
    r.paintSample(ctx, { x: 6, y: 6, w: SAMPLE.w - 12, h: SAMPLE.h - 12 }, id, lens, dpr);
  }, [disk, id, scheme, lens, units]);
  return <canvas ref={ref} className="sample-tile" style={{ width: SAMPLE.w, height: SAMPLE.h }} aria-label={disk.nodes[id].name} />;
}
