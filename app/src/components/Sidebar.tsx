import { motion } from "motion/react";
import { useRef, type KeyboardEvent } from "react";
import { t } from "../i18n";
import { spring } from "../motion/tokens";
import { allScreens, screenGroups, settingsScreen, type ScreenDef } from "../screens/registry";
import { useApp } from "../store/app";
import { SectionIcon } from "./SectionIcon";

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
    <nav ref={navRef} aria-label={t("nav.label")} onKeyDown={onKeyDown} className="sidebar">
      <div data-tauri-drag-region className="titlebar-space" />
      <div className="flex flex-1 flex-col gap-5 overflow-y-auto">
        {screenGroups.map((group, i) => (
          <ul key={i} className="flex flex-col gap-1">
            {group.map((def) => (
              <SidebarItem key={def.id} def={def} active={def.id === screen} onSelect={setScreen} />
            ))}
          </ul>
        ))}
      </div>
      <ul className="flex flex-col gap-1 pt-2">
        <SidebarItem def={settingsScreen} active={settingsScreen.id === screen} onSelect={setScreen} />
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
  return (
    <li>
      <button
        type="button"
        data-screen={def.id}
        aria-current={active ? "page" : undefined}
        tabIndex={active ? 0 : -1}
        onClick={() => onSelect(def.id)}
        className={`sidebar-item focus-ring ${active ? "is-active" : ""}`}
      >
        {active && (
          <motion.span layoutId="sidebar-pill" transition={spring.soft} className="sidebar-pill" aria-hidden />
        )}
        <SectionIcon def={def} size="sm" />
        <span className="relative truncate">{t(def.title)}</span>
      </button>
    </li>
  );
}
