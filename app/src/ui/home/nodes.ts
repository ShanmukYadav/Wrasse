import type { DiskNode } from "../../api";

/** Folders and apps you can step into; models and files are single items (select, don't drill). */
export function isDrillable(n: DiskNode): boolean {
  return (n.kind === "dir" || n.kind === "volume" || n.kind === "app") && n.children.length > 0;
}
