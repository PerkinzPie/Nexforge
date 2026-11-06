# Nexforge

A fast, quiet addon manager for World of Warcraft: **WoW Forever**, **Classic Era** and **Anniversary**. Addons come from GitHub releases, WoWInterface and CurseForge.

## Features

- **Picks up what you already have.** Your existing addons are recognised and matched to where they came from; nothing is moved or reinstalled.
- **One search across every host.** Results are ranked by name, with screenshots, descriptions and the right build for your client.
- **Updates you can read first.** See each update's release notes, then update one addon or all of them.
- **One-click rollback.** Every update keeps the version before it, so a broken release is one click from gone.
- **Pin or skip.** Hold an addon at its current version, or skip one release and take the next.
- **Every client in one place.** Forever, Classic Era and Anniversary installs side by side, each with its own addon list.
- **Safe if something goes wrong.** An update cut short by a crash or power cut is put back the next time Nexforge opens.

## What it won't do

- **No account, telemetry, ads, tray icon or auto-start.**
- **No surprise updates.** Nothing changes until you click, unless you turn on automatic updates. Pinned addons are never touched.
- **No wandering.** It never touches `WTF`, `SavedVariables` or any folder it didn't install or adopt.
- **No hidden traffic.** It talks only to the addon hosts, and logs every request it makes.
- **No lock-in.** Your addon list is a plain `addons.toml` next to your addons, settings are `config.toml`, and API keys stay in Windows Credential Manager.

## Install

Download the `.msi` from the [latest release](https://github.com/PerkinzPie/Nexforge/releases/latest). Windows 10 or 11.

On first launch Nexforge finds your game clients and adopts their AddOns folders: it writes an `addons.toml` and moves nothing.

## Sources

| Host | Key | Notes |
|---|---|---|
| GitHub releases | none (optional token raises the rate limit) | Reads `release.json` for per-client builds. A bundled index maps popular addons to their repos. |
| WoWInterface | none | |
| CurseForge | yours, free from console.curseforge.com | Authors who block third-party downloads are shown as unavailable; Nexforge never works around that. |
| Wago | not supported | Wago issues API keys to paying subscribers only. |

## Build

Requires Rust (stable), Node 22 and pnpm.

```
cargo test --workspace          # core, CLI and app tests (offline)
cd app
pnpm install
pnpm tauri dev                  # run the app
pnpm tauri build                # release build and .msi
```

The `nexforge` command-line tool (`crates/nexforge-cli`) does everything the app does: `detect`, `reconcile`, `search`, `check`, `update`, `install`, `remove`, `pin`, `rollback` and more.

## Releases

What changed in each version is in [CHANGELOG.md](CHANGELOG.md). Releases are made with [Knope](https://knope.tech):

1. With a change players would notice, add a change file: run `knope document-change`, or add `.changeset/<name>.md` by hand:

   ```markdown
   ---
   default: minor
   ---

   Search finds names with symbols in them.
   ```

   `major` is for a change in how something works, `minor` for something new, `patch` for a fix. While Nexforge is 0.x, `major` bumps the middle number and the others bump the last.
2. Each push to `master` with change files opens or refreshes a **Release x.y.z** pull request. It bumps the version in `Cargo.toml`, `Cargo.lock`, `app/package.json` and `tauri.conf.json`, and adds the new section to `CHANGELOG.md`.
3. Merge that pull request to build the `.msi`, tag `vx.y.z` and publish the GitHub release, with the changelog section as its notes.

## Licence

MIT. Not affiliated with Blizzard Entertainment. World of Warcraft is a trademark of Blizzard Entertainment.
