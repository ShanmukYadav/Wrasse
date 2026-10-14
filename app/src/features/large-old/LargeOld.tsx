import { motion } from "motion/react";
import { useEffect, useMemo, useState } from "react";
import { api, docsOrDesktop, handPickPath, type BigFile, type DuplicateGroup, type FileKind, type ScannedFile } from "../../api";
import { Icon } from "../../design/icons/Icon";
import type { IconName } from "../../design/icons/nodes";
import { t } from "../../i18n";
import { useFormatBytes } from "../../shared/useFormatBytes";
import { ExportMenu } from "../export/ExportMenu";
import { largeOld } from "../export/exports";
import { pressable } from "../../motion/variants";
import { useApp } from "../../store/app";
import { MiddleEllipsis } from "../../shared/MiddleEllipsis";

// Large & Old (docs/history/phase3-plan.md §5): the biggest files in the home folder with last use, and
// Duplicates, where the user picks the copy to keep. Nothing is ticked until the user ticks it,
// including rows whose date says "approx". Copies in Documents, Desktop or iCloud Drive are
// never suggested. Documents and Desktop open the full caution sheet; iCloud Drive keeps the
// one-line note. Each move sends the size and mtime from the scan so a changed file is skipped.

const SIZES = [100e6, 500e6, 1e9, 5e9];
const KINDS: FileKind[] = ["video", "archive", "diskImage", "model", "audio", "image", "document", "other"];
const KIND_ICON: Record<FileKind, IconName> = {
  video: "video",
  archive: "archive",
  diskImage: "diskImage",
  model: "aiModel",
  audio: "audio",
  image: "photo",
  document: "document",
  other: "file",
};
const LARGE_MIN_KEY = "wrasse.largeMin";

export function LargeOld() {
  const [tab, setTab] = useState<"large" | "dupes">("large");
  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="segmented self-start" role="radiogroup" aria-label={t("largeOld.tabs")}>
        {(["large", "dupes"] as const).map((k) => (
          <button key={k} type="button" role="radio" aria-checked={tab === k} className="segment focus-ring" onClick={() => setTab(k)}>
            {tab === k && <span className="segment-indicator" aria-hidden />}
            <span className="relative">{t(`largeOld.tab.${k}`)}</span>
          </button>
        ))}
      </div>
      {tab === "large" ? <Large /> : <Dupes />}
    </div>
  );
}

