// The ⋯ actions for an installed addon (Installed rows and the details page).
import type { Item } from "../components/RowMenu.svelte";
import { api, type Addon } from "./api";
import { store } from "./store.svelte";

export function addonMenu(a: Addon, opts: { details?: boolean } = {}): Item[] {
  const s = store.status(a.id);
  const items: Item[] = [];
  if (opts.details && a.managed && a.source)
    items.push({ label: "Details", run: () => openDetails(a) });
  if (a.managed) items.push({ label: a.pinned ? "Unpin" : "Pin (never update)", run: () => store.setPinned(a, !a.pinned) });
  if (s?.release && (s.kind === "available" || s.kind === "pinned"))
    items.push({ label: `Skip ${s.release.version}`, run: () => store.skip(a, s.release!.version) });
  if (s?.kind === "skipped" && a.skippedVersion)
    items.push({ label: `Stop skipping ${a.skippedVersion}`, run: () => store.skip(a, "") });
  // After a rollback the newest kept version is the newer one.
  const back = s?.release?.version === a.kept[0] ? "Return to" : "Roll back to";
  if (a.kept.length) items.push({ label: `${back} ${a.kept[0]}`, run: () => store.rollback(a.id, a.name, a.kept[0]) });
  items.push({ label: a.managed ? "Change source" : "Identify source", run: () => (store.dialog = { kind: "install", adopt: a }) });
  items.push({ label: "Open folder", run: () => api.open("addon", store.current!, a.id).catch((e) => store.say("err", e.message)) });
  if (a.website) items.push({ label: "Open website", run: () => api.openUrl(a.website!).catch((e) => store.say("err", e.message)) });
  items.push({ label: "Remove", danger: true, run: () => (store.dialog = { kind: "remove", addon: a }) });
  return items;
}

/** Open an installed addon's details page. */
export function openDetails(a: Addon) {
  store.detail = { name: a.name, iconUrl: a.iconUrl, source: a.managed ? a.source : null, sourceId: a.sourceId, addonId: a.id };
}

/** A click anywhere on a row or card opens it, except on its own controls. */
export function clickRow(e: MouseEvent, open: () => void) {
  const t = e.target as Element;
  if (t.closest("a, input, select, [role=menu], button:not([data-row-open])")) return;
  if (window.getSelection()?.toString()) return;
  open();
}
