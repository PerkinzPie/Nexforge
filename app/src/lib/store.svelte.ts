// App state. Paints from the local snapshot first; network results update in place.

import { api, inTauri, onProgress, type Addon, type AddonStatus, type Recent, type Snapshot, type SourceError } from "./api";

export type View = "installed" | "updates" | "browse" | "settings";
export type InstalledTab = "all" | "updates" | "attention" | "pinned";
export interface Toast { kind: "ok" | "err"; text: string; undo?: () => void }

export type Dialog =
  | { kind: "install"; adopt?: Addon; initial?: string }
  | { kind: "remove"; addon: Addon }
  | { kind: "key"; name: "curseforge" | "github" }
  | { kind: "anyway"; spec: string; name: string };

class Store {
  snap = $state<Snapshot | null>(null);
  loadError = $state<string | null>(null);
  view = $state<View>("installed");
  statuses = $state<Record<string, AddonStatus>>({});
  checkedAt = $state<string | null>(null);
  checking = $state(false);
  sourceErrors = $state<SourceError[]>([]);
  /** id → percent (null while the size is unknown). Present while downloading. */
  progress = $state<Record<string, number | null>>({});
  /** id → what is happening ("Updating", "Rolling back" …). */
  busy = $state<Record<string, string>>({});
  toast = $state<Toast | null>(null);
  /** Installs started from Install from URL, shown in the activity tray. */
  jobs = $state<Record<string, { title: string }>>({});
  recent = $state<Recent[]>([]);
  dialog = $state<Dialog | null>(null);
  filter = $state("");
  tab = $state<InstalledTab>("all");
  /** Show the welcome screen (no config existed at launch). */
  firstRun = $state(false);
  readonly preview = !inTauri;

  #toastTimer: ReturnType<typeof setTimeout> | undefined;

  get current() {
    return this.snap?.current ?? null;
  }
  get installation() {
    return this.snap?.installations.find((i) => i.id === this.current) ?? null;
  }
  get addons() {
    return this.snap?.addons ?? [];
  }
  status(id: string): AddonStatus | undefined {
    return this.statuses[id];
  }
  get updates(): Addon[] {
    return this.addons.filter((a) => this.statuses[a.id]?.kind === "available");
  }
  get pinnedCount() {
    return this.addons.filter((a) => a.pinned).length;
  }
  /** Not managed yet, no build for this flavour, or its check failed. */
  needsLook(a: Addon): boolean {
    const k = this.statuses[a.id]?.kind;
    if (!a.managed || k === "no_build" || k === "failed" || k === "unavailable") return true;
    return !k && a.flavourMismatch;
  }
  get attentionCount() {
    return this.addons.filter((a) => this.needsLook(a)).length;
  }
  /** Does `a` belong in the Installed tab `t`? */
  inTab(a: Addon, t: InstalledTab): boolean {
    switch (t) {
      case "all": return true;
      case "updates": return this.statuses[a.id]?.kind === "available" || this.busy[a.id] === "Updating";
      case "attention": return this.needsLook(a);
      case "pinned": return a.pinned;
    }
  }

