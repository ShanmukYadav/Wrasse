import { MagnifyingGlass } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { suggest } from "../command/commands";
import { useGlassRegion } from "../glass/regions";
import { t } from "../i18n";
import { duration, ease, spring } from "../motion/tokens";
import { useApp, type Plan } from "../store/app";
import { Bytes } from "../components/Bytes";

export function CommandBar({ glassClass }: { glassClass: string }) {
  const barRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  useGlassRegion("command", barRef, "top");
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
      <div ref={barRef} className={`command-bar ${glassClass}`}>
        <MagnifyingGlass size={17} weight="bold" className="text-muted shrink-0" aria-hidden />
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
            className="command-results glass-css"
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
                <span className="relative">{item.label}</span>
                {item.hint && <span className="relative command-hint">{item.hint}</span>}
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
  const addToTray = useApp((s) => s.addToTray);
  const clearPlan = useApp((s) => s.clearPlan);
  const fraction = Math.min(1, plan.total / plan.target);
  const short = plan.total < plan.target;
  return (
    <motion.div
      className="plan-meter glass-css"
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
          <Bytes value={plan.total} /> · {t("plan.candidates", { count: plan.ids.length })}
        </span>
      </div>
      <div className="meter-track">
        <motion.div className="meter-fill" initial={{ scaleX: 0 }} animate={{ scaleX: fraction }} transition={spring.gentle} />
      </div>
      {short && <p className="text-caption text-your-call">{t("plan.short")}</p>}
      <div className="flex gap-2">
        <button type="button" className="btn btn-primary focus-ring" onClick={() => addToTray(plan.ids)} disabled={!plan.ids.length}>
          {t("plan.addAll", { count: plan.ids.length })}
        </button>
        <button type="button" className="btn focus-ring" onClick={clearPlan}>
          {t("plan.clear")}
        </button>
      </div>
    </motion.div>
  );
}
