// Every user-facing string goes through t() (PLAN.md §7.7). English only for v1.
import { en, type MessageKey } from "./en";

const messages: Record<MessageKey, string> = en;

export function t(key: MessageKey, vars?: Record<string, string | number>): string {
  let text = messages[key] ?? key;
  if (vars) {
    for (const [name, value] of Object.entries(vars)) {
      text = text.replaceAll(`{${name}}`, String(value));
    }
  }
  return text;
}

export type { MessageKey };
