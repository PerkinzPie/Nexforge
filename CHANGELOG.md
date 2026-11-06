# Changelog

Notable changes to Nexforge, newest first. Nexforge is in 0.x: a release that changes how something works bumps the middle number, anything else bumps the last.

## 0.1.1 (2026-11-06)

### Features

- Addons you edited by hand get their own backup before an update, so rolling back brings your edits back.
- Settings can check for a new version of Nexforge on demand.
- Two Nexforge windows, or the window and the CLI, can no longer change the same game install at once.
- When one addon can't be checked, only that addon shows Check failed; the others still check and update.
- An update or removal cut short by a crash or power cut is undone the next time Nexforge opens.
- Search ranks results by how well the name matches and spells out symbols, so Forever++ finds ForeverPlusPlus.

### Fixes

- The Can't update tab is now called Issues.
- Oversized or malformed downloads, archives and images are refused, and a damaged settings or addons file is set aside instead of stopping the app.
- Screenshots in addon details are sharper.
- Rows and cards no longer move while updates run or release notes load.

## 0.1.0 (2026-11-05)

### Features

- First release. Install, update, pin, roll back and remove addons from GitHub releases, WoWInterface and CurseForge, for WoW Forever, Classic Era and Anniversary.
