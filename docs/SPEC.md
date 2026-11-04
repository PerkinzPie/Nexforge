# Nexforge — Specification

Version 0.1 · 2026-11-04 · Owner: Scott (Nexabyte Entertainment)

## 1. Outcome

A small, fast desktop app that opens instantly showing the player's installed addons, tells them what has updates, lets them browse and install from four hosts, and never does anything to their addons folder they didn't click. No account, no telemetry, open source, state in plain files.

Built because the existing managers are variously ad-supported, account-gated, closed, Electron-heavy, or quietly change the user's addons.

## 2. Scope

**v1 does:** browse/search, install, update, remove, pin, skip a version, roll back, adopt an existing AddOns folder (reconcile), manage two game installations (WoW Forever; Classic Era/Anniversary), check for updates on launch (toggleable), install from a pasted URL.

**v1 does not:** WeakAuras/Plater string updates, profile/WTF backup, addon settings editing, tray mode, auto-start, Retail/Midnight support (keep the enum open for it), Linux/macOS packaging (core must compile there).

## 3. Architecture

```
┌──────────────────────────────────────────────────────────┐
│ app/ (Tauri 2)                                           │
│   Svelte UI  ◄── IPC commands/events ──►  Rust commands  │
└─────────────────────────────┬────────────────────────────┘
                              │ uses
┌─────────────────────────────▼────────────────────────────┐
│ crates/nexforge-core                                     │
│  installations  ─ detect + manage game installs           │
│  toc            ─ parse .toc, flavour suffixes            │
│  reconcile      ─ map AddOns folders → known addons       │
│  state          ─ addons.toml / config.toml (serde+toml)  │
│  catalogue      ─ SQLite index for Browse (rusqlite)      │
│  sources/       ─ Source trait + github, wowi, wago, cf   │
│  install        ─ download → verify → stage → swap → keep │
│  net            ─ one reqwest client, allowlist, logging  │
│  keys           ─ keyring wrapper                          │
└──────────────────────────────────────────────────────────┘
          ▲
          │ uses
   crates/nexforge-cli  (nexforge list|check|update|install|remove|pin|rollback|reconcile)
```

### 3.1 Flavour

```rust
pub enum Flavour { Forever, ClassicEra, Anniversary, Retail /* reserved */ }
```

Detection: scan the WoW root (`World of Warcraft/`) for `_<name>_/` folders containing `Wow.exe` (or `World of Warcraft.app`). Map folder → flavour: `_forever_` → Forever; `_classic_era_` → ClassicEra; `_anniversary_` → Anniversary; `_retail_` → Retail. Prefer `.flavor.info` in that folder when present (Blizzard writes a `product` line); fall back to the folder name. The user can also point at a folder manually.

Each installation = `{ id, flavour, path, addons_dir = path/Interface/AddOns, state_file = path/Interface/AddOns/addons.toml }`.

### 3.2 TOC parsing

Read every `*.toc` in an addon folder. Prefer the flavour-specific file for the current installation:

| Flavour | TOC suffixes tried, in order |
|---|---|
| Forever | `_Forever.toc`, `_Camelot.toc`, `.toc` |
| ClassicEra | `_Vanilla.toc`, `_Classic.toc`, `.toc` |
| Anniversary | `_TBC.toc`, `_BCC.toc`, `_Vanilla.toc`, `.toc` |

Fields we read: `## Interface`, `## Title`, `## Version`, `## Author`, `## Notes`, `## X-Curse-Project-ID`, `## X-Wago-ID`, `## X-WoWI-ID`, `## X-Website`, `## X-GitHub` (rare), `## Dependencies`/`## RequiredDeps`. Strip WoW colour codes (`|cXXXXXXXX…|r`) from Title. `## Interface: 16001` (and any 16xxx) = Forever; 11xxx = Classic Era; 2xxxx = TBC-era; 5xxxx = Mists-era; 12xxxx = Retail.

### 3.3 Reconcile (adopting an existing AddOns folder)

1. List every folder in `AddOns/` (ignore `Blizzard_*`, ignore files).
2. Parse TOCs. Group folders that share the same `X-Curse-Project-ID` / `X-Wago-ID` / `X-WoWI-ID`, or that are listed as dependencies of a folder with an ID, into one addon.
3. For each group, pick a source in priority order: the ID a source can confirm (Wago `_match` can confirm by folder fingerprint; CurseForge by project ID; WoWInterface by ID; GitHub by `X-Website`/`X-GitHub` pointing at a repo). Preference when several confirm: Wago > GitHub > WoWInterface > CurseForge, because keys and author opt-outs make CurseForge the least reliable.
4. Unmatched groups are shown in the UI as "found in AddOns, not managed yet" with an "Identify source" action (search by folder name, or paste a URL).
5. Write results to `addons.toml`. Reconcile never deletes or moves anything.

