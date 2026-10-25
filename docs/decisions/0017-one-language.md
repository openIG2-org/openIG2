# ADR 0017: One language everywhere

Date: 2026-10-24. Status: accepted (a maintainer's play report of 2026-10-24; the maintainer
accepted the option). A deliberate deviation from the classic client (ADR 0009), with the original's mix kept as an
option. Implemented by `ig2-client::ui::names_txt17` and the settings read in `window.rs`.

## Context
ig2.exe mixes languages on purpose or by accident:
- `StringReadAll` (exe 0x531e40) fills the name tables from the **Hungarian** `txt17.txt` whatever the game's language
  (re/screens/starmap.md §11, §15): the empire names `BirodalomNev`, the morale names `Moral_Nev`, the problems
  `Problemak_Nev`, the surface, wealth and relation names, the tax, auto-build and priority menus, the radial icons'
  tooltips, the colony's area names, the spy screen's names. An English run shows "GODAN KIRALYSAG" and "MOR: Jó".
- The main menu's ring labels are pictures (`textures/main/eng/`, `eng` a literal in both binaries,
  re/screens/mainscr.md §2 item 11). Every localization of the install ships the same English pictures
  (`localization/<lang>/dataraw/textures/main/eng/` are byte-identical copies).

The project wants one language on every screen.

## Decision
1. **Option "one language", on by default.** The name tables come from `txt17.txt` of the game's own language (the
   localized file, the same sections and indices as the Hungarian one; checked for en, hu, de), encoded in that
   language's code page, so they match every other text on the screen.
2. **Classic mode** (`OneLanguage` 0 in the remake's settings file, or `--texts classic`): the Hungarian file as the
   exe reads it. The capture comparisons (`run_screendiff.py`) run the exe, so the client's `--snapshot` defaults to
   classic; acceptance compares classic mode.
3. **The ring labels stay the install's pictures** in both modes. The engine ships no texts of its own (ADR 0001), and
   the game's own text tables have no string for four of them (SINGLE PLAYER, MULTIPLAYER, CREDITS, the tutorial's
   PART 1..5); the others exist only in longer forms ("Local network", "Normal game") that don't fit a ring. Drawing
   them as text would need laid-out labels on the 3D rings and strings we would have to write ourselves; that is a
   separate proposal if the maintainers want it.
4. The rules don't change. Names the game writes into its own state stay as the exe writes them in both modes:
   the new game's race, tech and design names and the first fleet's name (`StringReadAll` into the exe's tables,
   saved), and the empire names the game's posts quote (`Game::empire_names`, shipped in the multiplayer snapshot).
   They are saved and must be byte-identical on every peer and against the exe's replays, so a message can still
   name an empire in Hungarian; only the tables the screens draw from switch.

## Consequences
- Multiplayer: each player sees the screens' names in their own language; nothing new crosses the wire.
- Making the game-state names (point 4) follow the language too would need them out of the saved state (drawn from
  indices at display time); a later step if the maintainers want it.
- Lines formatted with these names can be longer in some languages ("Solarian Federation" against "Emberi
  Szövetség"); the exe's own field widths and clipping still apply.
- The Options screen has no row for it yet (a remake row, styled like the original's, joins the one for ADR 0015).
