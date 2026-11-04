// Sample data for reviewing the UI in a plain browser (`pnpm dev`, no Tauri).
// Never used inside the app.

import type { Addon, CheckResult, Hit, Progress, Recent, Settings, Snapshot } from "./api";

const settings: Settings = {
  checkOnLaunch: true, autoApply: false, keepPreviousVersions: 3, selfUpdateCheck: false,
  github: true, wowinterface: true, curseforge: false,
  keyGithub: false, keyCurseforge: false, keyWago: false,
  dataDir: "C:\\Users\\you\\AppData\\Local\\Nexabyte\\Nexforge",
  networkLog: "C:\\Users\\you\\AppData\\Local\\Nexabyte\\Nexforge\\network.log",
  version: "0.1.0",
};

const a = (id: string, name: string, author: string, folders: number, source: string | null, version: string, extra: Partial<Addon> = {}): Addon => ({
  id, name, author, folders: Array.from({ length: folders }, (_, i) => (i ? `${name}_${i}` : name)),
  source, sourceLabel: { github: "GitHub", wowinterface: "WoWInterface", curseforge: "CurseForge", wago: "Wago" }[source ?? ""] ?? "Unknown",
  sourceId: source ? id : "", version, pinned: false, skippedVersion: "", flavourMismatch: false,
  managed: source !== null, website: null, iconUrl: null, knownIds: [], kept: [], ...extra,
});

const addons: Addon[] = [
  a("questie", "Questie", "AeroScripts", 4, "github", "10.4.1"),
  a("dbm", "Deadly Boss Mods", "MysticalOS", 9, "github", "11.2.9"),
  a("details", "Details! Damage Meter", "Terciob", 5, "curseforge", "12.4.0"),
  a("weakauras", "WeakAuras", "WeakAuras team", 5, "github", "5.19.4", { kept: ["5.19.3", "5.19.2"] }),
  a("bagnon", "Bagnon", "Tuller", 3, "wowinterface", "11.0.5"),
  a("atlasloot", "AtlasLoot Classic", "Lag123", 6, "github", "3.4.6", { pinned: true, skippedVersion: "3.4.8" }),
  a("leatrix", "Leatrix Plus", "Leatrix", 1, "curseforge", "1.15.84"),
  a("plater", "Plater Nameplates", "Terciob", 1, "github", "2.5.3"),
  a("castbars", "ClassicCastbars", "Wardz", 2, "github", "1.8.2"),
  a("tomtom", "TomTom", "Cladhaire", 1, "curseforge", "4.0.9"),
  a("guildbank", "GuildBankTweaks", "", 1, null, "1.2"),
];

const rel = (version: string, notes: string[], size: number) => ({
  version, publishedAt: "2026-11-03T12:00:00Z", size, channel: "stable", flavourUnknown: false, notes, hasMoreNotes: notes.length > 1,
});

const check: CheckResult = {
  checkedAt: new Date(Date.now() - 2 * 60 * 1000).toISOString(),
  sourceErrors: [],
  results: [
    { id: "questie", kind: "available", release: rel("10.4.2", ["Forever: added quest data for the nine launch dungeons", "Fixed tracker overlapping the minimap at some UI scales", "Updated German and French translations"], 2_200_000), message: null },
    { id: "dbm", kind: "available", release: rel("11.2.10", ["Initial Forever support: Onyxia timers and new dungeon modules", "Removed Retail-only warning sounds from the Classic build"], 7_100_000), message: null },
    { id: "details", kind: "unavailable", release: null, message: "CurseForge needs an API key (Settings)." },
    { id: "weakauras", kind: "up_to_date", release: null, message: null },
    { id: "bagnon", kind: "up_to_date", release: null, message: null },
    { id: "atlasloot", kind: "pinned", release: rel("3.4.8", [], 1_000_000), message: null },
    { id: "leatrix", kind: "unavailable", release: null, message: "CurseForge needs an API key (Settings)." },
    { id: "plater", kind: "available", release: rel("2.5.4", ["Forever nameplate scaling"], 1_400_000), message: null },
    { id: "castbars", kind: "no_build", release: null, message: null },
    { id: "tomtom", kind: "unavailable", release: null, message: "CurseForge needs an API key (Settings)." },
    { id: "guildbank", kind: "not_managed", release: null, message: null },
  ],
};