### 3.4 State files

`addons.toml` (one per installation, next to the addons):

```toml
schema = 1
flavour = "forever"

[[addon]]
id = "questie"                       # slug we assign, stable
name = "Questie"
source = "github"                    # github | wowinterface | wago | curseforge
source_id = "Questie/Questie"        # repo, numeric id, wago id, cf project id
installed_version = "10.4.2"
installed_at = "2026-11-04T21:14:08Z"
folders = ["Questie", "Questie-Core", "Questie-Lib", "Questie-Loc"]
pinned = false
skipped_version = ""                 # version the user chose to skip
channel = "stable"                   # stable | beta | alpha
```

`config.toml` (in the app data dir):

```toml
schema = 1
check_on_launch = true
auto_apply = false
keep_previous_versions = 3
self_update_check = false
[[installation]]
id = "forever"
flavour = "forever"
path = 'C:\Games\World of Warcraft\_forever_'
[sources]
github = true
wowinterface = true
wago = true        # key in credential store under "nexforge/wago"
curseforge = false # enabled when a key is saved under "nexforge/curseforge"
```

Previous versions: `<app data>/backups/<installation>/<addon id>/<version>.zip` (the original downloaded archive, so rollback is a re-extract). Keep `keep_previous_versions` most recent.

### 3.5 Source trait

```rust
#[async_trait]
pub trait Source: Send + Sync {
    fn id(&self) -> SourceId;
    fn needs_key(&self) -> bool;
    async fn search(&self, q: &str, flavour: Flavour) -> Result<Vec<CatalogueEntry>>;
    async fn resolve(&self, source_id: &str) -> Result<AddonInfo>;            // name, authors, icon, summary, url
    async fn releases(&self, source_id: &str, flavour: Flavour) -> Result<Vec<Release>>; // newest first, flavour-filtered
    async fn resolve_url(&self, url: &str) -> Result<Option<String>>;        // "install from URL" → source_id
    async fn batch_latest(&self, ids: &[String], flavour: Flavour) -> Result<HashMap<String, Release>>; // for update checks
}

pub struct Release { version: String, download_url: String, published_at: DateTime, size: Option<u64>, channel: Channel, flavours: Vec<Flavour>, changelog: Option<String>, sha256: Option<String> }
```

Each source maps its own flavour labels to `Flavour` exactly as in `docs/API-NOTES.md`. If a source lists no release for the installation's flavour, `releases()` returns an empty vec and the UI shows "No Forever build listed" with an "Install anyway" option that picks the newest release of any flavour.

### 3.6 Update check

Concurrent across sources, bounded (4 per host), with ETag/If-Modified-Since where the host supports it. Results are written to the catalogue cache and emitted to the UI as they arrive. A failed source is reported per-source, never as a global failure. Rate-limit awareness: GitHub unauthenticated is 60/hour, so batch (one call per repo, cached 10 minutes) and surface "GitHub rate limit, try again at HH:MM" rather than failing silently; allow an optional personal access token in the credential store.

### 3.7 Install / update engine

1. Download to `<app data>/tmp/<uuid>.zip` with progress events.
2. Verify sha256 if the host gave one; always check the archive opens and contains at least one `*.toc` at depth 1.
3. Extract to `<app data>/staging/<uuid>/`.
4. Compute the folder set the archive owns (top-level dirs).
5. Move the current folders for this addon to `backups/.../<old version>/` (as the original zip if we have it; otherwise zip them now).
6. Move staged folders into `AddOns/` with a rename per folder. On any failure, restore from the backup and report.
7. Update `addons.toml`, emit done.

Never extract directly into `AddOns/`. Never overwrite a folder that belongs to a different addon in `addons.toml` without asking.

### 3.8 Catalogue (Browse)

SQLite at `<app data>/catalogue.db`, FTS5 table over `(source, source_id, name, authors, summary, flavours, downloads, updated_at, icon_url)`. Refreshed in the background: WoWInterface `filelist.json` (one call), Wago `popular` + search results as they happen, CurseForge search as it happens, GitHub only by known repos (there is no GitHub-wide addon index; seed from a bundled list of well-known addon repos). Search queries hit SQLite only; the network never blocks typing. Delete-safe.

