# ADR 0031: Settings from the main menu, and a language row

Date: 2026-11-01. Status: accepted. A deliberate deviation from the classic client (ADR 0009), with the original's
menu kept as an option. Spec: `re/screens/options.md` §14, `re/screens/mainscr.md` §6.4. Implemented by
`ig2-client::mainscr` (`Act::Settings`, the label), `ig2-client::options` (`OptionsScreen::settings_only`, the
restart line), `ig2-client::display` (the two rows, `language`) and `ig2-client::main` (`language`).

## Context
The original's Options screen (`AktKep` 13) opens from a game screen with Esc or `O` (`DoKeyboard` exe 0x4db1a0) and
from the main menu only behind the "Load game" ring, as the load-only screen (`OptionsCsakLoadLehet`, options.md §1):
its settings pages are there, but a player looking for the settings doesn't look under Load. The main menu takes no
keys of its own (`MainscrKeyboardHandler` exe 0x5000c0 reads only Enter, and only with the debug switch).

The texts' language is fixed at start: `--lang`, else `IG2_LANG`, else what the GOG exe would pick (`ig2.settings`'
`language=`, `locale.txt`, the system locale; locale.md). The install's files, the fonts' character order (the
Hungarian charset), the UI and message tables, the speech, the localized textures and the sim's own text tables are
all read in that language when the client starts, many of them by state that lives for the whole run.

## Decision
1. **Option "Settings in menu"** (Remake row, `MenuSettings` in the remake's settings file; on by default, 0 = the
   original's menu): the main menu shows a "Settings" label in the bottom right corner while its main ring set is up
   (no race choice, scenario list, name entry or multiplayer screen), and `O` there does the same as a press on it.
2. It opens the Options screen as the load-only screen does (over the main menu's scene, Back to the main menu), but
   with the slot list, Load and Save left out: the sound and game pages and the Remake rows only.
3. **Row "Language"** (Remake row, `Language`): Default (0, the language the client would pick without the row, the
   classic), English, Magyar, Deutsch, Francais, Espanol, Italiano (1 to 6; plain letters, as the
   remake rows are drawn in the game's font). It sits under `--lang` and `IG2_LANG` and over the exe's choice; while
   one of those two is set the row is greyed.
4. A changed language takes effect at the next start: the screen shows "Restart to change the language" under the
   rows while the row differs from its value at start. Switching live would mean reopening the install and
   rebuilding the texts, fonts, speech and textures and the sim's tables of a running game; not worth the risk of a
   half-switched screen.

## Consequences
- The original's menu and Options screen are unchanged with `MenuSettings` 0 and `Language` 0; capture comparisons
  of the main menu set `MenuSettings` 0 (the label isn't the exe's).
- Recorded replays are unaffected: neither row reaches the sim.
