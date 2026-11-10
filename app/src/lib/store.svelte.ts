// App state. Paints from the local snapshot first; network results update in place.

import { api, inTauri, onProgress, type Addon, type AddonStatus, type Hit, type Recent, type Snapshot, type SourceError } from "./api";
import { plural } from "./format";

export type View = "installed" | "updates" | "browse" | "settings";
export type InstalledTab = "all" | "updates" | "attention" | "pinned";
export interface Toast { kind: "ok" | "err"; text: string; undo?: () => void }
/** An addon's details page, opened from a row; Back returns to `from`. */
export interface DetailTarget {
  name: string; iconUrl: string | null; source: string | null; sourceId: string;
  /** Installed addon, if any. */
  addonId?: string;
  /** The Browse result it came from, if any. */
  hit?: Hit;
}

export type Dialog =
  | { kind: "install"; adopt?: Addon; initial?: string }
  | { kind: "remove"; addon: Addon }
  | { kind: "key"; name: "curseforge" | "github" }
  | { kind: "anyway"; spec: string; name: string; adopt?: Addon };

interface Jobs { busy: Record<string, string>; progress: Record<string, number | null> }
type Updated = "updated" | "current" | "failed" | "busy";

class Store {
  snap = $state<Snapshot | null>(null);
  loadError = $state<string | null>(null);
  #view = $state<View>("installed");
  /** The open details page, drawn over the current view. */
  detail = $state<DetailTarget | null>(null);
  get view() {
    return this.#view;
  }
  /** Going to any view closes a details page. */
  set view(v: View) {
    this.#view = v;
    this.detail = null;
  }
  statuses = $state<Record<string, AddonStatus>>({});
  checkedAt = $state<string | null>(null);
  checking = $state(false);
  sourceErrors = $state<SourceError[]>([]);
  // Jobs belong to the game client they were started on: addon ids repeat
  // across clients, so `progress` and `busy` hold the current client's jobs
  // and the rest wait in `#parked` until that client is shown again.
  /** id → percent (null while the size is unknown). Present while downloading. */
  progress = $state<Record<string, number | null>>({});
  /** id → what is happening ("Updating", "Rolling back" …). */
  busy = $state<Record<string, string>>({});
  #parked = $state<Record<string, Jobs>>({});
  #updatingAllOn = $state<Record<string, boolean>>({});
  /** Update all is working through this client's list; its button stays disabled until done. */
  get updatingAll() {
    return !!this.#updatingAllOn[this.current ?? ""];
  }
  /** id → why its last update failed. Cleared by a retry or the next check. */
  failed = $state<Record<string, string>>({});
  toast = $state<Toast | null>(null);
  /** Installs started from Install from URL, shown in the activity tray. */
  jobs = $state<Record<string, { title: string }>>({});
  recent = $state<Recent[]>([]);
  dialog = $state<Dialog | null>(null);
  filter = $state("");
  /** The top bar's search, shown by Browse. */
  query = $state("");
  tab = $state<InstalledTab>("all");
  /** A newer Nexforge, when the opt-in check found one. */
  newVersion = $state<{ version: string; url: string } | null>(null);
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
  /** Why "All up to date" can't be said after the last check, if it can't. */
  get unchecked(): string | null {
    const n = this.addons.filter((a) => ["failed", "unavailable"].includes(this.statuses[a.id]?.kind ?? "")).length;
    if (n) return `${plural(n, "addon")} couldn't be checked.`;
    return this.sourceErrors.length ? "Some addons couldn't be checked." : null;
  }
  get attentionCount() {
    return this.addons.filter((a) => this.needsLook(a)).length;
  }
  /** Does `a` belong in the Installed tab `t`? */
  inTab(a: Addon, t: InstalledTab): boolean {
    switch (t) {
      case "all": return true;
      // A row stays until the next check once its update is done, so the list never closes up under the pointer.
      case "updates": return this.statuses[a.id]?.kind === "available" || this.busy[a.id] === "Updating" || a.id in this.updatedNow;
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
      const j = p.installation === this.current ? this : this.#parked[p.installation];
      if (j && (p.id in j.progress || p.id in j.busy)) j.progress[p.id] = p.percent;
    });
    await this.load(null);
    // Back from copying addons in by hand: look again (at most every few seconds).
    window.addEventListener("focus", () => {
      if (Date.now() - this.#rescannedAt > 3000) this.rescan();
    });
    if (this.snap?.firstRun) {
      this.firstRun = true;
      return; // the welcome screen adopts, then calls finishFirstRun()
    }
    if (this.snap?.adoptedNow) {
      this.say("ok", `Found ${this.addons.length} addons.`);
    } else {
      await this.rescan();
    }
    const st = this.snap?.settings;
    if ((st?.checkOnLaunch || st?.autoApply) && this.current) await this.check();
    if (st?.autoApply) void this.#applyElsewhere();
    this.refreshCatalogue();
    this.checkSelf();
  }

