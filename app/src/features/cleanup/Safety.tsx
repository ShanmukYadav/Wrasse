import { Icon } from "../../design/icons/Icon";
import type { Safety } from "../../design/tokens";
import { t } from "../../i18n";

// Always icon + text, never color alone.
export function SafetyLabel({ safety }: { safety: Safety }) {
  return (
    <span className={`safety safety-${safety}`}>
      <Icon name={safety} tone={safety} size={16} />
      {t(`safety.${safety}`)}
    </span>
  );
}
