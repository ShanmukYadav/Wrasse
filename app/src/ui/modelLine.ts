import type { DiskNode, LoadedModel, ModelTool } from "../api";
import { t } from "../i18n";

export function toolName(tool: ModelTool): string {
  return t(`model.tool.${tool}`);
}

/** Last used, with "approx" when the date comes from file times. */
export function usedLabel(n: DiskNode): string {
  const when = n.lastOpenedDays === 0 ? t("detail.today") : t("detail.daysAgo", { count: n.lastOpenedDays });
  return n.approx ? t("model.usedApprox", { when }) : when;
}

/** Tool, quantization and last used, for a model tile. */
export function modelCaption(n: DiskNode): string {
  if (!n.model) return "";
  return [toolName(n.model.tool), n.model.quantization, usedLabel(n)].filter(Boolean).join(" · ");
}

function normalizeOllama(name: string): string {
  let n = name.trim().replace(/^registry\.ollama\.ai\//i, "");
  if (n.toLowerCase().startsWith("library/")) n = n.slice("library/".length);
  return n.toLowerCase();
}

/** The loaded entry for this model, if Ollama or LM Studio is holding it. */
export function loadedFor(n: DiskNode, list: LoadedModel[]): LoadedModel | undefined {
  if (!n.model) return undefined;
  if (n.model.tool === "lmStudio") return list.find((m) => m.tool === "lmStudio");
  const name = n.model.ollamaName;
  if (!name || n.model.removal !== "ollama") return undefined;
  const want = normalizeOllama(name);
  return list.find((m) => m.tool === "ollama" && normalizeOllama(m.name) === want);
}
