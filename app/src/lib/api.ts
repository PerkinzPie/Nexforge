// Typed bridge to the Rust commands in src-tauri/src/lib.rs.
// Outside Tauri (plain browser, for design review) it serves sample data from
// ./preview.ts and the UI shows a "Browser preview" banner.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Installation { id: string; flavour: string; label: string; short: string; path: string; detected: boolean }
export interface Addon {
  id: string; name: string; author: string | null; folders: string[];
  source: string | null; sourceLabel: string; sourceId: string; version: string;
  pinned: boolean; skippedVersion: string; flavourMismatch: boolean; managed: boolean;
  website: string | null; iconUrl: string | null; knownIds: [string, string][];
}
export interface Settings {
  checkOnLaunch: boolean; autoApply: boolean; keepPreviousVersions: number; selfUpdateCheck: boolean;
  github: boolean; wowinterface: boolean; curseforge: boolean;
  keyGithub: boolean; keyCurseforge: boolean; keyWago: boolean;
  dataDir: string; networkLog: string; version: string;
}
export interface Snapshot {
  installations: Installation[]; current: string | null;
  flavour: string; flavourLabel: string; flavourShort: string;
  addons: Addon[]; stateExists: boolean; addonsDirExists: boolean; adoptedNow: boolean;
  lastRun: { at: string; count: number } | null;
  settings: Settings; sourcesOn: string[];
}
export interface ReleaseInfo {
  version: string; publishedAt: string | null; size: number | null; channel: string;
  flavourUnknown: boolean; notes: string[]; hasMoreNotes: boolean;
}
export type StatusKind = "up_to_date" | "available" | "pinned" | "skipped" | "no_build" | "not_managed" | "unavailable" | "failed";
export interface AddonStatus { id: string; kind: StatusKind; release: ReleaseInfo | null; message: string | null }
export interface SourceError { source: string; sourceLabel: string; message: string }
export interface CheckResult { checkedAt: string; results: AddonStatus[]; sourceErrors: SourceError[] }
export interface Progress { id: string; done: number; total: number | null; percent: number | null }
export interface Recent { id: string; name: string; source: string | null; from: string; to: string; at: string }
export interface Hit {
  source: string; sourceLabel: string; sourceId: string; spec: string; name: string; authors: string[];
  summary: string | null; flavours: string[]; forFlavour: boolean; flavourUnknown: boolean;
  downloads: number | null; updatedAt: string | null; iconUrl: string | null; installed: boolean; alsoOn: string[];
}
export interface SearchResult { hits: Hit[]; errors: SourceError[] }
export type SettingsInput = Pick<Settings, "checkOnLaunch" | "autoApply" | "keepPreviousVersions" | "selfUpdateCheck" | "github" | "wowinterface" | "curseforge">;

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) {
    const { preview } = await import("./preview");
    return preview(cmd, args ?? {}) as Promise<T>;
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    // Commands reject with a plain sentence from the core.
    throw new Error(typeof e === "string" ? e : String((e as Error)?.message ?? e));
  }
}

export const api = {
  snapshot: (installationId: string | null) => call<Snapshot>("snapshot", { installationId }),
  reconcile: (installationId: string) => call<number>("reconcile_now", { installationId }),
  check: (installationId: string) => call<CheckResult>("check", { installationId }),
  update: (installationId: string, id: string) => call<Addon | null>("update", { installationId, id }),
  install: (installationId: string, spec: string, adopt: string | null, anyway: boolean) =>
    call<Addon>("install", { installationId, spec, adopt, anyway }),
  remove: (installationId: string, id: string) => call<void>("remove", { installationId, id }),
  restore: (installationId: string, id: string) => call<Addon>("restore", { installationId, id }),
  setPinned: (installationId: string, id: string, pinned: boolean) => call<void>("set_pinned", { installationId, id, pinned }),
  skip: (installationId: string, id: string, version: string) => call<void>("skip", { installationId, id, version }),
  rollback: (installationId: string, id: string) => call<{ from: string; to: string }>("rollback", { installationId, id }),
  recent: (installationId: string) => call<Recent[]>("recent", { installationId }),
  search: (installationId: string | null, query: string) => call<SearchResult>("search", { installationId, query }),
  image: (url: string) => call<string>("image", { url }),
  clientIcon: (installationId: string) => call<string | null>("client_icon", { installationId }),
  settings: () => call<Settings>("settings"),
  saveSettings: (s: SettingsInput) => call<Settings>("save_settings", { s }),
  setKey: (name: string, value: string) => call<Settings>("set_key", { name, value }),
  clearKey: (name: string) => call<Settings>("clear_key", { name }),
  open: (what: "log" | "data" | "addons" | "addon", installationId?: string, id?: string) =>
    call<void>("open", { what, installationId, id }),
  openUrl: (url: string) => call<void>("open_url", { url }),
  addInstallation: (replace: string | null) => call<Installation | null>("add_installation", { replace }),
};

export async function onProgress(cb: (p: Progress) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    const { onPreviewProgress } = await import("./preview");
    return onPreviewProgress(cb);
  }
  return listen<Progress>("progress", (e) => cb(e.payload));
}
