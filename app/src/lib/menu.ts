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
  items.push({ label: a.managed ? "Change source…" : "Identify source…", run: () => (store.dialog = { kind: "install", adopt: a }) });
  items.push({ label: "Open folder", run: () => api.open("addon", store.current!, a.id).catch((e) => store.say("err", e.message)) });
  if (a.website) items.push({ label: "Open website", run: () => api.openUrl(a.website!).catch((e) => store.say("err", e.message)) });
  items.push({ label: "Remove…", danger: true, run: () => (store.dialog = { kind: "remove", addon: a }) });
  return items;
}

/** Open an installed addon's details page. */
export function openDetails(a: Addon) {
  store.detail = { name: a.name, iconUrl: a.iconUrl, source: a.managed ? a.source : null, sourceId: a.sourceId, addonId: a.id };
}
