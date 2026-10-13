import { Icon } from "../design/icons/Icon";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { suggest } from "../command/commands";
import { t } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import { useApp, type Plan } from "../store/app";
import { Bytes } from "../shared/Bytes";
import { iconFor } from "../design/icons/semantic";
import { bytesText } from "../lib/format";
import { planReason } from "../features/cleanup/planReason";

export function CommandBar() {
  const inputRef = useRef<HTMLInputElement>(null);
  const open = useApp((s) => s.commandOpen);
  const setOpen = useApp((s) => s.setCommandOpen);
  const plan = useApp((s) => s.plan);
  const [text, setText] = useState("");
  const [active, setActive] = useState(0);
  const items = useMemo(() => (open ? suggest(text) : []), [open, text]);

  // ⌘K opens; typing anywhere opens with that character.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.defaultPrevented) return;
      const target = e.target as HTMLElement;
      const typing = target.tagName === "INPUT" || target.tagName === "TEXTAREA";
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen(true);
        inputRef.current?.focus();
        inputRef.current?.select();
        return;
      }
      if (typing || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key.length === 1 && /\S/.test(e.key) && e.key !== "-") {
        e.preventDefault();
        setText(e.key);
        setActive(0);
        setOpen(true);
        inputRef.current?.focus();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setOpen]);

  function close() {
    setOpen(false);
    setText("");
    inputRef.current?.blur();
  }

  function run(i: number) {
    const item = items[i];
    if (!item) return;
    close();
    item.run();
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(items.length - 1, a + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(0, a - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(active);
    }
  }

  return (
    <div className="command-wrap">
      <div className="command-bar">
        <Icon name="search" size={16} className="command-icon" />
        <input
          ref={inputRef}
          className="command-input"
          value={text}
          placeholder={t("cmd.placeholder")}
          aria-label={t("cmd.label")}
          role="combobox"
          aria-expanded={open}
          aria-controls="command-results"
          aria-activedescendant={open && items[active] ? `cmd-${items[active].key}` : undefined}
          onFocus={() => setOpen(true)}
          onBlur={() => setTimeout(() => setOpen(false), 120)}
          onChange={(e) => {
            setText(e.target.value);
            setActive(0);
            setOpen(true);
          }}
          onKeyDown={onKeyDown}
        />
        <kbd className="kbd">⌘K</kbd>
      </div>

      <AnimatePresence>
        {open && items.length > 0 && (
          <motion.ul
            id="command-results"
            role="listbox"
            className="command-results"
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0, transition: { duration: duration.fast, ease: ease.out } }}
            exit={{ opacity: 0, y: -6, transition: { duration: duration.fast, ease: ease.inOut } }}
          >
            {items.map((item, i) => (
              <li
                key={item.key}
                id={`cmd-${item.key}`}
                role="option"
                aria-selected={i === active}
                className={`command-item ${i === active ? "is-active" : ""}`}
                onMouseEnter={() => setActive(i)}
                onMouseDown={(e) => {
                  e.preventDefault();
                  run(i);
                }}
              >
                {i === active && <motion.span layoutId="command-active" className="command-active" transition={spring.snappy} aria-hidden />}
                <span className="relative flex items-center gap-3">
                  <Icon name={item.icon} tone={item.tone} size={20} />
                  <span className="flex min-w-0 flex-col">
                    <span className="command-label">{item.label}</span>
                    {item.hint && <span className="command-hint">{item.hint}</span>}
                  </span>
                </span>
              </li>
            ))}
          </motion.ul>
        )}
      </AnimatePresence>

      <AnimatePresence>{plan && !open && <PlanMeter plan={plan} />}</AnimatePresence>
    </div>
  );
}