const recent: Recent[] = [
  { id: "weakauras", name: "WeakAuras", source: "github", from: "5.19.3", to: "5.19.4", at: new Date(Date.now() - 20 * 3600 * 1000).toISOString() },
];

const hits: Hit[] = [
  { source: "wowinterface", sourceLabel: "WoWInterface", sourceId: "1", spec: "wowinterface:1", name: "Bagnon", authors: ["Tuller"], summary: "Single-window inventory and bank with search, rulesets and per-character views.", flavours: ["Forever"], forFlavour: true, flavourUnknown: false, downloads: 1_200_000, updatedAt: new Date(Date.now() - 2 * 86400e3).toISOString(), iconUrl: null, installed: true, alsoOn: ["CurseForge"] },
  { source: "github", sourceLabel: "GitHub", sourceId: "plusmouse/Baganator", spec: "github:plusmouse/Baganator", name: "Baganator", authors: ["plusmouse"], summary: "Fast single-view bags with category grouping and built-in bank and guild bank support.", flavours: [], forFlavour: false, flavourUnknown: true, downloads: null, updatedAt: null, iconUrl: null, installed: false, alsoOn: ["Wago", "CurseForge"] },
  { source: "wowinterface", sourceLabel: "WoWInterface", sourceId: "3", spec: "wowinterface:3", name: "Sorted", authors: ["Vexrin"], summary: "List-style inventory that sorts items by category, quality or level.", flavours: ["Classic Era"], forFlavour: false, flavourUnknown: false, downloads: 90_000, updatedAt: null, iconUrl: null, installed: false, alsoOn: [] },
];

const snap = (current: string | null): Snapshot => ({
  installations: [
    { id: "forever", flavour: "forever", label: "WoW Forever", short: "Forever", path: "C:\\Games\\World of Warcraft\\_forever_", detected: true },
    { id: "classic-era", flavour: "classic-era", label: "Classic Era", short: "Classic Era", path: "C:\\Games\\World of Warcraft\\_classic_era_", detected: true },
  ],
  current: current ?? "forever", flavour: "forever", flavourLabel: "WoW Forever", flavourShort: "Forever",
  addons, stateExists: true, addonsDirExists: true, adoptedNow: false,
  firstRun: typeof location !== "undefined" && location.search.includes("firstrun") && !welcomed,
  lastRun: { at: new Date(Date.now() - 20 * 3600 * 1000).toISOString(), count: 2 },
  settings, sourcesOn: ["github", "wowinterface"],
});

