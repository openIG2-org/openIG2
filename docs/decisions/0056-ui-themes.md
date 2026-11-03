# ADR 0056: UI themes

Date: 2026-11-01. Status: accepted; amended by ADR 0073 (the built-in Modern theme's colours). Implemented by `ig2-client::theme` (the theme files, the current theme) and
the remake's own screens that draw with it (`options::modern`, `stats_screen`, `replay_viewer`, `mp_lobby`,
`event_log`); the option is a Remake row (ADR 0048) on the Interface tab (ADR 0053).

## Context
The decision of 2026-11-01 (`docs/ENHANCEMENTS.md` → Decisions): a themeable UI, after the main-menu Options
screen, whose style is the first theme. The remake's own screens (the Options screen of ADR 0053, the history and
statistics screen, the replay list and bar, the lobby, the event log) are drawn with our own primitives — the kit's
faders (rounded boxes), filled quads and the game's font — each with its own colour constants. None of them has an
original to match, so their look can change freely; the game's own screens must keep matching the exe.

## Decision
1. **A theme** is a set of colours (panel fill and edge, tabs, buttons, text, dimmed text, disabled text, accent,
   hover, the switch and slider tracks and knob), the frame style (the faders' transparencies of panel and button
   fill and edge) and the font scale (the game font's sizes for titles and for everything else). It is drawn
   only with our primitives; no picture, model or other art is shipped or needed.
2. **Classic** (the default) is no theme: every screen draws exactly as before, the game's own screens always do.
   The main-menu Options screen's Classic look takes the colours the remake's other menu screens already use (the
   game's palette for text, the history screen's boxes).
3. **Modern** is the built-in theme: the Options screen's look as shipped in ADR 0053, kept as a theme file in the
   source (`engine/crates/ig2-client/src/themes/modern.toml`) and read through the same parser as a mod's.
4. **Mod themes**: a folder in `mods/` (ADR 0044) with a `theme.toml` offers its theme under the folder's name,
   whether or not the mod is enabled in `enabled.txt` (choosing the theme is the enabling; a theme changes no rule
   and stays outside the multiplayer fingerprint). Folders are listed in name order after Classic and Modern.
5. **The file** is TOML: every key optional, starting from Modern's values; colours are `[r, g, b]` in 0..255,
   transparencies 0..255 (the faders' own unit), `font_title` and `font_text` the font's size indices (titles 0..4,
   other text 0..2: the screens' columns are laid out for size 2; the layout itself is not themed). An unknown key, a
   wrong type or a value out of range is an error naming the key; a theme that doesn't parse isn't offered, and
   choosing it anyway (a stored name) gives Classic with the error in the log.
6. **The option**: "UI theme" (`UiTheme`) on the Interface tab and the in-game Remake page: Classic / Modern / the
   mods' themes, stored by name (as the resolution is stored by its label); a stored name no longer offered reads as
   Classic. Apply restyles the screens at once.
7. **Where it applies**: the remake's own screens of the Context. The game's 2D panels (top bar, info boxes) and the
   quick game's panel (the exe's scenario panel) keep the original's art and colours: tinting them would change
   accepted capture comparisons and is left for a later step behind the same option.

## Consequences
- A screen of ours added later reads `theme::current()` (or `theme::look`) instead of its own constants.
- Classic captures and the exe comparisons are unchanged; the remake's own screens have no exe capture.
