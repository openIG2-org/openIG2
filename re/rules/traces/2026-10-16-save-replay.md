# 2026-10-16: an original save loaded into the sim and replayed against the exe

Spec: `re/formats/save.md` (the format), `re/rules/time-and-rng.md` §1 (the tick order). Code: `ig2-sim::save`
(the bridge), `ig2-sim::game` (the tick driver), `engine/crates/ig2-sim/tests/save_replay.rs`. Tracer:
`tools/trace/run_replay.py` + `replay.js`.

## Method

The exe and the port start from the same original save, `_local/saves/quick-1.sav` (quick.scn, 23 October 2327,
written by the exe's own `LoadSave` under Frida). `run_replay.py` starts `quick.scn`, loads the save with the
exe's `LoadSave(99, name, 1)` (exe 0x543c80) from the frame hook and sets the speed. At every entry of
`IdoNapValtozott` (exe 0x53e970: once per strategic tick, right after `FRISSIT_Datum` advanced the clock) it
writes save k with `LoadSave(100 + k, name, 2)`, dumps the globals the save doesn't carry (radar tables, story
statics, `BirodalomMIzik`/`BirodalomTamadhat`, cursors, `DipOldNap`, the weighted-choice state, …) and pins libc
`rand()` with `srand(seed + Ido2)`. Save 0 (taken right after the load) is byte-identical to `quick-1.sav`.

The port loads save 0 into a `SaveState` (the exe's static globals as a sparse memory image keyed by VA, the
pointer-held records as arrays, the story queues), applies the load-time dump, and for each tick k runs
`Game::advance_clock`, compares its state's chunks with save k and its unsaved globals with dump k, seeds its
libc (MSVC LCG) the same way, then runs `Game::run_tick` (economy on days 1/7/13/19/25, relations, the AI, spies,
AI messages, research, the story tick, `FRISSIT_Diplomacy`, the starmap with arrivals and production). Each
system decodes its view from the state and stores it back, so they see each other's writes as in the exe.

## Recordings

| Recording | Speed | Ticks | Game time |
|---|---|---|---|
| `replay-quick1-x16` | ×16 | 40 | 23 Oct → 6 Nov 2327 |
| `replay-quick1-x16-long` | ×16 | 300 | 23 Oct 2327 → 28 Jan 2328 |
| `replay-quick1-x1` | ×1 | 150 | 23 → 26 Oct 2327 |

## Result

All 490 ticks match: every chunk the port models (calendar, money and pools, empires, contacts, relations,
last-contact table, spies, fleets, ships, vehicles, waypoints, planets, per-planet tables, techs, research,
designs, buildings, the AI's tables, `StoryAdat`, the story queues and tables, RNG streams) equals the exe's save
byte for byte, and so do the unsaved globals the ported systems write (radar tables, story statics and queues,
`DipOldNap`, the weighted-choice state, autobuild pick, map cursor, trade ports). Covered on the way: 16 economy
steps (3 autobuild purchases, finished buildings), the year's close (1 January), a research completion with
the AI's new designs (story special 0x0D, `FRISSIT_MI_DESIGN`), a spy mission, the human meeting an empire on
radar (story special 0x0B).

The replay found two hand-offs between systems the driver must carry out (both from the specs): a finished
project raises story special 0x0D and runs `FRISSIT_MI_DESIGN(e, false)`; the human meeting an empire raises
story special 0x0B and `FelfedezBirodalom`. It also found that `BirodalomMIzik`/`BirodalomTamadhat` (exe
0x915F58/0x915F44) aren't saved: without them the AI's main pass skips every empire.

## What comes from the exe

- Unported `FRISSIT_Trade` (the merchants, ELF 0x7b0ad0): its stream 11, the merchant countdown `StoryAdat+0xc4`
  and the trade fields `+0x6c..+0x80` of the design it offers are copied from save k (nothing ported reads them);
  the port models its one libc draw on a new month.
- Localised names (`DesignPoenosanElnevez`, the main fleet's name) and the uninitialised bytes of appended
  records (`Game::uninit`, as the AI replay's `fresh_design`) come from the exe's save.
- Chunks only unported systems write are reported, not compared: `InfoCharts*` (`FRISSIT_Info`), `Trade*`, the
  message lists (`Uzenetek*`, `ScrollUzenetek*`), the starmap view (`StarmapPlanetKering`) and per-tick counters
  in the ground battlefields (`WF[i]+0x8d0/+0x8d4`).

## Mutation check

Skipping a system in the driver is caught: economy (tick 52), relations, the AI, spies, AI messages, research,
the story tick, `FRISSIT_Diplomacy`, the starmap (tick 2), production (tick 114), the research hand-offs (tick
112), story special 0x0B (tick 282), the battle counter. Not caught (never reached by these runs): trade's
monthly libc draw (no libc draw follows a month change) and `FelfedezBirodalom` (the empire met was 9, which
gets no contact slot).
