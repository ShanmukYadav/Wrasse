import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { api, cautionFor, strongest, type RemovePreview } from "../api";
import { Icon } from "../design/icons/Icon";
import { iconFor, toneFor } from "../design/icons/semantic";
import { t } from "../i18n";
import { formatBytes } from "../lib/format";
import { duration, ease, spring } from "../motion/tokens";
import { pressable } from "../motion/variants";
import { useApp, type ConfirmKind } from "../store/app";

// Confirms a removal before it happens:
// - "caution": OS folders (Documents, Desktop, …) and data marked Keep; needs "I understand".
// - "deleteNow", "emptyTray", "emptyTrash": gone for good; labeled "Not undoable".
// For items it asks the core for a preview first, so it can say what will be skipped (and why)
// and warn when iCloud syncs an item to the user's other devices.
export function CautionSheet() {
  const c = useApp((s) => s.confirmDelete);
  return <AnimatePresence>{c && <Sheet key={`${c.kind}:${c.ids.join(",")}`} ids={c.ids} kind={c.kind} handPicked={c.handPicked} />}</AnimatePresence>;
}

function Sheet({ ids, kind, handPicked }: { ids: number[]; kind: ConfirmKind; handPicked: boolean }) {
  const disk = useApp((s) => s.disk);
  const units = useApp((s) => s.info?.units ?? "decimal");
  const platform = useApp((s) => s.info?.platform ?? "macOS");
  const tray = useApp((s) => s.tray);
  const dryRun = useApp((s) => s.dryRun);
  const { confirmDeleteRequest, cancelDelete } = useApp.getState();
  const [understood, setUnderstood] = useState(false);
  const [preview, setPreview] = useState<RemovePreview | null>(null);
  const items = ids.map((id) => disk.nodes[id]).filter(Boolean);
  const fmt = (b: number) => {
    const f = formatBytes(b, units);
    return `${f.value} ${f.unit}`;
  };

  useEffect(() => {
    if (kind !== "caution" && kind !== "deleteNow") return;
    let live = true;
    void api
      .preview(disk, ids, { handPicked, deleteNow: kind === "deleteNow" })
      .then((p) => live && setPreview(p))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [disk, ids, kind, handPicked]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && cancelDelete();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cancelDelete]);

  const permanent = kind !== "caution";
  const total = kind === "emptyTray" ? tray.reduce((s, i) => s + i.size, 0) : (preview?.bytes ?? items.reduce((s, n) => s + n.size, 0));
  const title =
    kind === "emptyTray"
      ? t("confirm.emptyTrayTitle", { count: tray.length })
      : kind === "emptyTrash"
        ? t("confirm.emptyTrashTitle")
        : items.length === 1
          ? t("caution.titleOne", { name: items[0].name })
          : t("caution.titleMany", { count: items.length });
  const body =
    kind === "emptyTray"
      ? t("confirm.emptyTrayBody")
      : kind === "emptyTrash"
        ? t("confirm.emptyTrashBody")
        : kind === "deleteNow"
          ? t("confirm.deleteNowBody")
          : strongest(disk, ids) === "osFolder"
            ? t("caution.osFolder", { names: items.filter((n) => cautionFor(disk, n.id).kind === "osFolder").map((n) => n.name).join(", "), platform })
            : t("caution.protected");
  const synced = preview?.synced.length ?? 0;
  const skipped = preview?.skipped ?? [];
  const needsTick = kind === "caution";
  const listed = kind === "emptyTray" ? tray.map((i) => ({ key: i.entry ?? String(i.id), name: i.name, path: i.path ?? "", size: i.size, node: disk.nodes[i.id] })) : items.map((n) => ({ key: String(n.id), name: n.name, path: n.path, size: n.size, node: n }));

  return (
    <motion.div
      className="overlay-scrim"
      onClick={cancelDelete}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1, transition: { duration: duration.base, ease: ease.out } }}
      exit={{ opacity: 0, transition: { duration: duration.base, ease: ease.inOut } }}
    >
      <motion.div
        role="alertdialog"
        aria-modal="true"
        aria-label={title}
        className="overlay-panel caution glass-css"
        onClick={(e) => e.stopPropagation()}
        initial={{ opacity: 0, scale: 0.96 }}
        animate={{ opacity: 1, scale: 1, transition: spring.soft }}
        exit={{ opacity: 0, scale: 0.96, transition: { duration: duration.fast, ease: ease.inOut } }}
      >
        <div className="caution-head">
          <span className="caution-icon">
            <Icon name={permanent ? "trash" : "warning"} tone="warning" size={24} />
          </span>
          <h2 className="text-title">{title}</h2>
          {permanent && <span className="chip chip-amber">{t("detail.notUndoable")}</span>}
        </div>
        <p className="text-label">{body}</p>
        {synced > 0 && (
          <p className="caution-synced text-label">
            <Icon name="cloud" tone="warning" size={16} /> {t("confirm.synced", { count: synced })}
          </p>
        )}
        {kind !== "emptyTrash" && (
          <ul className="caution-list">
            {listed.slice(0, 5).map((it) => (
              <li key={it.key} className="caution-item">
                <Icon name={it.node ? iconFor(it.node) : "file"} tone={it.node ? toneFor(it.node) : "neutral"} size={16} />
                <span className="caution-name">{it.name}</span>
                <span className="text-caption text-muted">{it.path}</span>
                <span className="caution-size tabular">{fmt(it.size)}</span>
              </li>
            ))}
            {listed.length > 5 && <li className="text-caption text-muted">{t("caution.more", { count: listed.length - 5 })}</li>}
          </ul>
        )}
        {skipped.length > 0 && (
          <p className="text-caption text-your-call">
            {t("confirm.skipped", { count: skipped.length })}{" "}
            {[...new Set(skipped.map((s) => t(`skip.${s.reason}`)))].join(" · ")}
          </p>
        )}
        <p className="text-caption text-muted">{permanent ? t("confirm.permanentNote") : t("caution.trayNote")}</p>
        {dryRun && <p className="text-caption text-muted">{t("confirm.dryRunNote")}</p>}
        {needsTick && (
          <label className="caution-check">
            <input type="checkbox" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} />
            <span className="text-label">{t("caution.understand")}</span>
          </label>
        )}
        <div className="caution-actions">
          <button type="button" className="btn focus-ring" onClick={cancelDelete} autoFocus>
            {t("caution.cancel")}
          </button>
          <motion.button type="button" className="btn btn-danger focus-ring" disabled={needsTick && !understood} onClick={confirmDeleteRequest} {...pressable}>
            <Icon name="trash" size={16} />{" "}
            {kind === "caution"
              ? t("caution.confirm", { size: fmt(total) })
              : kind === "emptyTray"
                ? t("confirm.emptyTrayButton", { size: fmt(total) })
                : kind === "emptyTrash"
                  ? t("confirm.emptyTrashButton", { size: fmt(items[0]?.size ?? 0) })
                  : t("confirm.deleteNowButton", { size: fmt(total) })}
          </motion.button>
        </div>
      </motion.div>
    </motion.div>
  );
}
