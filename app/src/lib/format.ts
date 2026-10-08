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
  return { value: value < 10 ? value.toFixed(1) : value.toFixed(0), unit: UNITS[unit] };
}
