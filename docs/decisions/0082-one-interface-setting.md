# ADR 0082: One Interface setting, switched live

Date: 2026-11-06. Status: accepted. Replaces the per-screen options of ADR 0065 (starmap HUD), ADR 0067 (in-game
menu), ADR 0070 (colony), ADR 0071 (research and production), ADR 0072 (info), ADR 0073 §5 (cursors), ADR 0075
(design), ADR 0076 (messages, diplomacy, spy, trade), ADR 0077 (battle HUD, with the starmap's) and ADR 0078 (main
menu, help, new game and end-of-game pages); their designs stand. Implemented by `ig2-client::display`
(`INTERFACE`, `modern`, `Display::modern`), `ig2-client::window::menus` (`App::apply_display`) and the
`*_modern_up` gates in `ig2-client::window::modern`.

## Context
Each modern screen came with its own Remake row, twelve in all. Mixed combinations (a modern starmap bar over a
classic colony, a modern menu over classic cursors) were never designed, and changing a row during a game did not
reliably show the chosen look: the main menu read its row once when the front end was created, an open in-game
menu kept its kind until it was closed.

## Decision
1. **One row.** "Interface" (Options, Interface; `Interface` in our settings file): Classic (0, the original's
   screens) or Modern (1, the default) for every screen at once: starmap HUD and production, colony, research,
   info, messages, diplomacy, spy, trade, design, the battle HUD, the in-game menu and help, the main menu with its
   new game and end-of-game pages, and the cursors. The UI theme (ADR 0056) and the UI scale (ADR 0062) stay their
   own rows.
2. **Old settings files.** Without `Interface` the file's old per-screen keys (`ModernStarmapHud`,
   `ModernGameMenu`, `ModernColony`, `ModernInfo`, `ModernMessages`, `ModernCursors`, `ModernResearch`,
   `ModernMainMenu`, `ModernDesign`, `ModernDiplomacy`, `ModernSpy`, `ModernTrade`) decide: Modern if any of them
   is on or unset (they were on by default), Classic only if all were off. The first stored change writes
   `Interface`; the old keys are left in the file and no longer read once it is there.
3. **Live both ways.** A change applies when the settings are stored (`App::apply_display`, every change on the
   classic screen, Apply on the modern page): the per-screen gates read the one flag every frame, the main menu's
   own flag (and the grouped menu it brings, ADR 0059) is updated, an open in-game menu turns into the other
   interface's at once (the modern page opens on its Display tab, the classic screen on its Remake page), the
   cursors switch. The main menu's Settings page (ADR 0031) stays the modern page in both, as before.
4. **Snapshots.** `--modern` draws the modern interface in `--snapshot` runs; the old `--modern-<screen>` flags
   are kept as its aliases. Input scripts compared with the exe still run the classic screens.

## Consequences
One switch to test and to describe; a player can no longer keep a single classic screen inside the modern
interface. Enhancement rows of the separate options now name the Interface setting.
