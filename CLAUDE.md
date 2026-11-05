# Nexforge

A fast, trustworthy World of Warcraft addon manager by Nexabyte Ltd. Browse, install, update, pin, roll back and remove addons from GitHub Releases, WoWInterface, Wago.io and CurseForge, for WoW Forever and Classic Era/Anniversary installs.

## Stack

- **Core:** Rust crate `nexforge-core` (no UI deps). Tokio, reqwest (rustls), serde, toml, rusqlite (bundled), zip, sha2, keyring. Edition 2021, MSRV = current stable.
- **App:** Tauri 2 shell `nexforge-app` over the core. Svelte 5 + TypeScript + Vite frontend; plain CSS with design tokens in `app/src/app.css` (Radix Colors Slate). Bits UI (headless) for menus, dialogs, tooltips and tabs; Lucide icons in `Icon.svelte`. No styled component kit.
- **CLI:** `nexforge-cli` over the core, for testing and scripting. Build this before the GUI.
- **Target:** Windows first (the author's machine). Keep the core cross-platform.

## Non-negotiables (the reason this project exists)

- No account, no telemetry, no analytics, no ads, no auto-start, no tray process.
- Outbound connections only to: `api.github.com`, `github.com` (release assets), `objects.githubusercontent.com` / `release-assets.githubusercontent.com` (asset download redirects), `api.mmoui.com`, `cdn.wowinterface.com` / `www.wowinterface.com` (downloads), `addons.wago.io`, `api.curseforge.com` plus CurseForge's CDN for downloads. Icons and screenshots only (image responses, via the separate image fetch): `cdn-wow.mmoui.com`, `avatars.githubusercontent.com`, `cdn.wago.io`, `media.forgecdn.net`. Anything else is a bug. Log every request (host, path, reason) to a local text file.
- Updates are listed, never applied, until the user clicks, unless they turn on auto-apply. Pinned addons are never auto-updated.
- Never touch `WTF/`, `SavedVariables`, or any folder we did not install or adopt.
- Keep the previous version of every addon we update so rollback is one click.
- State is plain files the user can read: `addons.toml` per installation, `config.toml` for settings. SQLite is a cache only and must be safe to delete.
- API keys go in the OS credential store (Windows Credential Manager via `keyring`), never in a file.
- UI paints from local state before any network call. No startup spinner.

## Conventions

- Rust: `cargo fmt`, `cargo clippy -D warnings`, unit tests next to code, integration tests in `tests/` using recorded HTTP fixtures (no live network in CI).
- One `Source` trait, four implementations. A new host should be one new file.
- Flavour is an enum; every source maps its own labels to it in one place.
- Errors: `thiserror` in the core, surfaced to the UI as plain sentences a player can act on.
- Commit messages: imperative, one line, body only when needed.

## Repo layout

```
nexforge/
  Cargo.toml            # workspace
  crates/
    nexforge-core/
    nexforge-cli/
  app/                  # Tauri 2 shell + Svelte UI
  assets/               # app icon: nexforge-icon.svg, 1024 PNG (source for `tauri icon`), hand-made .ico
  .github/workflows/     # CI on every push; release on v* tags
```

## Status

Built and published at https://github.com/PerkinzPie/Nexforge (MIT). Sources: GitHub releases (with the bundled index `crates/nexforge-core/data/github-index.toml`; add entries only after the check in its header), WoWInterface, and CurseForge (on only with a key; its tests use synthetic fixtures). CurseForge was verified live on 2026-11-04: installs by Project ID, the batch update check, featured lists (which fill its catalogue), and Forever's `gameVersionTypeId` 88568; search with a self-serve console key answered 403 intermittently (a key issued by CurseForge on 2026-11-05 searched reliably), so a refusal pauses search for a minute. Authors who disallow third-party distribution cannot be installed. Wago is not supported (paid key). Pushing a `v*` tag builds the MSI and publishes a release, which the opt-in self-update check reads. Not yet verified live: Forever addons after launch. The design docs and mockups were removed once built; they remain in git history before the commit that removed them.
