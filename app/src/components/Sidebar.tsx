import { motion } from "motion/react";
import { useRef, type KeyboardEvent } from "react";
import { t } from "../i18n";
import { spring } from "../motion/tokens";
import { allScreens, screenGroups, settingsScreen, type ScreenDef } from "../screens/registry";
import { useApp } from "../store/app";

export function Sidebar() {
  const navRef = useRef<HTMLElement>(null);
  const screen = useApp((s) => s.screen);
  const setScreen = useApp((s) => s.setScreen);

  // Arrow keys move between sections, like a native source list.
  function onKeyDown(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const index = allScreens.findIndex((s) => s.id === screen);
    const next = (index + (e.key === "ArrowDown" ? 1 : -1) + allScreens.length) % allScreens.length;
    const id = allScreens[next].id;
    setScreen(id);
    navRef.current?.querySelector<HTMLButtonElement>(`[data-screen="${id}"]`)?.focus();
  }

  return (
    <nav
      ref={navRef}
      aria-label={t("nav.label")}
      onKeyDown={onKeyDown}
      className="sidebar flex h-full w-[232px] shrink-0 flex-col px-3 pb-3"
    >
      <div data-tauri-drag-region className="titlebar-space shrink-0" />
      <div className="flex flex-1 flex-col gap-4 overflow-y-auto">
        {screenGroups.map((group, i) => (
          <ul key={i} className="flex flex-col gap-0.5">
            {group.map((def) => (
              <SidebarItem key={def.id} def={def} active={def.id === screen} onSelect={setScreen} />
            ))}
          </ul>
        ))}
      </div>
      <ul className="flex flex-col gap-0.5 pt-2">
        <SidebarItem
          def={settingsScreen}
          active={settingsScreen.id === screen}
          onSelect={setScreen}
        />
      </ul>
    </nav>
  );
}

function SidebarItem({
  def,
  active,
  onSelect,
}: {
  def: ScreenDef;
  active: boolean;
  onSelect: (id: ScreenDef["id"]) => void;
}) {
  const Icon = def.icon;
  return (
    <li>
      <button
        type="button"
        data-screen={def.id}
        aria-current={active ? "page" : undefined}
        tabIndex={active ? 0 : -1}
        onClick={() => onSelect(def.id)}
        className={`focus-ring relative flex h-8 w-full items-center gap-2.5 rounded-[var(--radius-sm)] px-2.5 text-left text-[13px] transition-colors duration-150 ${
          active ? "text-[var(--text)]" : "text-[var(--text-muted)] hover:text-[var(--text)]"
        }`}
      >
        {active && (
          <motion.span
            layoutId="sidebar-pill"
            transition={spring.soft}
            className="absolute inset-0 rounded-[var(--radius-sm)] bg-[var(--pill)]"
            aria-hidden
          />
        )}
        <Icon
          size={16}
          strokeWidth={1.9}
          className={`relative ${active ? "text-[var(--accent)]" : ""}`}
          aria-hidden
        />
        <span className="relative truncate">{t(def.title)}</span>
      </button>
    </li>
  );
}
