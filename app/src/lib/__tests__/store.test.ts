import { describe, expect, it } from "vitest";
import type { Addon, AddonStatus, Snapshot } from "../api";
import { store } from "../store.svelte";

const addon = (id: string, extra: Partial<Addon> = {}): Addon => ({
  id, name: id, author: null, folders: [id], source: "github", sourceLabel: "GitHub", sourceId: `o/${id}`,
  version: "1.0", pinned: false, skippedVersion: "", flavourMismatch: false, managed: true,
  website: null, iconUrl: null, knownIds: [], kept: [], ...extra,
});
const status = (id: string, kind: AddonStatus["kind"]): AddonStatus => ({ id, kind, release: null, message: null });

function seed() {
  const addons = [
    addon("questie"),
    addon("dbm"),
    addon("castbars"),
    addon("unknown", { managed: false, source: null, sourceId: "" }),
    addon("atlas", { pinned: true }),
    addon("details", { source: "curseforge" }),
    addon("oldtoc", { flavourMismatch: true }),
  ];
  store.snap = { addons, installations: [], current: "forever" } as unknown as Snapshot;
  store.statuses = {
    questie: status("questie", "available"),
    dbm: status("dbm", "up_to_date"),
    castbars: status("castbars", "no_build"),
    unknown: status("unknown", "not_managed"),
    atlas: status("atlas", "pinned"),
    details: status("details", "unavailable"),
  };
  return addons;
}

describe("Installed tabs", () => {
  it("sorts addons into All, Updates, Can't update and Pinned", () => {
    const addons = seed();
    const ids = (t: Parameters<typeof store.inTab>[1]) => addons.filter((a) => store.inTab(a, t)).map((a) => a.id);
    expect(ids("all")).toHaveLength(7);
    expect(ids("updates")).toEqual(["questie"]);
    // No source, no build for this flavour, source needs a key, or an
    // unchecked addon whose TOC does not load here.
    expect(ids("attention")).toEqual(["castbars", "unknown", "details", "oldtoc"]);
    expect(ids("pinned")).toEqual(["atlas"]);
    expect(store.attentionCount).toBe(4);
    expect(store.updates.map((a) => a.id)).toEqual(["questie"]);
  });

  it("an addon being updated stays on the Updates tab", () => {
    const addons = seed();
    store.statuses.questie = status("questie", "up_to_date");
    store.busy.questie = "Updating";
    expect(store.inTab(addons[0], "updates")).toBe(true);
    delete store.busy.questie;
    expect(store.inTab(addons[0], "updates")).toBe(false);
  });
});

describe("toasts", () => {
  it("carry an undo action and clear when dismissed", () => {
    let undone = false;
    store.say("ok", "Removed Questie.", () => (undone = true));
    expect(store.toast?.text).toBe("Removed Questie.");
    store.toast?.undo?.();
    expect(undone).toBe(true);
    store.toast = null;
    expect(store.toast).toBeNull();
  });
});
