import { describe, expect, it, vi } from "vitest";
import { api } from "../api";
import type { Addon, AddonStatus, Snapshot } from "../api";
import { store } from "../store.svelte";

const addon = (id: string, extra: Partial<Addon> = {}): Addon => ({
  id, name: id, author: null, folders: [id], source: "github", sourceLabel: "GitHub", sourceId: `o/${id}`,
  version: "1.0", installedAt: null, pinned: false, skippedVersion: "", flavourMismatch: false, managed: true,
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
  it("sorts addons into All, Updates, Issues and Pinned", () => {
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
    // Once updated it stays listed until the next check.
    store.updatedNow.questie = "2.0";
    expect(store.inTab(addons[0], "updates")).toBe(true);
    store.updatedNow = {};
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

describe("Update all", () => {
  it("runs once however often it is clicked, and never resets a running download", async () => {
    seed();
    store.statuses.dbm = status("dbm", "available");
    const calls: string[] = [];
    const finish: Record<string, () => void> = {};
    const update = vi.spyOn(api, "update").mockImplementation((_inst, id) => {
      calls.push(id);
      return new Promise((resolve) => (finish[id] = () => resolve(null)));
    });
    const snapshot = vi.spyOn(api, "snapshot").mockImplementation(async () => store.snap!);
    const tick = () => new Promise((r) => setTimeout(r, 0));

    const first = store.updateAll();
    expect(store.updatingAll).toBe(true);
    await tick();
    store.progress.questie = 40;
    const second = store.updateAll();
    await store.update("questie");
    expect(store.progress.questie).toBe(40);

    finish.questie();
    await tick();
    finish.dbm();
    await Promise.all([first, second]);
    expect(calls).toEqual(["questie", "dbm"]);
    expect(store.updatingAll).toBe(false);
    update.mockRestore();
    snapshot.mockRestore();
  });
});

describe("jobs stay with their game client", () => {
  it("Update all keeps to the client it was clicked on after a switch", async () => {
    seed();
    store.statuses.dbm = status("dbm", "available");
    const calls: string[] = [];
    const finish: Record<string, () => void> = {};
    const update = vi.spyOn(api, "update").mockImplementation((inst, id) => {
      calls.push(`${inst}:${id}`);
      return new Promise((resolve) => (finish[id] = () => resolve(null)));
    });
    const era = { addons: [addon("questie"), addon("dbm")], installations: [], current: "era", recovered: [] } as unknown as Snapshot;
    const forever = { ...store.snap!, recovered: [] } as unknown as Snapshot;
    const snapshot = vi.spyOn(api, "snapshot").mockImplementation(async (id) => (id === "era" ? era : forever));
    const recent = vi.spyOn(api, "recent").mockResolvedValue([]);
    const tick = () => new Promise((r) => setTimeout(r, 0));

    const all = store.updateAll();
    await tick();
    expect(store.busy.questie).toBe("Updating");

    // Switch client mid-run; the same addons have updates there too.
    await store.load("era");
    store.statuses = { questie: status("questie", "available"), dbm: status("dbm", "available") };
    expect(store.busy.questie).toBeUndefined();
    expect(store.updatingAll).toBe(false);

    finish.questie();
    await tick();
    finish.dbm();
    await all;
    expect(calls).toEqual(["forever:questie", "forever:dbm"]);
    expect(store.statuses.questie.kind).toBe("available");
    // Back on the first client nothing is left running.
    await store.load("forever");
    expect(store.busy).toEqual({});
    update.mockRestore();
    snapshot.mockRestore();
    recent.mockRestore();
  });

  it("Undo acts on the client the job ran on", async () => {
    seed();
    const snapshot = vi.spyOn(api, "snapshot").mockImplementation(async () => store.snap!);
    const update = vi.spyOn(api, "update").mockResolvedValue(addon("questie", { version: "2.0" }));
    const rollback = vi.spyOn(api, "rollback").mockResolvedValue({ from: "2.0", to: "1.0" });
    await store.update("questie");
    const undo = store.toast?.undo;
    store.snap = { ...store.snap!, current: "era" };
    undo?.();
    expect(rollback).toHaveBeenCalledWith("forever", "questie", null);
    [snapshot, update, rollback].forEach((m) => m.mockRestore());
  });
});

describe("failures and undo", () => {
  it("Update all says which addons failed and keeps the reason on the row", async () => {
    seed();
    store.statuses.dbm = status("dbm", "available");
    const snapshot = vi.spyOn(api, "snapshot").mockImplementation(async () => store.snap!);
    const update = vi.spyOn(api, "update").mockImplementation(async (_i, id) => {
      if (id === "questie") throw new Error("github.com could not be reached.");
      return addon("dbm", { version: "2.0" });
    });
    await store.updateAll();
    expect(store.toast).toMatchObject({ kind: "err", text: "Updated dbm. Couldn't update questie." });
    expect(store.failed.questie).toBe("github.com could not be reached.");
    expect(store.statuses.questie.kind).toBe("available");
    [snapshot, update].forEach((m) => m.mockRestore());
  });

  it("undoing a skip lists the update again", async () => {
    const addons = seed();
    const snapshot = vi.spyOn(api, "snapshot").mockImplementation(async () => store.snap!);
    const skip = vi.spyOn(api, "skip").mockResolvedValue(undefined);
    store.statuses.questie = { ...status("questie", "available"), release: { version: "2.0" } as AddonStatus["release"] };
    await store.skip(addons[0], "2.0");
    expect(store.statuses.questie.kind).toBe("skipped");
    store.toast?.undo?.();
    await new Promise((r) => setTimeout(r, 0));
    expect(store.statuses.questie.kind).toBe("available");
    [snapshot, skip].forEach((m) => m.mockRestore());
  });

  it("undoing an install over the player's own folders rolls back; a new addon is removed", async () => {
    const addons = seed();
    const snapshot = vi.spyOn(api, "snapshot").mockImplementation(async () => store.snap!);
    const rollback = vi.spyOn(api, "rollback").mockResolvedValue({ from: "2.0", to: "1.0" });
    const remove = vi.spyOn(api, "remove").mockResolvedValue(undefined);
    const install = vi.spyOn(api, "install").mockResolvedValue(addon("unknown", { folders: ["Unknown"], version: "2.0" }));
    await store.install("github:o/unknown", addons[3], false);
    store.toast?.undo?.();
    expect(rollback).toHaveBeenCalledWith("forever", "unknown", null);

    install.mockResolvedValue(addon("fresh"));
    await new Promise((r) => setTimeout(r, 0));
    await store.install("github:o/fresh", null, false);
    store.toast?.undo?.();
    expect(remove).toHaveBeenCalledWith("forever", "fresh");
    [snapshot, rollback, remove, install].forEach((m) => m.mockRestore());
  });

  it("does not say All up to date when addons could not be checked", () => {
    seed();
    store.sourceErrors = [];
    expect(store.unchecked).toBe("1 addon couldn't be checked.");
    store.statuses.details = status("details", "up_to_date");
    expect(store.unchecked).toBeNull();
  });
});