// Takes the plan as a prop so it keeps rendering while it animates out after the plan ends.
function PlanMeter({ plan }: { plan: Plan }) {
  const { requestDelete, togglePlanItem, setPlanAll, openUninstall, requestEmptyTray } = useApp.getState();
  const clearPlan = useApp((s) => s.clearPlan);
  const locate = useApp((s) => s.locate);
  const disk = useApp((s) => s.disk);
  const tray = useApp((s) => s.tray);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const fmt = (b: number) => bytesText(b, units);
  // Apps that Homebrew or Finder remove open their uninstall sheet instead of going in bulk.
  // A tool's own cleanup (brew, Docker, simulators) confirms on its own and never goes to the tray.
  const routeOf = (id: number) => disk.nodes[id]?.app?.route.kind ?? "tray";
  const bulk = (id: number) => routeOf(id) === "tray" && !disk.nodes[id]?.delegate;
  const cleanupLabel = (id: number) => {
    const action = disk.nodes[id]?.delegateAction;
    if (action === "thinSnapshots") return t("home.ghostFree");
    if (action === "emptyTrash") return t("detail.emptyTrash");
    return t("detail.runCleanup");
  };
  const picked = plan.ids.filter((id) => !plan.off.includes(id) && bulk(id));
  const pickedBytes = picked.reduce((s, id) => s + (plan.bytes[id] ?? 0), 0);
  // Progress: freed for good, pending in the tray (since the plan started), and still to do.
  const pending = tray.filter((i) => (i.at ?? 0) >= plan.startedAt).reduce((s, i) => s + i.size, 0);
  const done = plan.freed + pending + plan.inTrash;
  const pct = (b: number) => `${Math.min(100, (b / plan.target) * 100)}%`;
  const short = plan.shortfall > 0 ? "plan.shortBy" : done + pickedBytes < plan.target ? "plan.shortPicked" : null;
  const allOn = picked.length === plan.ids.filter(bulk).length;
  return (
    <motion.div
      className="plan-meter"
      role="status"
      initial={{ opacity: 0, y: -6 }}
      animate={{ opacity: 1, y: 0, transition: { duration: duration.base, ease: ease.out } }}
      exit={{ opacity: 0, y: -6, transition: { duration: duration.fast, ease: ease.inOut } }}
    >
      <div className="flex items-baseline justify-between gap-4">
        <span className="text-label">
          {t("plan.title")} <Bytes value={plan.target} className="text-strong" />
        </span>
        <span className="text-caption text-muted tabular">
          <Bytes value={pickedBytes} /> · {t("plan.picked", { picked: picked.length, count: plan.ids.length })}
        </span>
      </div>
      <div className="meter-track plan-track" aria-hidden>
        <span className="plan-seg is-freed" style={{ width: pct(plan.freed) }} />
        <span className="plan-seg is-pending" style={{ width: pct(pending + plan.inTrash) }} />
        <span className="plan-seg is-picked" style={{ width: pct(pickedBytes) }} />
      </div>
      <div className="plan-legend text-caption tabular">
        <span>
          <i className="plan-dot is-freed" /> {t("plan.freedNow", { size: fmt(plan.freed) })}
        </span>
        <span>
          <i className="plan-dot is-pending" /> {t("plan.pendingTray", { size: fmt(pending) })}
        </span>
        {plan.inTrash > 0 && <span>{t("plan.inTrash", { size: fmt(plan.inTrash) })}</span>}
        {pending > 0 && plan.freed < plan.target && (
          <button type="button" className="link focus-ring" onClick={requestEmptyTray}>
            {t("plan.emptyTrayNow")} · {t("detail.notUndoable")}
          </button>
        )}
      </div>
      {short && (
        <p className="text-caption text-your-call">
          {short === "plan.shortBy" ? t("plan.shortBy", { size: fmt(plan.shortfall) }) : t(short)}
          {short === "plan.shortBy" && plan.heldRecent > 0 && ` ${t("plan.heldRecent", { size: fmt(plan.heldRecent) })}`}
          {short === "plan.shortBy" && plan.heldRunning > 0 && ` ${t("plan.heldRunning", { size: fmt(plan.heldRunning) })}`}
        </p>
      )}
      {/* Every candidate, ranked, with why it was picked. Clicking one shows it on the map. */}
      <ol className="plan-list">
        {plan.ids.map((id, i) => {
          const n = disk.nodes[id];
          const on = !plan.off.includes(id);
          const viaSheet = !bulk(id);
          return (
            <li key={id} className={`plan-row ${on && !viaSheet ? "is-on" : "is-off"}`}>
              {viaSheet ? (
                <span className="plan-tick is-sheet" aria-hidden />
              ) : (
                <button type="button" role="checkbox" aria-checked={on} aria-label={t("select.tick", { name: n.name })} className={`plan-tick focus-ring ${on ? "is-on" : ""}`} onClick={() => togglePlanItem(id)}>
                  {on && <Icon name="check" size={16} />}
                </button>
              )}
              <button type="button" className="plan-item focus-ring" onClick={() => locate(id)}>
                <span className="plan-rank tabular">{i + 1}</span>
                <Icon name={iconFor(n)} tone={n.category} size={20} />
                <span className="plan-text">
                  <span className="plan-name">
                    {n.name} <span className="plan-where">{t("plan.in", { parent: disk.nodes[n.parent].name })}</span>
                  </span>
                  <span className="plan-why">{planReason(disk, n, plan.why[id])}</span>
                </span>
                <span className="plan-size tabular">{fmt(plan.bytes[id] ?? n.size)}</span>
              </button>
              {viaSheet && disk.nodes[id]?.delegate && (
                <button type="button" className="link focus-ring plan-uninstall" onClick={() => requestDelete([id])}>
                  {cleanupLabel(id)}
                </button>
              )}
              {viaSheet && !disk.nodes[id]?.delegate && (
                <button type="button" className="link focus-ring plan-uninstall" onClick={() => openUninstall(id)}>
                  {t("plan.uninstall")}
                </button>
              )}
            </li>
          );
        })}
      </ol>
      <div className="flex items-center gap-2">
        {plan.ids.length > 0 ? (
          <>
            <button type="button" className="btn btn-primary focus-ring" onClick={() => requestDelete(picked.flatMap((id) => plan.parts[id] ?? [id]), false)} disabled={!picked.length}>
              <Icon name="tray" size={16} /> {t("plan.deletePicked", { count: picked.length, size: fmt(pickedBytes) })}
            </button>
            <button type="button" className="link focus-ring" onClick={() => setPlanAll(!allOn)}>
              {allOn ? t("plan.selectNone") : t("plan.selectAll")}
            </button>
          </>
        ) : (
          <span className="text-caption text-muted">{t("plan.allHandled")}</span>
        )}
        <span className="flex-1" />
        <button type="button" className="btn focus-ring" onClick={clearPlan}>
          {t("plan.clear")}
        </button>
      </div>
    </motion.div>
  );
}

