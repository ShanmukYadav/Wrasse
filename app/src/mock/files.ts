// Mock Large & Old and Duplicates data for the browser preview.
import type { BigFile, DuplicateGroup, PathRemoval } from "../api/types";

const GB = 1e9;
const FILES: BigFile[] = [
  { path: "~/Movies/Iceland Trip 2023.mov", bytes: 14.2 * GB, lastUsedDays: 610, approx: false, kind: "video" },
  { path: "~/Downloads/Xcode_16.1.xip", bytes: 11.9 * GB, lastUsedDays: 420, approx: true, kind: "archive" },
  { path: "~/Movies/Wedding Highlights.mov", bytes: 11.6 * GB, lastUsedDays: 40, approx: false, kind: "video" },
  { path: "~/Downloads/Windows11_ARM.iso", bytes: 6.5 * GB, lastUsedDays: 390, approx: true, kind: "diskImage" },
  { path: "~/Documents/Projects/render-final.mov", bytes: 4.1 * GB, lastUsedDays: 700, approx: false, kind: "video" },
  { path: "~/models/llama-3.1-8b.Q8_0.gguf", bytes: 8.5 * GB, lastUsedDays: 12, approx: false, kind: "model" },
  { path: "~/Music/Session stems.zip", bytes: 2.2 * GB, lastUsedDays: 900, approx: true, kind: "archive" },
  { path: "~/Pictures/Exports/panorama.tif", bytes: 0.9 * GB, lastUsedDays: 500, approx: false, kind: "image" },
  { path: "~/Downloads/ubuntu-24.04.iso", bytes: 0.6 * GB, lastUsedDays: 200, approx: true, kind: "diskImage" },
];

export function mockLarge(minBytes: number): BigFile[] {
  return FILES.filter((f) => f.bytes >= minBytes).sort((a, b) => b.bytes - a.bytes);
}

export function mockDuplicates(): DuplicateGroup[] {
  return [
    {
      hash: "a1",
      bytes: 3.1 * GB,
      reclaimable: 3.1 * GB,
      copies: [
        { path: "~/Movies/Beach Day.mov", modifiedDays: 400, handPickOnly: false, clone: false },
        { path: "~/Movies/Beach Day copy.mov", modifiedDays: 380, handPickOnly: false, clone: false },
      ],
    },
    {
      hash: "b2",
      bytes: 1.4 * GB,
      reclaimable: 2.8 * GB,
      copies: [
        { path: "~/Documents/Photos export.zip", modifiedDays: 300, handPickOnly: true, clone: false },
        { path: "~/Downloads/Photos export.zip", modifiedDays: 300, handPickOnly: false, clone: false },
        { path: "~/Desktop/Old project.zip", modifiedDays: 250, handPickOnly: true, clone: false },
      ],
    },
    {
      hash: "c3",
      bytes: 0.8 * GB,
      reclaimable: 0,
      copies: [
        { path: "~/Downloads/dataset.parquet", modifiedDays: 90, handPickOnly: false, clone: true },
        { path: "~/Downloads/dataset (1).parquet", modifiedDays: 90, handPickOnly: false, clone: true },
      ],
    },
  ];
}

/** Sizes for mock removals, from the mock lists above. */
export function mockRemoval(paths: string[]): PathRemoval {
  const size = (p: string) => FILES.find((f) => f.path === p)?.bytes ?? mockDuplicates().find((g) => g.copies.some((c) => c.path === p))?.bytes ?? 0;
  const done = paths.map((path) => ({ path, bytes: size(path), entry: `mock/${path}` }));
  return { done, skipped: [], bytes: done.reduce((s, d) => s + d.bytes, 0), dryRun: false };
}
