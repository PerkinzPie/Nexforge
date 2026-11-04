# Nexforge brand and UI tokens

Derived from the Nexabyte design system (`Nexabyte-Tools/Herald/App.xaml`, `Shelly/App.xaml`) and tuned for dark surfaces. All text colours below clear WCAG 4.5:1 on the surfaces they are used on.

## Name and mark

- Product: **Nexforge**. Wordmark set in Jost 600, uppercase, 4% tracking.
- Mark: coral hexagon (two facets, light from above), off-white anvil with a lit top face and an offset shadow, gold ember and two sparks. Source: `assets/nexforge-crest.svg` (48-unit viewBox; the indigo background rect is for the app icon; drop it when placing on a surface).
- Credit: "A Nexabyte Entertainment product." Nexabyte wordmark and hex-cluster mark: `Nexabyte-Tools/Shelly/Resources/nexabyte-logo.svg`.

## Colours

```css
:root {
  /* Nexabyte brand */
  --nx-indigo:        #2C2A4E;  /* sidebar, brand panels */
  --nx-coral:         #D74653;  /* mark, large fills, progress bars */
  --nx-plum:          #653350;  /* second flavour tile */

  /* surfaces */
  --bg:               #12111C;  /* window */
  --surface:          #1A1927;  /* cards, tables */
  --raised:           #22213A;  /* buttons, inputs, chips */
  --border:           #2E2D45;
  --divider:          #26253B;

  /* text */
  --text:             #F2F2F4;  /* 15.5:1 on surface */
  --text-2:           #A8A8B0;  /* 7.3:1 */
  --text-3:           #8A8A94;  /* 5.1:1 — lifted from brand #707078, which fails at small sizes */
  --text-on-indigo-2: #A8A8B0;  /* 5.7:1 on indigo */

  /* coral for UI (brand coral fails 4.5:1 at small sizes) */
  --accent-fill:      #E35A66;  /* button background; use --bg text on it (5.3:1) */
  --accent-fill-hover:#EB6F7A;
  --accent-text:      #EB6F7A;  /* coral text on surface (5.9:1) */
  --accent-tint:      #3A2230;  /* coral icon tile / chip background */

  /* status (differ in lightness, not just hue) */
  --good:             #4FD1B5;  --good-tint:  #1F3A36;   /* up to date · Wago */
  --attention:        #F5C451;  --attn-tint:  #3A3320;   /* no build for flavour · WoWInterface */
  --info:             #A6AAF0;  --info-tint:  #2A2C55;   /* pinned, links · GitHub */
  --act:              #EB6F7A;                            /* updates · CurseForge */
}
```

Sources are shown as neutral chips with their name (`--raised` background, `--text-soft` text); further hosts for the same addon as outlined chips. No coloured source dots (changed 2026-11-04).

## Type

| Role | Face | Size / weight |
|---|---|---|
| Page titles, wordmark, big numbers | Jost | 26px / 600, 1% tracking |
| Card titles | Manrope | 16px / 800 |
| Row titles, buttons | Manrope | 13.5px / 700 |
| Body | Manrope | 13px / 400, line-height 1.55 |
| Section labels | Manrope | 11.5px / 700, uppercase, 6% tracking, `--text-3` |
| Versions, paths | system monospace | 12.5px, `#C9C8D6` |

Both faces are on Google Fonts; bundle them with the app (OFL) rather than loading from the network.

## Shape and spacing

- Radii: 14px cards/tables, 12px icon tiles ≥44px, 9–10px buttons/inputs/tiles ≤40px, 6px tags.
- Buttons 36px tall (32px in rows), inputs 36px, search 46px.
- Table rows 60px with a 38px icon tile; sidebar 232px; content padding 28px 32px.
- No gradients, no drop shadows in the UI (the crest is the one exception). Depth comes from the four surface steps.

## Rules

1. Coral means "there is something to do". Nothing decorative is coral.
2. A source is a neutral chip with its name. Colour is kept for status, not for hosts.
3. Icon tiles show the host's real icon at runtime; a monogram on a tinted tile is the fallback. Game clients show their own icon, read from the local `Wow.exe` / `WowClassic.exe`.
4. The indigo panel is for brand surfaces only: the sidebar and the About block.
5. Paint from local state first. Network results update in place; never a full-screen spinner.
6. Copy is short: a label, not a sentence. Empty states say what is ("All up to date", "None."), not why.
