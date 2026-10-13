// Mirror of wrasse-core::format (same rules and test vectors) so animated numbers can be
// formatted every frame without a backend round trip. Keep the two in sync.
export type UnitSystem = "decimal" | "binary";

const UNITS = ["bytes", "KB", "MB", "GB", "TB", "PB"];

export function formatBytes(bytes: number, system: UnitSystem): { value: string; unit: string } {
  const base = system === "binary" ? 1024 : 1000;
  if (bytes < base) {
    const n = Math.max(0, Math.round(bytes));
    return { value: String(n), unit: n === 1 ? "byte" : "bytes" };
  }
  let value = bytes;
  let unit = 0;
  while (value >= base && unit < UNITS.length - 1) {
    value /= base;
    unit++;
  }
  if (value >= 99.95 && value.toFixed(0).length > 3 && unit < UNITS.length - 1) {
    value /= base;
    unit++;
  }
  // One decimal below 100 ("19.8 GB", "6.1 GB"), dropped when it's .0 ("39 GB").
  const text = value < 100 ? value.toFixed(1).replace(/\.0$/, "") : value.toFixed(0);
  return { value: text, unit: UNITS[unit] };
}

/** "19.8 GB": the value and unit as one string. */
export function bytesText(bytes: number, system: UnitSystem): string {
  const f = formatBytes(bytes, system);
  return `${f.value} ${f.unit}`;
}
