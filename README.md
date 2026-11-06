# Nexforge

An addon manager for World of Warcraft: **WoW Forever**, **Classic Era** and **Anniversary**. It installs and updates addons from GitHub releases, WoWInterface and CurseForge.

## Features

- Automatically detects your installed addons.
- Search GitHub, WoWInterface and CurseForge at once.
- Install, update and remove addons in one click.
- Update one addon or all of them, with release notes.
- Roll back any addon to an earlier version.
- Pin an addon to its current version, or skip a release.
- Manage Forever, Classic Era and Anniversary installs side by side.

## Privacy and safety

- No account, telemetry, ads, tray icon or auto-start.
- Only updates when you click, unless you turn on automatic updates. Pinned addons are never updated.
- Never touches `WTF`, `SavedVariables` or any folder it didn't install or adopt.
- Only connects to the addon hosts, and logs every request it makes.
- Stores your addon list in a plain `addons.toml` next to your addons, and settings in `config.toml`. API keys are kept in Windows Credential Manager.

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
