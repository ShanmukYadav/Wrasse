import { Icon } from "../design/icons/Icon";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { suggest } from "../command/commands";
import { t } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import { useApp, type Plan } from "../store/app";
import { Bytes } from "../components/Bytes";
import { iconFor } from "../design/icons/semantic";
import { formatBytes } from "../lib/format";
import { planReason } from "./planReason";

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
  const { requestDelete, togglePlanItem, setPlanAll } = useApp.getState();
  const clearPlan = useApp((s) => s.clearPlan);
  const locate = useApp((s) => s.locate);
  const disk = useApp((s) => s.disk);
  const units = useApp((s) => s.info?.units ?? "decimal");
  // Every candidate starts ticked; the meter and the button follow what's still ticked.
  const picked = plan.ids.filter((id) => !plan.off.includes(id));
  const pickedBytes = picked.reduce((s, id) => s + (disk.nodes[id]?.size ?? 0), 0);
  const fraction = Math.min(1, pickedBytes / plan.target);
  // Short because nothing more is available, or because the user unticked some.
  const short = plan.total < plan.target ? "plan.short" : pickedBytes < plan.target ? "plan.shortPicked" : null;
  const allOn = picked.length === plan.ids.length;
  const pf = formatBytes(pickedBytes, units);
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
      <div className="meter-track">
        <motion.div className="meter-fill" initial={{ scaleX: 0 }} animate={{ scaleX: fraction }} transition={spring.gentle} />
      </div>
      {short && <p className="text-caption text-your-call">{t(short)}</p>}
      {/* Every candidate, ranked, with why it was picked. Clicking one shows it on the map. */}
      <ol className="plan-list">
        {plan.ids.map((id, i) => {
          const n = disk.nodes[id];
          const f = formatBytes(n.size, units);
          const on = !plan.off.includes(id);
          return (
            <li key={id} className={`plan-row ${on ? "" : "is-off"}`}>
              <button type="button" role="checkbox" aria-checked={on} aria-label={t("select.tick", { name: n.name })} className={`plan-tick focus-ring ${on ? "is-on" : ""}`} onClick={() => togglePlanItem(id)}>
                {on && <Icon name="check" tone="white" size={16} />}
              </button>
              <button type="button" className="plan-item focus-ring" onClick={() => locate(id)}>
                <span className="plan-rank tabular">{i + 1}</span>
                <Icon name={iconFor(n)} tone={n.category} size={20} />
                <span className="plan-text">
                  <span className="plan-name">
                    {n.name} <span className="plan-where">{t("plan.in", { parent: disk.nodes[n.parent].name })}</span>
                  </span>
                  <span className="plan-why">{planReason(disk, n)}</span>
                </span>
                <span className="plan-size tabular">
                  {f.value} {f.unit}
                </span>
              </button>
            </li>
          );
        })}
      </ol>
      <div className="flex items-center gap-2">
        <button type="button" className="btn btn-primary focus-ring" onClick={() => requestDelete(picked)} disabled={!picked.length}>
          <Icon name="trash" size={16} /> {t("plan.deletePicked", { count: picked.length, size: `${pf.value} ${pf.unit}` })}
        </button>
        <button type="button" className="link focus-ring" onClick={() => setPlanAll(!allOn)}>
          {allOn ? t("plan.selectNone") : t("plan.selectAll")}
        </button>
        <span className="flex-1" />
        <button type="button" className="btn focus-ring" onClick={clearPlan}>
          {t("plan.clear")}
        </button>
      </div>
    </motion.div>
  );
}

