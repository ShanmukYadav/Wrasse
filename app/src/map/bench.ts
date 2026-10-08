// Scripted fps benchmark: sonar scan, lens switching and camera zoom at 2,000 and 20,000 blocks.
// Results go to the terminal running `tauri dev` (and the console in a browser).
import { t } from "../i18n";
import { devLog } from "../lib/backend";
import { useApp } from "../store/app";
import type { LensId } from "../design/tokens";
import { renderer } from "./controller";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Result = { scenario: string; blocks: number; frames: number; avgFps: number; p95Ms: number; worstMs: number; drawMs: number; rasterMs: number };

function summarize(scenario: string): Result {
  const r = renderer()!;
  const { dt, draw, raster } = r.takeFrames();
  const sorted = [...dt].sort((a, b) => a - b);
  const mean = dt.reduce((s, v) => s + v, 0) / Math.max(1, dt.length);
  return {
    scenario,
    blocks: r.view?.blocks.length ?? 0,
    frames: dt.length,
    avgFps: dt.length ? 1000 / mean : 0,
    p95Ms: sorted[Math.floor(sorted.length * 0.95)] ?? 0,
    worstMs: sorted[sorted.length - 1] ?? 0,
    drawMs: draw.reduce((s, v) => s + v, 0) / Math.max(1, draw.length),
    rasterMs: raster.length ? Math.max(...raster) : 0,
  };
}

let running = false;

export async function runBenchmark(): Promise<Result[]> {
  if (running) return [];
  running = true;
  const app = useApp.getState;
  const results: Result[] = [];
  app().notify(t("bench.running"));
  for (const count of [2000, 20000] as const) {
    app().setBlockCount(count);
    app().setLens("all");
    await wait(600);
    const r = renderer()!;
    r.takeFrames();

    app().startScan();
    await wait(2800);
    results.push(summarize("sonar scan"));

    for (const lens of ["junk", "apps", "aiModels", "dev", "largeOld", "grew", "all"] as LensId[]) {
      app().setLens(lens);
      await wait(380);
    }
    results.push(summarize("lens switch"));

    const v = r.view!;
    const groups = [...v.groups].sort((a, b) => b.w * b.h - a.w * a.h).slice(0, 3);
    for (const g of groups) {
      app().setFocus(g.id);
      await wait(650);
      app().setFocus(app().disk.root);
      await wait(650);
    }
    results.push(summarize("zoom in/out"));
  }
  app().setBlockCount(2000);
  for (const res of results) {
    await devLog(
      `[bench] ${res.blocks} blocks · ${res.scenario}: ${res.avgFps.toFixed(1)} fps avg, p95 ${res.p95Ms.toFixed(1)} ms, worst ${res.worstMs.toFixed(1)} ms, draw ${res.drawMs.toFixed(2)} ms/frame, slowest new-view raster ${res.rasterMs.toFixed(0)} ms, ${res.frames} frames`,
    );
  }
  (window as unknown as { __wrasseBench: Result[] }).__wrasseBench = results;
  app().notify(t("bench.done"));
  running = false;
  return results;
}