  /** Auto-apply for the clients not on screen: check each, update what it
   *  lists (pinned and skipped addons are not listed), and say how it went. */
  async #applyElsewhere() {
    for (const i of this.snap?.installations ?? []) {
      if (i.id === this.current || !i.detected) continue;
      let done = 0;
      let failed = 0;
      try {
        const r = await api.check(i.id);
        for (const x of r.results.filter((x) => x.kind === "available")) {
          const u = await this.update(x.id, { inst: i.id, name: x.id, quiet: true });
          if (u === "updated") done++;
          if (u === "failed") failed++;
        }
      } catch {
        continue; // its own check says why when that client is opened
      }
      if (failed) this.say("err", `${i.label}: updated ${plural(done, "addon")}, ${failed} failed.`);
      else if (done) this.say("ok", `${i.label}: updated ${plural(done, "addon")}.`);
    }
  }

  /** Opt-in: is there a newer Nexforge? (Off unless turned on in Settings.) */
  checkSelf() {
    api.selfUpdate().then((n) => (this.newVersion = n)).catch(() => {});
  }

  /** Settings' Check now: always asks, and says what it found. */
  checkingSelf = $state(false);
  async checkSelfNow() {
    if (this.checkingSelf) return;
    this.checkingSelf = true;
    try {
      const n = await api.selfUpdate(true);
      this.newVersion = n;
      this.say("ok", n ? `Nexforge ${n.version} is available.` : `Nexforge ${this.snap?.settings.version ?? ""} is the latest version.`);
    } catch (e) {
      this.say("err", (e as Error).message);
    } finally {
      this.checkingSelf = false;
    }
  }

  /** Refill the Browse catalogue in the background (at most daily), then
   *  repaint so Installed picks up host icons it now knows. */
  refreshCatalogue() {
    api.refreshCatalogue(false).then(() => this.load()).catch(() => {});
  }

  async finishFirstRun() {
    this.firstRun = false;
    await this.load(null);
    if (this.snap?.settings.checkOnLaunch && this.current) await this.check();
    this.refreshCatalogue();
    this.checkSelf();
  }

  /** The jobs of one client: the visible maps for the current one. The maps
   *  themselves are returned, so a job keeps its own through a switch. */
  #jobsOf(inst: string): Jobs {
    if (inst === this.current) return { busy: this.busy, progress: this.progress };
    if (!this.#parked[inst]) this.#parked[inst] = { busy: {}, progress: {} };
    return this.#parked[inst];
  }

  /** Re-read local state (fast; no network). */
  async load(installationId: string | null = this.current) {
    try {
      const s = await api.snapshot(installationId);
      const old = this.snap?.current;
      if (s.current !== old) {
        // Another client: its running jobs come back, this one's are set aside.
        if (old) this.#parked[old] = { busy: this.busy, progress: this.progress };
        const back = s.current ? this.#parked[s.current] : undefined;
        this.busy = back?.busy ?? {};
        this.progress = back?.progress ?? {};
        if (s.current) delete this.#parked[s.current];
        this.failed = {};
        this.skippedNow = {};
        this.tab = "all";
        this.statuses = {};
        this.updatedNow = {};
        this.checkedAt = null;
        this.sourceErrors = [];
      }
      this.snap = s;
      this.loadError = null;
      if (s.recovered.length) {
        this.say("ok", `Restored ${s.recovered.join(", ")} after an interrupted update.`);
      }
      if (s.configReset && !this.#toldReset) {
        this.#toldReset = true;
        this.say("err", `Settings couldn't be read and were reset. The old file is saved as ${s.configReset}.`);
      }
      if (s.current) this.recent = await api.recent(s.current).catch(() => []);
    } catch (e) {
      this.loadError = (e as Error).message;
    }
  }

  async switchTo(id: string) {
    if (id === this.current) return;
    await this.load(id);
    await this.rescan();
    if (this.snap?.settings.checkOnLaunch) await this.check();
  }

  /** The Refresh button and Ctrl+R: pick up hand-installed addons, then check. */
  async refresh() {
    if (this.checking) return;
    await this.rescan();
    await this.check(true);
  }

  #rescannedAt = 0;
  /** Pick up addons added to (or removed from) the AddOns folder by hand. */
  async rescan() {
    const inst = this.current;
    if (!inst || this.firstRun) return;
    this.#rescannedAt = Date.now();
    try {
      const added = await api.reconcile(inst);
      if (inst !== this.current) return;
      await this.load();
      if (added) this.say("ok", added === 1 ? "Found 1 new addon." : `Found ${added} new addons.`);
    } catch {
      // A folder we can't read stays as it was; the next look tries again.
    }
  }

  #toldReset = false;
  #checkingFor: string | null = null;
  #recheck = false;

  /** Check this client for updates. `fresh` (the Refresh button) asks the
   *  hosts again rather than trusting a list from earlier today. With
   *  auto-apply on, what the check finds is then updated. */
  async check(fresh = false) {
    const inst = this.current;
    if (!inst) return;
    if (this.checking) {
      // Switched client mid-check: check this one when that finishes.
      if (this.#checkingFor !== inst) this.#recheck = true;
      return;
    }
    this.checking = true;
    this.#checkingFor = inst;
    try {
      const r = await api.check(inst, fresh);
      if (inst !== this.current) return;
      const next: Record<string, AddonStatus> = {};
      for (const s of r.results) next[s.id] = s;
      this.statuses = next;
      this.updatedNow = {};
      this.skippedNow = {};
      this.failed = {};
      this.checkedAt = r.checkedAt;
      this.sourceErrors = r.sourceErrors;
      if (this.snap?.settings.autoApply && !this.firstRun) void this.updateAll();
    } catch (e) {
      this.say("err", (e as Error).message);
    } finally {
      this.checking = false;
      this.#checkingFor = null;
      if (this.#recheck) {
        this.#recheck = false;
        void this.check();
      }
    }
  }

  /** addons.toml could not be read: set it aside and adopt AddOns afresh. */
  async resetState() {
    const inst = this.current;
    if (!inst) return;
    try {
      const kept = await api.resetState(inst);
      await this.load();
      this.say("ok", `Addon list rebuilt. The old one is saved as ${kept}.`);
    } catch (e) {
      this.say("err", (e as Error).message);
    }
  }

  /** Run one job on client `inst`. Errors go to `onError`, or a toast. */
  async run<T>(
    inst: string,
    id: string,
    label: string,
    job: () => Promise<T>,
    done?: (r: T) => string | null,
    undo?: (r: T) => (() => void) | undefined,
    onError?: (message: string) => void,
  ): Promise<T | undefined> {
    const jobs = this.#jobsOf(inst);
    if (jobs.busy[id]) return;
    jobs.busy[id] = label;
    try {
      const r = await job();
      const msg = done?.(r);
      if (msg) this.say("ok", msg, undo?.(r));
      return r;
    } catch (e) {
      const m = (e as Error).message;
      if (onError) onError(m);
      else this.say("err", m);
    } finally {
      delete jobs.busy[id];
      delete jobs.progress[id];
    }
  }

  /** id → version updated to since the last check; Updates keeps their cards in place. */
  updatedNow = $state<Record<string, string>>({});
  /** id → version skipped since the last check; its card stays too. */
  skippedNow = $state<Record<string, string>>({});

  /** Update one addon on client `inst` (the current one unless given).
   *  `quiet` leaves the toast to the caller (Update all sums up). */
  async update(id: string, opts: { inst?: string; name?: string; quiet?: boolean } = {}): Promise<Updated> {
    const inst = opts.inst ?? this.current;
    if (!inst) return "failed";
    const here = () => inst === this.current;
    const name = opts.name ?? this.addons.find((x) => x.id === id)?.name;
    const jobs = this.#jobsOf(inst);
    // Already updating: a second click must not reset its progress bar.
    if (!name || jobs.busy[id]) return "busy";
    jobs.progress[id] = 0;
    if (here()) delete this.failed[id];
    let error: string | null = null;
    const r = await this.run(inst, id, "Updating", () => api.update(inst, id), (r) =>
      opts.quiet ? null : r ? `Updated ${r.name} to ${r.version}.` : `${name} is up to date.`,
      (r) => (r ? () => this.rollback(id, name, null, inst) : undefined),
      (m) => {
        error = m;
        if (!opts.quiet) this.say("err", m);
      },
    );
    if (error !== null) {
      // The row keeps the reason until a retry or the next check.
      if (here()) this.failed[id] = error;
      return "failed";
    }
    if (here()) {
      if (r) this.updatedNow[id] = r.version;
      const s = this.statuses[id];
      // Keep the release (its notes) so the Updates card keeps its size until the next check.
      if (s) this.statuses[id] = { ...s, kind: "up_to_date" };
      await this.load();
    }
    return r ? "updated" : "current";
  }

  /** Update everything listed for this client, then say how it went. The
   *  list belongs to the client it was clicked on, wherever the player goes. */
  async updateAll() {
    const inst = this.current;
    if (!inst || this.#updatingAllOn[inst]) return;
    this.#updatingAllOn[inst] = true;
    const done: string[] = [];
    const failed: string[] = [];
    try {
      for (const a of this.updates.filter((x) => !x.pinned)) {
        if (inst === this.current) {
          // Updated from its own button, pinned or skipped since the list was taken.
          const now = this.addons.find((x) => x.id === a.id);
          if (this.statuses[a.id]?.kind !== "available" || !now || now.pinned) continue;
        }
        const r = await this.update(a.id, { inst, name: a.name, quiet: true });
        if (r === "updated") done.push(a.name);
        if (r === "failed") failed.push(a.name);
      }
    } finally {
      delete this.#updatingAllOn[inst];
    }
    const ok = done.length === 1 ? `Updated ${done[0]}.` : done.length ? `Updated ${done.length} addons.` : "";
    if (failed.length) {
      const which = failed.length > 3 ? plural(failed.length, "addon") : failed.join(", ");
      this.say("err", `${ok} Couldn't update ${which}.`.trim());
    } else if (ok) {
      this.say("ok", ok);
    }
  }

  async setPinned(a: Addon, pinned: boolean, inst = this.current) {
    if (!inst) return;
    const ok = await this.run(inst, a.id, pinned ? "Pinning" : "Unpinning", () => api.setPinned(inst, a.id, pinned).then(() => true),
      () => `${pinned ? "Pinned" : "Unpinned"} ${a.name}.`, () => () => this.setPinned(a, !pinned, inst));
    if (!ok || inst !== this.current) return;
    await this.load();
    const s = this.statuses[a.id];
    if (s && pinned && s.kind === "available") this.statuses[a.id] = { ...s, kind: "pinned" };
    if (s && !pinned && s.kind === "pinned") this.statuses[a.id] = { ...s, kind: s.release ? "available" : "up_to_date" };
  }

  /** Skip `version` of an addon; an empty version stops skipping. */
  async skip(a: Addon, version: string, inst = this.current) {
    if (!inst) return;
    const ok = await this.run(inst, a.id, "Skipping", () => api.skip(inst, a.id, version).then(() => true),
      () => (version ? `Skipping ${a.name} ${version}.` : `No longer skipping ${a.name} updates.`),
      () => (version ? () => this.skip(a, "", inst) : undefined));
    if (!ok || inst !== this.current) return;
    const s = this.statuses[a.id];
    if (s?.release) this.statuses[a.id] = { ...s, kind: version ? "skipped" : a.pinned ? "pinned" : "available" };
    if (version) this.skippedNow[a.id] = version;
    else delete this.skippedNow[a.id];
    await this.load();
  }

  async rollback(id: string, name: string, to: string | null = null, inst = this.current) {
    if (!inst) return;
    const r = await this.run(inst, id, "Rolling back", () => api.rollback(inst, id, to), (r) =>
      `Rolled ${name} back to ${r.to}.`,
    );
    if (r && inst === this.current) {
      delete this.updatedNow[id];
      const s = this.statuses[id];
      // Back on the newest release: nothing is being held back.
      if (s) this.statuses[id] = { ...s, kind: s.release?.version === r.to ? "up_to_date" : "skipped" };
      await this.load();
    }
  }

  async remove(a: { id: string; name: string }, inst = this.current) {
    if (!inst) return;
    const ok = await this.run(inst, a.id, "Removing", () => api.remove(inst, a.id).then(() => true), () =>
      `Removed ${a.name}.`,
      () => () => this.restore(a.id, a.name, inst),
    );
    if (ok && inst === this.current) {
      delete this.statuses[a.id];
      await this.load();
    }
  }

  /** Put a removed addon back (undo of remove). */
  async restore(id: string, name: string, inst = this.current) {
    if (!inst) return;
    this.jobs[id] = { title: name };
    const r = await this.run(inst, id, "Restoring", () => api.restore(inst, id), (e) => `Restored ${e.name}.`);
    delete this.jobs[id];
    if (r && inst === this.current) await this.load();
  }

  /** Install by spec (source:id, URL or name). `title` puts it in the
   *  activity tray (used when there is no row to show progress on). */
  async install(spec: string, adopt: Addon | null, anyway: boolean, title?: string, onError?: (message: string) => void): Promise<boolean> {
    const inst = this.current;
    if (!inst) return false;
    const key = adopt?.id ?? spec;
    const jobs = this.#jobsOf(inst);
    if (jobs.busy[key]) return false;
    // Folders already in AddOns that this install replaces were the
    // player's: Undo puts them back. Only a brand-new addon is removed.
    const before = this.addons;
    const replaced = (folders: string[]) =>
      before.some((x) => x.folders.some((f) => folders.some((g) => g.toLowerCase() === f.toLowerCase())));
    jobs.progress[key] = 0;
    if (title) this.jobs[key] = { title };
    const r = await this.run(inst, key, "Installing", () => api.install(inst, spec, adopt?.id ?? null, anyway), (e) =>
      `Installed ${e.name} ${e.version}.`,
      (e) => () => (replaced(e.folders) ? this.rollback(e.id, e.name, null, inst) : this.remove(e, inst)),
      onError,
    );
    delete this.jobs[key];
    if (r && inst === this.current) {
      this.statuses[r.id] = { id: r.id, kind: "up_to_date", release: null, message: null };
      await this.load();
    }
    return !!r;
  }
}

export const store = new Store();