let listeners: ((p: Progress) => void)[] = [];
let welcomed = false;
let removed: Addon | undefined;
export function onPreviewProgress(cb: (p: Progress) => void) {
  listeners.push(cb);
  return () => { listeners = listeners.filter((l) => l !== cb); };
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function preview(cmd: string, args: Record<string, any>): Promise<unknown> {
  switch (cmd) {
    case "snapshot": return snap(args.installationId);
    case "adopt_all": {
      await wait(900);
      welcomed = true;
      const s = snap(null);
      return [
        { installation: s.installations[0], addons: 0, managed: 0 },
        { installation: s.installations[1], addons: 46, managed: 12 },
      ];
    }
    case "check": await wait(600); return { ...check, checkedAt: new Date().toISOString() };
    case "recent": return recent;
    case "rollback": {
      await wait(400);
      const ad = addons.find((x) => x.id === args.id)!;
      const to = args.to ?? ad.kept[0];
      const from = ad.version;
      ad.kept = [from, ...ad.kept.filter((k) => k !== to)];
      ad.version = to;
      return { from, to };
    }
    case "details": {
      await wait(300);
      const h = hits.find((x) => x.source === args.source && x.sourceId === args.sourceId);
      const ad = addons.find((x) => x.source === args.source && x.sourceId === args.sourceId);
      const name = h?.name ?? ad?.name ?? String(args.sourceId);
      return {
        name, authors: h?.authors ?? (ad?.author ? [ad.author] : []), iconUrl: h?.iconUrl ?? null,
        screenshots: [1, 2, 3, 4, 5].map((n) => `preview://shot/${n}`), screenshotThumbs: [1, 2, 3, 4, 5].map((n) => `preview://shot/${n}`),
        summary: h?.summary ?? null,
        about: [h?.summary ?? `${name} for World of Warcraft.`, "Sample description paragraph from the host, shown in the browser preview only."],
        url: `https://example.invalid/${args.sourceId}`,
        releases: [
          { version: "2.4.0", publishedAt: "2026-11-02T10:00:00Z", size: 2_100_000, channel: "stable", flavours: ["Forever", "Classic Era"], notes: ["Forever support", "Fixed a tooltip error"] },
          { version: "2.3.1", publishedAt: "2026-10-12T10:00:00Z", size: 2_000_000, channel: "stable", flavours: ["Classic Era"], notes: ["Bug fixes"] },
          { version: "2.3.0", publishedAt: "2026-09-30T10:00:00Z", size: 1_900_000, channel: "stable", flavours: ["Classic Era"], notes: [] },
        ],
      };
    }
    case "search": await wait(250); return { hits: hits.filter((h) => h.name.toLowerCase().includes(String(args.query).toLowerCase()) || "bag".includes(String(args.query).toLowerCase())), errors: [] };
    case "settings": return settings;
    case "save_settings": Object.assign(settings, args.s); return settings;
    case "update": {
      for (let p = 0; p <= 100; p += 10) { listeners.forEach((l) => l({ id: args.id, done: p, total: 100, percent: p })); await wait(120); }
      const addon = addons.find((x) => x.id === args.id)!;
      const r = check.results.find((x) => x.id === args.id);
      if (r?.release) { addon.version = r.release.version; r.kind = "up_to_date"; r.release = null; }
      return addon;
    }
    case "install": {
      for (let p = 0; p <= 100; p += 5) { listeners.forEach((l) => l({ id: args.adopt ?? args.spec, done: p, total: 100, percent: p })); await wait(90); }
      await wait(700);
      const name = String(args.spec).split(/[:/]/).pop() ?? "Addon";
      const added = { ...addons[0], id: name.toLowerCase(), name, version: "1.0.0", folders: [name] };
      addons.push(added);
      return added;
    }
    case "remove": { const i = addons.findIndex((x) => x.id === args.id); if (i >= 0) removed = addons.splice(i, 1)[0]; return null; }
    case "restore": { await wait(600); if (removed) addons.push(removed); return removed; }
    case "skip": return null;
    case "set_pinned": { const x = addons.find((y) => y.id === args.id); if (x) x.pinned = args.pinned; return null; }
    case "image": {
      // Sample screenshots for the browser preview: a tinted frame with its number.
      const m = /^preview:\/\/shot\/(\d+)$/.exec(String(args.url));
      if (!m) throw new Error("no images in preview");
      await wait(250);
      const hue = [210, 160, 30, 280, 350][(Number(m[1]) - 1) % 5];
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720"><rect width="1280" height="720" fill="hsl(${hue} 30% 22%)"/><rect x="80" y="80" width="420" height="260" rx="8" fill="hsl(${hue} 30% 32%)"/><text x="640" y="400" font-family="sans-serif" font-size="96" fill="hsl(${hue} 40% 75%)" text-anchor="middle">Screenshot ${m[1]}</text></svg>`;
      return `data:image/svg+xml;base64,${btoa(svg)}`;
    }
    case "self_update": return settings.selfUpdateCheck ? { version: "0.2.0", url: "https://github.com/PerkinzPie/Nexforge/releases" } : null;
    case "refresh_catalogue": await wait(300); return { total: 8264, refreshedAt: new Date().toISOString(), errors: [] };
    case "popular": return args.source ? hits.filter((h) => h.source === args.source) : hits.filter((h) => h.forFlavour).map((h) => ({ ...h }));
    default: throw new Error(`"${cmd}" is not available in the browser preview.`);
  }
}
