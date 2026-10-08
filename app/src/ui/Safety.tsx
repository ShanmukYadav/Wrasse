import { LockSimple, ShieldCheck, WarningCircle } from "@phosphor-icons/react";
import type { Safety } from "../design/tokens";
import { t } from "../i18n";

const ICONS = { safe: ShieldCheck, yourCall: WarningCircle, keep: LockSimple };

// Always icon + text, never color alone.
export function SafetyLabel({ safety }: { safety: Safety }) {
  const Icon = ICONS[safety];
  return (
    <span className={`safety safety-${safety}`}>
      <Icon size={14} weight="fill" aria-hidden />
      {t(`safety.${safety}`)}
    </span>
  );
}