### 3.9 Network layer

One `reqwest::Client`, rustls, 20s timeout, User-Agent `Nexforge/<version> (+https://nexabyte.uk)`. A host allowlist is enforced in code; a request to any other host returns an error and is logged. Every request appends a line to `<app data>/network.log`: `time host path status bytes reason`. The Settings screen opens this file.

## 4. UI

Single window, 1280×820 default, min 960×640. Left sidebar (indigo), content on the right. Four views. Keyboard: `Ctrl+F` focuses the filter/search, `Ctrl+R` checks for updates, `Ctrl+,` opens settings.

Every screen below has a mockup in `docs/mockups/`. Build to the mockup; they are the signed-off design.

### 4.1 Sidebar
Crest + "NEXFORGE" wordmark (Jost). Installation switcher card (flavour tile, name, "14 addons · 3 updates"). Nav: Installed (count), Updates (coral badge), Browse, Settings. Footer: last-checked with a status dot, "No account, no telemetry. Network log" link, Nexabyte mark and "NEXABYTE ENTERTAINMENT".

### 4.2 Installed (`Main`)
Header: title, "addons.toml is in sync" line, Filter input, Check now, Update all (n).
Summary strip: Installed, Updates available (coral), Pinned (indigo), Need attention (gold), Last update run.
Table: icon tile (host icon; monogram fallback), name + "n folders · by author", version (`old → new`, with inline progress bar while downloading), source (coloured dot + name), status cell (Update button / Up to date / Pinned / "No Forever build" / "Identify source"), row menu (Pin, Skip this version, Change source, Open folder, Open website, Remove).

### 4.3 Updates
Header: title, "n available · nothing is applied until you click", Check now, Update all.
One card per update: icon, name, `old → new`, source, released date + size, release notes (first ~3 bullets), Skip, Update. A card in progress shows a coral progress bar and percentage. Below: "Recently updated · one click to roll back" list with the kept version tag and a Roll back button.

### 4.4 Browse
Header with "Local index of 4 sources · n addons · refreshed x ago", Install from URL.
Big search input. Source chips (All, GitHub, Wago, WoWInterface, CurseForge) + "Forever builds only" checkbox (on by default). Results: icon, name, author, source dot + "also on …", summary, updated/version/downloads, Install / Installed / "Install anyway" when only another flavour exists. Below: "Popular for Forever this week" card row (from Wago popular + WoWInterface top downloads).

### 4.5 Settings
Game installations (flavour tile, path, Detected pill, Change; Add installation). Updates (check on launch ON, auto-apply OFF, keep previous versions = 3). Sources (GitHub, WoWInterface enabled; Wago key saved/Replace; CurseForge No key/Add key, with the opt-out caveat). Privacy and diagnostics (self-update check OFF, Open log, Open data folder). About panel on indigo with the Nexabyte wordmark.

## 5. Phased plan

**Phase 1 — core + CLI (no network keys needed).** Installations detection; TOC parser with tests against real TOCs from the author's AddOns folder; `addons.toml` read/write; `Source` trait; GitHub and WoWInterface sources with recorded fixtures; reconcile; install engine with backup/rollback; `nexforge-cli` covering list/check/update/install/remove/pin/rollback/reconcile. Acceptance: run `nexforge reconcile` against `D:\…\_forever_\Interface\AddOns`, get a sensible `addons.toml`, update one GitHub addon, roll it back.

**Phase 2 — Wago and CurseForge.** Key storage via keyring; both sources; "Install anyway" cross-flavour path; catalogue DB + FTS; network log.

**Phase 3 — Tauri app.** Svelte UI to the mockups; IPC commands mirroring the CLI; progress events; keyboard shortcuts; packaging (MSI via Tauri bundler) with the crest as app icon.

**Phase 4 — polish.** Install from URL for all four hosts; "Popular this week"; host icons with disk cache; self-update check (opt-in); first-run flow (detect installs → reconcile → done).

## 6. Open questions to resolve on launch night (2026-11-04, 23:00 UK)

- Confirm the Forever install folder is `_forever_` on a real install (WowUp and CurseForge assume it; Blizzard used `_classic_beta_` in beta).
- Confirm `.flavor.info` contents for Forever.
- Confirm `## Interface: 16001` on launch-day addon releases and whether CurseForge's `1.60.1` game version label changed.
