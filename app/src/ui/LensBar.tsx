import { motion } from "motion/react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Icon } from "../design/icons/Icon";
import type { CategoryId } from "../design/tokens";
import { t } from "../i18n";
import { bytesText } from "../lib/format";
import { useReducedMotion } from "../lib/prefs";
import { spring } from "../motion/tokens";
import { useApp } from "../store/app";
import { CATEGORIES, FILTERS } from "./lenses";
import { buttonBytes, buttonCount, filtersOn } from "./scope";

// Categories are one choice. Large & Old and Grew are filters on the current folder.
// The See count is buttonCount: the same rows the list shows for that scope.
export function LensBar() {
  const category = useApp((s) => s.lens);
  const filters = useApp((s) => s.filters);
  const plan = useApp((s) => s.plan);
  const disk = useApp((s) => s.disk);
  useApp((s) => s.diskVersion);
  const focus = useApp((s) => s.focus);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const setLens = useApp((s) => s.setLens);
  const toggleFilter = useApp((s) => s.toggleFilter);
  const reduced = useReducedMotion();
  const pills = useRef<Record<string, HTMLButtonElement | null>>({});
  const [box, setBox] = useState<{ left: number; width: number } | null>(null);
  const [glowKey, setGlowKey] = useState(0);
  const prevFilters = useRef<number | null>(null);
  const selecting = useApp((s) => s.checked.length > 0);
  const active: CategoryId | null = plan ? null : category;
  const filtering = !plan && filtersOn(filters);
  const count = filtering ? buttonCount(disk, focus, category, filters) : 0;
  const bytes = filtering && count > 0 ? buttonBytes(disk, focus, category, filters) : 0;
  const empty = filtering && count === 0;
  const hot = filtering && count > 0;
  const filterSig = (filters.largeOld ? 1 : 0) | (filters.grew ? 2 : 0);

  useEffect(() => {
    const before = prevFilters.current;
    prevFilters.current = filterSig;
    if (before === null || reduced || count === 0) return;
    // Glow once when a filter turns on and this folder has rows. Category changes do not.
    if ((filterSig & ~before) !== 0) setGlowKey((k) => k + 1);
  }, [filterSig, count, reduced]);

  useLayoutEffect(() => {
    const measure = () => {
      const el = active ? pills.current[active] : null;
      setBox(el ? { left: el.offsetLeft, width: el.offsetWidth } : null);
    };
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [active]);

  if (selecting) return null;

  const label = !filtering ? t("largeOld.showList") : empty ? t("scope.nothing") : t("lens.seeItems", { count, size: bytesText(bytes, units) });

  const open = () => {
    if (empty) return;
    const { setOverlay, setViewMode } = useApp.getState();
    // Large & Old keeps the move overlay. Grew alone opens the filtered list. No filter opens the overlay.
    if (filters.largeOld || !filtering) setOverlay("largeOld");
    else setViewMode("list");
  };

  return (
    <div className="lens-bar">
      <div className="lens-cats" role="radiogroup" aria-label={t("scope.categories")}>
        {box && (
          <motion.span
            className="lens-indicator"
            initial={false}
            animate={{ x: box.left, width: box.width }}
            transition={spring.soft}
            aria-hidden
          />
        )}
        {CATEGORIES.map(({ id }) => {
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
      </div>
      <span className="lens-divider" aria-hidden />
      <FilterMenu on={filtering} toggle={toggleFilter} filters={filters} />
      <button
        type="button"
        className={`lens-pill lens-go focus-ring ${hot ? "is-hot" : ""} ${empty ? "is-disabled" : ""}`}
        disabled={empty}
        onClick={open}
      >
        {hot && glowKey > 0 && !reduced && <span key={glowKey} className="lens-glow" aria-hidden />}
        <span className="relative">{label}</span>
      </button>
    </div>
  );
}

/** Large & Old and Grew stay in this menu until the filter button is opened. */
function FilterMenu({
  on,
  filters,
  toggle,
}: {
  on: boolean;
  filters: { largeOld: boolean; grew: boolean };
  toggle: (id: "largeOld" | "grew") => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const count = (filters.largeOld ? 1 : 0) + (filters.grew ? 1 : 0);
  // The closed button has to say how many filters are on. Opening the menu drops the label.
  const collapsedLabel = !open && count > 0 ? t("scope.filterCount", { count }) : null;

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    const onDown = (e: PointerEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onDown, true);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onDown, true);
    };
  }, [open]);

  return (
    <div className="lens-filter-wrap" ref={root}>
      {open && (
        <div className="lens-filter-pop" role="group" aria-label={t("scope.filters")}>
          {FILTERS.map(({ id }) => {
            const pressed = filters[id];
            return (
              <button
                key={id}
                type="button"
                aria-pressed={pressed}
                className={`lens-filter focus-ring ${pressed ? "is-on" : ""}`}
                onClick={() => toggle(id)}
              >
                {t(`lens.${id}`)}
              </button>
            );
          })}
        </div>
      )}
      <button
        type="button"
        className={`lens-filter-btn focus-ring ${on ? "is-on" : ""} ${open ? "is-open" : ""} ${collapsedLabel ? "has-count" : ""}`}
        aria-expanded={open}
        aria-label={collapsedLabel ? undefined : count > 0 ? t("scope.filterCount", { count }) : t("scope.filters")}
        onClick={() => setOpen((v) => !v)}
      >
        <Icon name="filter" size={16} />
        {collapsedLabel}
      </button>
    </div>
  );
}