function Large() {
  const fmt = useFormatBytes();
  const moveFilesToTray = useApp((s) => s.moveFilesToTray);
  const openFileCaution = useApp((s) => s.openFileCaution);
  const [min, setMin] = useState<number>(() => {
    try {
      return Number(localStorage.getItem(LARGE_MIN_KEY)) || 500e6;
    } catch {
      return 500e6;
    }
  });
  const [files, setFiles] = useState<BigFile[] | null>(null);
  const [oldOnly, setOldOnly] = useState(false);
  const [kinds, setKinds] = useState<Set<FileKind>>(new Set());
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setFiles(null);
    try {
      localStorage.setItem(LARGE_MIN_KEY, String(min));
    } catch {
      /* per-viewer convenience only */
    }
    api
      .largeFiles(min)
      .then((f) => live && setFiles(f))
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [min]);
  const shown = useMemo(
    () => (files ?? []).filter((f) => (!oldOnly || f.lastUsedDays > 365) && (kinds.size === 0 || kinds.has(f.kind))),
    [files, oldOnly, kinds],
  );
  const pickedFiles = shown.filter((f) => picked.has(f.path));
  const pickedBytes = pickedFiles.reduce((s, f) => s + f.bytes, 0);
  // Documents and Desktop get the full sheet. iCloud Drive alone keeps the one-line note.
  const inDocsDesktop = pickedFiles.some((f) => docsOrDesktop(f.path));
  const inCloudOnly = !inDocsDesktop && pickedFiles.some((f) => handPickPath(f.path));
  const [confirming, setConfirming] = useState(false);
  const scanned: ScannedFile[] = pickedFiles.map((f) => ({ path: f.path, bytes: f.bytes, mtime: f.mtime }));
  const move = () =>
    void moveFilesToTray(() => api.removePaths(scanned)).then((moved) => {
      setFiles((fs) => (fs ?? []).filter((f) => !moved.includes(f.path)));
      setPicked(new Set());
      setConfirming(false);
    });
  const present = new Set((files ?? []).map((f) => f.kind));

  return (
    <>
      <div className="largeold-filters">
        <label className="text-caption text-muted">
          {t("largeOld.atLeast")}{" "}
          <select className="select focus-ring" value={min} onChange={(e) => setMin(Number(e.target.value))}>
            {SIZES.map((s) => (
              <option key={s} value={s}>
                {fmt(s)}
              </option>
            ))}
          </select>
        </label>
        <label className="largeold-toggle text-caption">
          <input type="checkbox" checked={oldOnly} onChange={(e) => setOldOnly(e.target.checked)} /> {t("largeOld.oldOnly")}
        </label>
        <ExportMenu label={t("export.largeTitle")} build={() => largeOld(shown)} />
        <span className="largeold-chips">
          {KINDS.filter((k) => present.has(k)).map((k) => (
            <button
              key={k}
              type="button"
              className={`chip chip-toggle focus-ring ${kinds.has(k) ? "is-on" : ""}`}
              onClick={() => {
                const next = new Set(kinds);
                if (next.has(k)) next.delete(k);
                else next.add(k);
                setKinds(next);
              }}
            >
              {t(`fileKind.${k}`)}
            </button>
          ))}
        </span>
      </div>
      {error ? (
        <p className="text-label text-muted">{error.includes("scan first") ? t("largeOld.scanFirst") : error}</p>
      ) : files === null ? (
        <p className="text-label text-muted">{t("largeOld.loading")}</p>
      ) : shown.length === 0 ? (
        <p className="text-label text-muted">{t("largeOld.none")}</p>
      ) : (
        <ul className="largeold-list">
          {shown.map((f) => {
            const on = picked.has(f.path);
            const name = f.path.split("/").pop() ?? f.path;
            return (
              <li key={f.path}>
                <label className={`largeold-row ${on ? "is-on" : ""}`}>
                  <input
                    type="checkbox"
                    className="sr-only"
                    checked={on}
                    onChange={() => {
                      const next = new Set(picked);
                      if (on) next.delete(f.path);
                      else next.add(f.path);
                      setPicked(next);
                    }}
                  />
                  <span className={`sheet-check ${on ? "is-on" : ""}`} aria-hidden>
                    {on && <Icon name="check" size={16} />}
                  </span>
                  <Icon name={KIND_ICON[f.kind]} tone="files" size={16} />
                  <span className="largeold-main">
                    <span className="largeold-name">{name}</span>
                    <MiddleEllipsis text={f.path} className="text-caption text-muted" />
                  </span>
                  <span className="text-caption text-muted tabular largeold-age">
                    {f.lastUsedDays === 0 ? t("detail.today") : t("detail.daysAgo", { count: f.lastUsedDays })}
                    {f.approx && ` · ${t("largeOld.approx")}`}
                  </span>
                  <span className="tile-row-size tabular">{fmt(f.bytes)}</span>
                </label>
              </li>
            );
          })}
        </ul>
      )}
      <div className="largeold-foot">
        <motion.button
          type="button"
          className="btn btn-primary focus-ring"
          disabled={!pickedFiles.length}
          onClick={() => {
            if (inDocsDesktop) {
              openFileCaution(scanned, move);
              return;
            }
            if (inCloudOnly && !confirming) {
              setConfirming(true);
              return;
            }
            move();
          }}
          {...pressable}
        >
          <Icon name="tray" size={16} /> {confirming ? t("largeOld.moveAnyway") : t("largeOld.move", { count: pickedFiles.length, size: fmt(pickedBytes) })}
        </motion.button>
        <span className={`text-caption ${confirming ? "text-your-call" : "text-muted"}`}>
          {confirming ? t("largeOld.personalNote") : pickedFiles.length ? t("largeOld.ticked", { count: pickedFiles.length }) : t("largeOld.note")}
        </span>
      </div>
    </>
  );
}

