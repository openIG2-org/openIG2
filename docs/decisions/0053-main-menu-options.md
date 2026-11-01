# ADR 0053: the main menu's Options screen

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::options::modern` (the screen) on the model of
`ig2-client::options` and `ig2-client::display` (the rows, ADR 0048), opened by the main menu's Settings label and
`O` (ADR 0031).

## Context
The decision of 2026-11-01 (`docs/ENHANCEMENTS.md` → Decisions) splits Options in two: the in-game screen keeps the
original's look (ADR 0048, the Remake page), the main menu gets a modern screen with every setting in tabs. The
original reaches its settings from the menu only behind the Load ring; since ADR 0031 the Settings label opened the
in-game screen without its slots. That screen has three pages of the original's 640×480 rows and no word on what a
row does.

## Decision
1. **One model.** The screen edits the same `OptionsScreen` the in-game screen uses: its `rows` (the exe's 17 from
   the user's exe and the remake's from `display::rows`), its three sliders (music, sound, gamma) and, ours, the
   multiplayer name (`MultiplayerNev`, at most 9 letters as the menu's name entry). It writes through the same
   `OptionsScreen::store` into the remake's own settings file; no value has a second copy. A row added to
   `display::rows` shows on both screens.
2. **Tabs**: Display, Sound, Interface, Game, Accessibility, Multiplayer, Language. A remake row goes to the tab of
   its group (`options::Group`, which gains `Language`: the Language row, names in the game's language, Hungarian
   articles); an exe row on page 1 (graphics) to Display, on page 2 to Game; the gamma slider to Display, music and
   sound to Sound; the player name to Multiplayer. Order in a tab: on Display the remake's rows (window, resolution
   first), gamma, the exe's graphics rows; elsewhere the exe's rows and the name first, then ours; each in declared
   order.
3. **Controls**: an on/off row is a switch; a row with more values a choice `< value >` (left and right arrows step
   back and forward, a press on the value steps forward, wrapping); a slider a track with a knob and its percentage
   (a press or drag sets it, the arrows step 5); the name a text field. A disabled row (its dependency, a language
   forced by `--lang`/`IG2_LANG`) is dimmed and ignores input, as on the in-game screen (`check_dependencies` after
   every change).
4. **Names**: the exe's rows are named by their keys' meaning (`GetString(13, 2, n)` with the string the key stands
   for: rows 7–16 take strings 6–15), not the shifted names the in-game screen copies from the exe
   (re/screens/options.md §6); `Cutscenes` and `VROT` keep the exe's compiled names. The sliders keep their titles.
5. **Description line**: the hovered (else the focused) setting's one-line description above the buttons, from the
   remake text table (ADR 0042, English and Hungarian), as are the title, tabs, buttons and the restart notice.
6. **Back / Apply**: changes are previewed on the screen (the volumes are heard at once); Apply writes them into the
   settings file and applies them (window mode, resolution, sharp UI, …) with the screen staying up; Back leaves,
   taking back what was not applied. Esc and `O` (the key that opened it) are Back; Apply is dimmed and does nothing
   while nothing changed; an emptied name is not applied (the previous one stays).
7. **Keyboard**: Tab and Shift+Tab (also Page Down/Up) change tabs, Up/Down move the focus over the tab's settings
   and the two buttons, Left/Right step a value or slider, Enter or Space toggles, steps or presses the focused
   button; in the name field letters type (not with Ctrl or Alt), Backspace erases, Enter or Esc ends the edit. A
   held Esc, Enter, Space or `O` acts once, and its repeats don't reach the main menu after the screen closed. A press
   on a setting's label only focuses it; a press on its value side changes it.
8. **Looks**: drawn with the remake's own 2D primitives (the kit's rounded faders, filled quads) and the game's font
   (sizes 2 and 4), no shipped picture; the 640×480 layout in the window's virtual pixels (ADR 0010), so it scales
   with the window, Sharp UI (ADR 0043) and Sharp text (ADR 0046). Its colours and metrics are one `Theme` value, the
   first theme of the themeable UI that follows.

## Rejected
- **A second settings store for the new screen**: two copies drift; the in-game screen must see the same values.
- **A description line per row**: the rows would need twice the height and a Display tab of 18 settings would
  scroll; one line for the setting under the pointer reads the same.
- **Applying each change at once (as the in-game screen does)**: a resolution or fullscreen switch under the
  pointer moves the screen while the player clicks through values; Apply makes it one step.
