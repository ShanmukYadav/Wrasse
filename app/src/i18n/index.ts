// Every user-facing string goes through t() (PLAN.md §7.7). English only for v1.
import { en, type MessageKey } from "./en";

const messages: Record<string, string> = en;

// Accepts computed keys like `lens.${id}`; unknown keys render as the key itself.
export function t(key: MessageKey | (string & {}), vars?: Record<string, string | number>): string {
  let text = messages[key] ?? key;
  if (text.includes("|") && vars && typeof vars.count === "number") {
    const [one, other] = text.split("|");
    text = vars.count === 1 ? one : other;
  }
  if (vars) {
    for (const [name, value] of Object.entries(vars)) {
      text = text.replaceAll(`{${name}}`, typeof value === "number" ? value.toLocaleString() : value);
    }
  }
  return text;
}

export type { MessageKey };