function Dupes() {
  const fmt = useFormatBytes();
  const moveFilesToTray = useApp((s) => s.moveFilesToTray);
  const [groups, setGroups] = useState<DuplicateGroup[] | null>(null);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<[number, number]>([0, 0]);
  const [error, setError] = useState<string | null>(null);
  const [keep, setKeep] = useState<Record<string, string>>({});
  const [confirm, setConfirm] = useState<string | null>(null);

  const search = () => {
    setRunning(true);
    setError(null);
    setGroups(null);
    api
      .findDuplicates((h, total) => setProgress([h, total]))
      .then((g) => setGroups(g))
      .catch((e) => setError(String(e)))
      .finally(() => setRunning(false));
  };
  useEffect(() => () => void (running && api.cancelDuplicates()), [running]);

  if (!groups && !running)
    return (
      <div className="flex flex-col items-start gap-3">
        <p className="text-label text-muted">{error ? (error.includes("scan first") ? t("largeOld.scanFirst") : error.includes("cancelled") ? t("dupes.cancelled") : error) : t("dupes.intro")}</p>
        <motion.button type="button" className="btn btn-primary focus-ring" onClick={search} {...pressable}>
          <Icon name="search" size={16} /> {t("dupes.find")}
        </motion.button>
      </div>
    );
  if (running)
    return (
      <div className="flex flex-col gap-3">
        <p className="text-label">{t("dupes.searching")}</p>
        <div className="meter-track">
          <div className="meter-fill" style={{ transform: `scaleX(${progress[1] ? Math.min(1, progress[0] / progress[1]) : 0})` }} />
        </div>
        <span className="text-caption text-muted tabular">
          {progress[1] ? t("dupes.progress", { done: fmt(progress[0]), total: fmt(progress[1]) }) : t("dupes.comparing")}
        </span>
        <button type="button" className="btn focus-ring self-start" onClick={() => void api.cancelDuplicates()}>
          {t("dupes.stop")}
        </button>
      </div>
    );
  const list = groups ?? [];
  const total = list.reduce((s, g) => s + g.reclaimable, 0);
  return (
    <div className="flex min-h-0 flex-col gap-3">
      <span className="text-caption text-muted">{list.length ? t("dupes.summary", { count: list.length, size: fmt(total) }) : t("dupes.none")}</span>
      <div className="dupes-list">
        {list.map((g) => {
          const kept = keep[g.hash];
          const others = g.copies.filter((c) => c.path !== kept);
          const freeing = others.filter((c) => !c.clone).length * g.bytes;
          const personal = others.some((c) => c.handPickOnly);
          const confirmingThis = confirm === g.hash;
          return (
            <section key={g.hash} className="dupe-group">
              <div className="dupe-head">
                <span className="text-label text-strong">{g.copies[0].path.split("/").pop()}</span>
                <span className="text-caption text-muted tabular">
                  {t("dupes.copies", { count: g.copies.length, size: fmt(g.bytes) })}
                </span>
              </div>
              <p className="text-caption text-muted">{kept ? t("dupes.keepChosen") : t("dupes.pickKeep")}</p>
              <ul className="dupe-copies" role="radiogroup" aria-label={t("dupes.pickKeep")}>
                {g.copies.map((c) => (
                  <li key={c.path}>
                    <label className={`dupe-copy ${kept === c.path ? "is-kept" : kept ? "is-going" : ""}`}>
                      <input type="radio" name={g.hash} className="sr-only" checked={kept === c.path} onChange={() => { setKeep({ ...keep, [g.hash]: c.path }); setConfirm(null); }} />
                      <span className={`dupe-radio ${kept === c.path ? "is-on" : ""}`} aria-hidden>
                        {kept === c.path && <Icon name="check" size={16} />}
                      </span>
                      <span className="largeold-main">
                        <MiddleEllipsis text={c.path} className="text-caption" />
                        <span className="text-caption text-muted">
                          {t("dupes.changed", { count: c.modifiedDays })}
                          {c.handPickOnly && ` · ${t("dupes.handPick")}`}
                          {c.clone && ` · ${t("dupes.clone")}`}
                        </span>
                      </span>
                      <span className="text-caption tabular">{kept === c.path ? t("dupes.keep") : kept ? t("dupes.toTray") : ""}</span>
                    </label>
                  </li>
                ))}
              </ul>
              {kept && (
                <motion.button
                  type="button"
                  className="btn btn-primary focus-ring self-start"
                  onClick={() =>
                    personal && !confirmingThis
                      ? setConfirm(g.hash)
                      : void moveFilesToTray(() => api.removeDuplicates(kept, others.map((c) => c.path))).then((moved) => {
                          setConfirm(null);
                          if (moved.length) setGroups((gs) => (gs ?? []).filter((x) => x.hash !== g.hash));
                        })
                  }
                  {...pressable}
                >
                  <Icon name="tray" size={16} /> {confirmingThis ? t("largeOld.moveAnyway") : t("dupes.moveOthers", { count: others.length, size: fmt(freeing) })}
                </motion.button>
              )}
              {kept && confirmingThis && <p className="text-caption text-your-call">{t("largeOld.personalNote")}</p>}
            </section>
          );
        })}
      </div>
      <button type="button" className="link focus-ring self-start" onClick={search}>
        {t("dupes.again")}
      </button>
    </div>
  );
}