  /** Show a toast; `undo` adds an Undo button and keeps it up longer. */
  say(kind: "ok" | "err", text: string, undo?: () => void) {
    this.toast = { kind, text, undo };
    clearTimeout(this.#toastTimer);
    this.#toastTimer = setTimeout(() => (this.toast = null), kind === "err" ? 9000 : undo ? 8000 : 4000);
  }

  async start() {
    await onProgress((p) => {
      if (p.id in this.progress || p.id in this.busy) this.progress[p.id] = p.percent;
    });
    await this.load(null);
    if (this.snap?.firstRun) {
      this.firstRun = true;
      return; // the welcome screen adopts, then calls finishFirstRun()
    }
    if (this.snap?.adoptedNow) {
      this.say("ok", `Adopted ${this.addons.length} addons.`);
    }
    if (this.snap?.settings.checkOnLaunch && this.current) {
      await this.check();
      if (this.snap?.settings.autoApply) await this.updateAll();
    }
  }

  async finishFirstRun() {
    this.firstRun = false;
    await this.load(null);
    if (this.snap?.settings.checkOnLaunch && this.current) await this.check();
  }

  /** Re-read local state (fast; no network). */
  async load(installationId: string | null = this.current) {
    try {
      const s = await api.snapshot(installationId);
      if (s.current !== this.snap?.current) {
        this.tab = "all";
        this.statuses = {};
        this.checkedAt = null;
        this.sourceErrors = [];
      }
      this.snap = s;
      this.loadError = null;
      if (s.current) this.recent = await api.recent(s.current).catch(() => []);
    } catch (e) {
      this.loadError = (e as Error).message;
    }
  }

  async switchTo(id: string) {
    if (id === this.current) return;
    await this.load(id);
    if (this.snap?.settings.checkOnLaunch) await this.check();
  }

  async check() {
    const inst = this.current;
    if (!inst || this.checking) return;
    this.checking = true;
    try {
      const r = await api.check(inst);
      if (inst !== this.current) return;
      const next: Record<string, AddonStatus> = {};
      for (const s of r.results) next[s.id] = s;
      this.statuses = next;
      this.checkedAt = r.checkedAt;
      this.sourceErrors = r.sourceErrors;
    } catch (e) {
      this.say("err", (e as Error).message);
    } finally {
      this.checking = false;
    }
  }

  async run<T>(
    id: string,
    label: string,
    job: () => Promise<T>,
    done?: (r: T) => string | null,
    undo?: (r: T) => (() => void) | undefined,
  ): Promise<T | undefined> {
    if (this.busy[id]) return;
    this.busy[id] = label;
    try {
      const r = await job();
      const msg = done?.(r);
      if (msg) this.say("ok", msg, undo?.(r));
      return r;
    } catch (e) {
      this.say("err", (e as Error).message);
    } finally {
      delete this.busy[id];
      delete this.progress[id];
    }
  }

  async update(id: string) {
    const inst = this.current;
    const a = this.addons.find((x) => x.id === id);
    if (!inst || !a) return;
    this.progress[id] = 0;
    const r = await this.run(id, "Updating", () => api.update(inst, id), (r) =>
      r ? `Updated ${r.name} to ${r.version}.` : `${a.name} is up to date.`,
      (r) => (r ? () => this.rollback(id, a.name) : undefined),
    );
    if (r !== undefined) {
      const s = this.statuses[id];
      if (s) this.statuses[id] = { ...s, kind: "up_to_date", release: null };
      await this.load();
    }
  }

  async updateAll() {
    for (const a of this.updates.filter((x) => !x.pinned)) await this.update(a.id);
  }

  async setPinned(a: Addon, pinned: boolean) {
    const inst = this.current;
    if (!inst) return;
    await this.run(a.id, pinned ? "Pinning" : "Unpinning", () => api.setPinned(inst, a.id, pinned),
      () => `${pinned ? "Pinned" : "Unpinned"} ${a.name}.`, () => () => this.setPinned(a, !pinned));
    await this.load();
    const s = this.statuses[a.id];
    if (s && pinned && s.kind === "available") this.statuses[a.id] = { ...s, kind: "pinned" };
    if (s && !pinned && s.kind === "pinned") this.statuses[a.id] = { ...s, kind: s.release ? "available" : "up_to_date" };
  }

  async skip(a: Addon, version: string) {
    const inst = this.current;
    if (!inst) return;
    await this.run(a.id, "Skipping", () => api.skip(inst, a.id, version),
      () => (version ? `Skipping ${a.name} ${version}.` : null),
      () => (version ? () => this.skip(a, "") : undefined));
    const s = this.statuses[a.id];
    if (s?.release) this.statuses[a.id] = { ...s, kind: "skipped" };
    await this.load();
  }

  async rollback(id: string, name: string) {
    const inst = this.current;
    if (!inst) return;
    const r = await this.run(id, "Rolling back", () => api.rollback(inst, id), (r) =>
      `Rolled ${name} back to ${r.to}.`,
    );
    if (r) {
      const s = this.statuses[id];
      if (s) this.statuses[id] = { ...s, kind: "skipped" };
      await this.load();
    }
  }

  async remove(a: Addon) {
    const inst = this.current;
    if (!inst) return;
    const ok = await this.run(a.id, "Removing", () => api.remove(inst, a.id).then(() => true), () =>
      `Removed ${a.name}.`,
      () => () => this.restore(a.id, a.name),
    );
    if (ok) {
      delete this.statuses[a.id];
      await this.load();
    }
  }

  /** Put a removed addon back (undo of remove). */
  async restore(id: string, name: string) {
    const inst = this.current;
    if (!inst) return;
    this.jobs[id] = { title: name };
    const r = await this.run(id, "Restoring", () => api.restore(inst, id), (e) => `Restored ${e.name}.`);
    delete this.jobs[id];
    if (r) await this.load();
  }

  /** Install by spec (source:id, URL or name). `title` puts it in the
   *  activity tray (used when there is no row to show progress on). */
  async install(spec: string, adopt: Addon | null, anyway: boolean, title?: string): Promise<boolean> {
    const inst = this.current;
    if (!inst) return false;
    const key = adopt?.id ?? spec;
    const existed = adopt ? this.addons.some((x) => x.id === adopt.id && x.managed) : false;
    this.progress[key] = 0;
    if (title) this.jobs[key] = { title };
    const r = await this.run(key, "Installing", () => api.install(inst, spec, adopt?.id ?? null, anyway), (e) =>
      `Installed ${e.name} ${e.version}.`,
      // Undo a fresh install by removing it; a re-install by rolling back.
      (e) => () => (existed ? this.rollback(e.id, e.name) : this.removeQuietly(e.id, e.name)),
    );
    delete this.jobs[key];
    if (r) {
      this.statuses[r.id] = { id: r.id, kind: "up_to_date", release: null, message: null };
      await this.load();
      return true;
    }
    return false;
  }

  /** Remove without the confirm dialog (undo of an install). */
  async removeQuietly(id: string, name: string) {
    const a = this.addons.find((x) => x.id === id);
    if (a) await this.remove(a);
    else this.say("err", `${name} is no longer installed.`);
  }
}

export const store = new Store();
