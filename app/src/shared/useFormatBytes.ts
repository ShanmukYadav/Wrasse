import { bytesText } from "../lib/format";
import { useApp } from "../store/app";

/** Formats bytes in this Mac's units ("19.8 GB"). */
export function useFormatBytes(): (bytes: number) => string {
  const units = useApp((s) => s.info?.units ?? "decimal");
  return (b) => bytesText(b, units);
}
