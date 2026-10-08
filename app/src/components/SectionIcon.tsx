import { hueTile } from "../design/tokens";
import type { ScreenDef } from "../screens/registry";

// Duotone section icon on a tile of the section's hue.
export function SectionIcon({ def, size }: { def: ScreenDef; size: "sm" | "lg" }) {
  const Icon = def.icon;
  return (
    <span className={`section-icon section-icon-${size}`} style={{ background: hueTile(def.hue) }} aria-hidden>
      <Icon weight="duotone" size={size === "sm" ? 17 : 38} />
    </span>
  );
}
