import { useVirtualizer } from "@tanstack/react-virtual";
import { AnimatePresence, motion } from "motion/react";
import { useMemo, useRef, useState } from "react";
import { lensColors } from "../design/tokens";
import { t } from "../i18n";
import { formatBytes } from "../lib/format";
import { useScheme } from "../lib/prefs";
import { duration, ease } from "../motion/tokens";
import { useApp } from "../store/app";
import { SafetyLabel } from "./Safety";
import { Icon } from "../design/icons/Icon";
import { iconFor, toneFor } from "../design/icons/semantic";
import { ScopeEmpty } from "./ScopeEmpty";
import { scopeBits, visibleRows } from "./scope";

type SortKey = "name" | "category" | "size" | "lastOpened" | "safety";

export function ListView() {
  const visible = useApp((s) => s.viewMode === "list");
  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          className="list-layer"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
          exit={{ opacity: 0, transition: { duration: duration.base, ease: ease.inOut } }}
        >
          <List />
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function List() {
  const disk = useApp((s) => s.disk);
  const version = useApp((s) => s.diskVersion);
  const focus = useApp((s) => s.focus);
  const lens = useApp((s) => s.lens);
  const filters = useApp((s) => s.filters);
  const plan = useApp((s) => s.plan);
  const selected = useApp((s) => s.selected);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const { select, addToTray } = useApp.getState();
  const scheme = useScheme();
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({ key: "size", desc: true });
  const scrollRef = useRef<HTMLDivElement>(null);

  const bits = scopeBits(lens, filters);
  const rows = useMemo(() => {
    // The same rows buttonCount uses, once a category or filter is on. A plan keeps every row.
    const ids = plan || bits === 0
      ? disk.nodes[focus].children.filter((id) => disk.nodes[id].size > 0 && disk.nodes[id].kind !== "ghost")
      : visibleRows(disk, focus, lens, filters);
    const val = (id: number) => {
      const n = disk.nodes[id];
      switch (sort.key) {
        case "name": return n.name.toLowerCase();
        case "category": return n.category;
        case "size": return n.size;
        case "lastOpened": return n.lastOpenedDays;
        case "safety": return n.safety;
      }
    };
    return ids.sort((a, b) => {
      const va = val(a);
      const vb = val(b);
      const c = va < vb ? -1 : va > vb ? 1 : 0;
      return sort.desc ? -c : c;
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [disk, focus, sort, version, plan, bits, lens, filters]);

  const planRank = useMemo(() => new Map((plan?.ids ?? []).map((id, i) => [id, i + 1])), [plan]);
  const virtualizer = useVirtualizer({ count: rows.length, getScrollElement: () => scrollRef.current, estimateSize: () => 40, overscan: 12 });

  function onKeyDown(e: React.KeyboardEvent) {
    const i = selected !== null ? rows.indexOf(selected) : -1;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = Math.max(0, Math.min(rows.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
      select(rows[next], false);
      virtualizer.scrollToIndex(next);
    } else if (e.key === "Enter" && selected !== null) {
      e.preventDefault();
      select(selected, true);
    } else if (e.key === " " && selected !== null) {
      e.preventDefault();
      addToTray([selected]);
    }
  }

  const columns: [SortKey, string][] = [
    ["name", t("list.name")],
    ["category", t("list.category")],
    ["size", t("list.size")],
    ["lastOpened", t("list.lastOpened")],
    ["safety", t("list.safety")],
  ];

  return (
    <div className="list-panel glass-css">
      <div className="list-header" role="row">
        {columns.map(([key, label]) => (
          <button
            key={key}
            type="button"
            role="columnheader"
            aria-sort={sort.key === key ? (sort.desc ? "descending" : "ascending") : "none"}
            className={`list-col list-col-${key} focus-ring`}
            onClick={() => setSort({ key, desc: sort.key === key ? !sort.desc : key === "size" || key === "lastOpened" })}
          >
            {label} {sort.key === key ? (sort.desc ? "↓" : "↑") : ""}
          </button>
        ))}
      </div>
      <div
        ref={scrollRef}
        className="list-scroll focus-ring"
        tabIndex={0}
        role="listbox"
        aria-label={t("list.label", { count: rows.length })}
        aria-activedescendant={selected !== null ? `row-${selected}` : undefined}
        onKeyDown={onKeyDown}
      >
        {!plan && bits !== 0 && rows.length === 0 ? (
          <ScopeEmpty />
        ) : (
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((v) => {
            const id = rows[v.index];
            const n = disk.nodes[id];
            const matches = plan ? planRank.has(id) : true;
            const f = formatBytes(n.size, units);
            const dotLens = plan ? null : filters.grew ? "grew" : filters.largeOld ? "largeOld" : lens !== "all" ? lens : null;
            const dot = dotLens ? lensColors[scheme][dotLens] : undefined;
            return (
              <div
                key={id}
                id={`row-${id}`}
                role="option"
                aria-selected={selected === id}
                className={`list-row ${selected === id ? "is-selected" : ""}`}
                style={{ transform: `translateY(${v.start}px)`, opacity: plan ? (matches ? 1 : 0.25) : 1 }}
                onClick={() => select(id, false)}
                onDoubleClick={() => select(id, true)}
              >
                <span className="list-col list-col-name">
                  {dot && <span className="lens-dot" style={{ background: dot }} aria-hidden />}
                  {planRank.has(id) && <span className="rank">{planRank.get(id)}</span>}
                  <Icon name={iconFor(n)} tone={toneFor(n)} size={20} />
                  <span className="truncate list-name">{n.name}</span>
                </span>
                <span className="list-col list-col-category text-muted">{t(`category.${n.category}`)}</span>
                <span className="list-col list-col-size tabular">{f.value} {f.unit}</span>
                <span className="list-col list-col-lastOpened text-muted tabular">{n.kind === "ghost" ? "—" : n.lastOpenedDays === 0 ? t("detail.today") : t("detail.daysAgo", { count: n.lastOpenedDays })}</span>
                <span className="list-col list-col-safety"><SafetyLabel safety={n.safety} /></span>
              </div>
            );
          })}
        </div>
        )}
      </div>
    </div>
  );
}
