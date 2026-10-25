# ADR 0016: Deliberate keeps: tick-bound rates, MP-safe local actions, our own settings and save files, the developers' cheats, load and frame latency, unused build modes

Date: 2026-10-24. Status: accepted. A record of differences from the classic client (ADR 0009) that the remake keeps on
purpose, with no classic option, because the original's behaviour depends on the frame rate, breaks multiplayer, or
writes into the user's install. Each one is a row in `re/deviations.md` marked `keep`; this ADR is the reason those
rows stay open.

## Context
ADR 0009 makes fidelity the default and every deliberate difference opt-in. A few of the original's behaviours can't
be reproduced without giving up a goal the project ranks higher (README → Goals: multiplayer first, high refresh rates,
the user's install is read, never changed):
- Some per-frame code in ig2.exe runs once per rendered frame. Its rate follows the frame rate, which on the original
  machines was roughly constant and on modern ones isn't: at 240 Hz the effect runs four times as often.
- Some actions change the game locally on the machine that clicked, with no network message. In a lockstep game
  (ADR 0007) that splits the machines' states.
- The original writes its settings and saves over files in the install.

## Decision
1. **Tick-bound rates.** Where the exe runs game-visible per-frame work once per rendered frame, the client runs it
   at a fixed rate instead: the rate a 60 Hz display gives (60 a second), or once per 100 Hz UI tick where the code
   sits in the screen's input loop. Rendering stays at the display's rate and interpolates. Rows:
   - #98, colony thunder: `random(400)` drawn 60 times a second, not once a rendered frame (`audio.md` §6.2).
   - #107, trade screen: `Trade_Main`, the right-button quantity repeat, the scroll and the row drags once per 100 Hz
     UI tick (`trade.md` §6.2).
   - #123, diplomacy: `Diplomacy3DUpdate`'s per-frame steps 60 times a second (diplomacy.md §3.1).
   New cases of the same kind follow this rule and add a row; they don't need a new ADR.
2. **MP-safe local actions.** An action that changes the shared game goes through a lockstep order on every machine,
   even where the exe changes it locally or reads the receiving machine's own UI state. Rows:
   - #13, the ground war's retreat toggle (`MULTIGET_Colony` 0x12f): the exe flips the flag on the battlefield of
     the receiver's `AktPlanet`; ours flips it on the message's planet (colony.md §9.1). In single player both are the
     same planet.
   - #127, the landing clicks (`PlaceTank`, `UnplaceTank`, `PlaceAllTanks`): our own orders
     `GroundOrder::{Place, Unplace, PlaceAll}` (message kinds 0x10000..=0x10002); the exe changes the battlefield
     locally (colony.md §11.7).
   The orders carry the same effect as the exe's local change, so single-player results are the same.
3. **Our own settings and save files.** The remake never rewrites the original's files:
   - #69, settings: the keys the Options screen sets go to `ig2-rework-options.dat` beside `options.dat` (same
     format, read over the original's); `options.dat` stays as it is (options.md §7, §14).
   - #70, saves: slot saves are ADR 0007's format as `save<n>.ig2s`, listed over a `save<n>.sav` of the same slot;
     the original's `.sav` files load as before (options.md §5.4, §14).
4. **The developers' cheats** (added 2026-10-25, FX15). `ExecuteString`'s debug commands (stage B, about 90 cheats,
   input.md §5.1) need `DEBUGLEHET`, set only from an `init/config` file the GOG install doesn't have, or `DEBUGX`,
   set only by stage B's own `debug`; they act on the typing machine alone (a desync in multiplayer). They are not
   ported, and neither are the 0x3fd system lines only they send. This covers `DoKeyboard`'s debug keys, the battle's
   `DEBUGX` keys and the main menu's `DEBUGLEHET` Enter quick start (`MainscrKeyboardHandler` 0x5000c0). The client's `DEBUGX` readers stay, so a save with the flag set behaves as in the exe.
5. **Load times and frame latency** (added 2026-10-25, FX1). The original's waits that come from its loading or its
   frame loop, not from a rule, aren't reproduced:
   - #248, keys during a load: the exe refuses keys while a screen's synchronous `_Belep` load runs (its `FRISSITES`
     catch-up ticks run `DoKeyboard` with `Belep` set, ~120 ticks after a load onto the starmap). The client's loads
     don't take those ticks; the closing animations, which are timed by rule, still refuse keys (input.md §1.1).
   - #250, the opening sound: the exe plays a screen's opening sound two rendered frames after the key
     (`IG2_gameUpdate` carries the change out in the next frame, the screen's first `Main` sounds it; 1-2 UI ticks in
     the FX1 trace, frames 2003 → 2005 and 2708 → 2710), the client at the key. The closing sounds are at the key in both.
   - #279, tooltips during a load: the exe runs no `UpdateFrame` while a screen's `_Belep` load holds `Belep`, so no
     tooltip shows for those ~100 UI ticks after a load onto the starmap; the client, which has no such load, shows
     them after their usual delay (input.md §8).
   - #229, the frame order: `IG2_gameUpdate` (exe 0x4e0440 lines 38-53) runs a frame's ticks (`Fake_FRISSITES`, the
     keys), draws it (`UpdateFrame`), then polls the mouse (`Mouse_Lekerdez` 0x5105d0), hit-tests and runs the
     screen's `_Main`; a press is acted on in the same tick as in the client, but its 2D effect is first drawn one
     rendered frame later (design-fx8.json: the held − steps at frames 208/209/210 in both, the exe's grab shows the
     frame before). The orders and sounds `_Main` makes keep their ticks. Where a screen's own draw or 3D pass reads
     the input (so the order moves a tick, a sound or a state), that screen ports it: research (the list row
     and the mouse-chosen category one frame later), design (the slides from the next 3D update), the
     messages screen (FX12). A capture taken in the frame after an input may show the client one frame ahead.
6. **Build modes the GOG release doesn't use** (added 2026-10-25, FX1): #259, the remaster's mobile toolbar
   (`IG2_TOOLBAR` exe 0xfe6532, `toolbar=1`, off by default: spots 2..5 at 28 × 18, spot 7 at x 310, the history
   text x + 0x28, the touch sidebars; widescreen.md §5, input.md §6) is not modelled; the client draws the classic bar.

## Consequences
- These rows stay `keep` in `re/deviations.md`; their keep lines point at this ADR.
- A capture comparison that differs only through one of these rates (thunder count, trade repeat speed at another
  frame rate) is not a failed acceptance. Comparisons run at 60 Hz, where the rates agree.
- The original's saves and settings remain usable by ig2.exe after playing the remake; the remake's saves aren't
  readable by ig2.exe (an export to `.sav` would be a feature of its own).
