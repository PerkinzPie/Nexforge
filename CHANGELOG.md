# Changelog

Notable changes to Nexforge, newest first. Nexforge is in 0.x: a release that changes how something works bumps the middle number, anything else bumps the last.

## 0.1.2 (2026-11-10)

### Features

- Apply updates automatically now works after every check and on every game client. Game clients can be removed in Settings. Back from an addon's page returns to where you were. Install from URL stays open until the link is found. Refresh asks WoWInterface again instead of reusing the morning's list. A removed addon keeps only its last version. The Update button shows a bar and a percentage while it works.

### Fixes

- Shorter, clearer messages and tooltips.
- New Nexforge icon and colours.
- Fixed: Undo and Update all could act on the wrong game client; installing a version you already had replaced the kept copy of your own files; "All up to date" showed after a failed check; a failed update now stays marked with Retry; a skipped version can be un-skipped; the welcome screen no longer gets stuck when no client is found.
- Hand-installed addons have an Identify button; "Needs key" opens Settings and a saved key is used straight away; a pinned hand-installed addon is never replaced by another addon's files; WoWInterface addons made only for other game versions are no longer offered as compatible; older dates show their year; Check for updates in Settings always asks.
- Hover Up to date to see when an addon was installed or last updated.
- Update all is disabled while updates are running.

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
