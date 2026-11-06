# Nexforge

A fast World of Warcraft addon manager for **WoW Forever**, **Classic Era** and **Anniversary**. Install, update, pin, roll back and remove addons from GitHub releases, WoWInterface and CurseForge.

- No account, no telemetry, no ads, no tray process, no auto-start.
- Talks only to the addon hosts, and logs every request it makes.
- Nothing is updated until you click (unless you turn on automatic updates). Pinned addons are never touched.
- Never touches `WTF`, `SavedVariables` or any folder it did not install or adopt. Every update keeps the previous version, so rollback is one click.
- State is plain files you can read: `addons.toml` next to your addons, `config.toml` for settings. API keys live in Windows Credential Manager.

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
