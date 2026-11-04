// App state. Paints from the local snapshot first; network results update in place.

import { api, inTauri, onProgress, type Addon, type AddonStatus, type Recent, type Snapshot, type SourceError } from "./api";

export type View = "installed" | "updates" | "browse" | "settings";

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
  toast = $state<{ kind: "ok" | "err"; text: string } | null>(null);
  recent = $state<Recent[]>([]);
  dialog = $state<Dialog | null>(null);
  filter = $state("");
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
  get attentionCount() {
    return this.addons.filter((a) => {
      const k = this.statuses[a.id]?.kind;
      return !a.managed || k === "no_build" || k === "failed" || k === "unavailable";
    }).length;
  }

  say(kind: "ok" | "err", text: string) {
    this.toast = { kind, text };
    clearTimeout(this.#toastTimer);
    this.#toastTimer = setTimeout(() => (this.toast = null), kind === "err" ? 9000 : 4000);
  }

  async start() {
    await onProgress((p) => {
      if (p.id in this.progress || p.id in this.busy) this.progress[p.id] = p.percent;
    });
    await this.load(null);
    if (this.snap?.adoptedNow) {
      this.say("ok", `Found ${this.addons.length} addons in AddOns and wrote addons.toml. Nothing was moved.`);
    }
    if (this.snap?.settings.checkOnLaunch && this.current) {
      await this.check();
      if (this.snap?.settings.autoApply) await this.updateAll();
    }
  }

  /** Re-read local state (fast; no network). */
  async load(installationId: string | null = this.current) {
    try {
      const s = await api.snapshot(installationId);
      if (s.current !== this.snap?.current) {
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

  async run<T>(id: string, label: string, job: () => Promise<T>, done?: (r: T) => string | null): Promise<T | undefined> {
    if (this.busy[id]) return;
    this.busy[id] = label;
    try {
      const r = await job();
      const msg = done?.(r);
      if (msg) this.say("ok", msg);
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
      r ? `Updated ${r.name} to ${r.version}. The previous version is kept for rollback.` : `${a.name} is already up to date.`,
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
    await this.run(a.id, pinned ? "Pinning" : "Unpinning", () => api.setPinned(inst, a.id, pinned));
    await this.load();
    const s = this.statuses[a.id];
    if (s && pinned && s.kind === "available") this.statuses[a.id] = { ...s, kind: "pinned" };
    if (s && !pinned && s.kind === "pinned") this.statuses[a.id] = { ...s, kind: s.release ? "available" : "up_to_date" };
  }

  async skip(a: Addon, version: string) {
    const inst = this.current;
    if (!inst) return;
    await this.run(a.id, "Skipping", () => api.skip(inst, a.id, version), () => `Skipping ${a.name} ${version}.`);
    const s = this.statuses[a.id];
    if (s?.release) this.statuses[a.id] = { ...s, kind: "skipped" };
    await this.load();
  }

  async rollback(id: string, name: string) {
    const inst = this.current;
    if (!inst) return;
    const r = await this.run(id, "Rolling back", () => api.rollback(inst, id), (r) =>
      `Rolled ${name} back from ${r.from} to ${r.to}. ${r.from} will be skipped.`,
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
      `Removed ${a.name}. A copy is kept in the data folder.`,
    );
    if (ok) {
      delete this.statuses[a.id];
      await this.load();
    }
  }

  /** Install by spec (source:id, URL or name). Returns true on success. */
  async install(spec: string, adopt: Addon | null, anyway: boolean): Promise<boolean> {
    const inst = this.current;
    if (!inst) return false;
    const key = adopt?.id ?? spec;
    this.progress[key] = 0;
    const r = await this.run(key, "Installing", () => api.install(inst, spec, adopt?.id ?? null, anyway), (e) =>
      `Installed ${e.name} ${e.version}.`,
    );
    if (r) {
      this.statuses[r.id] = { id: r.id, kind: "up_to_date", release: null, message: null };
      await this.load();
      return true;
    }
    return false;
  }
}

export const store = new Store();
