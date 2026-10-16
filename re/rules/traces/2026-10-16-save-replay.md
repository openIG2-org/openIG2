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
| `replay-quick1-x16-ui` | ×16 | 60 | 23 Oct → 11 Nov 2327 (dumps `InfoOldHonap`, the scrolling line, the empire names) |
| `replay-quick1-spies-x16` | ×16 | 200 | 23 Oct → 27 Dec 2327 (`--spies`: staged spy missions, catch checks and reports; dumps the family names) |
| `replay-quick1-spies-force1-x16` | ×16 | 120 | `--spies-mode force1`: every mission succeeds; the human has money and one ship |
| `replay-quick1-spies-force2-x16` | ×16 | 40 | `--spies-mode force2 --seed 2`: every spy is caught; six of the human's caught by the AI |
| `replay-quick1-spies-force3-x16` | ×16 | 120 | `--spies-mode force3`: a recruit every day; one spy returning |
| `replay-quick1-spies-lost-x16` | ×16 | 40 | `--spies-mode lost`: a third empire revived, duels tuned to "identified and lost" |
| `replay-quick1-battles-space-x16` | ×16 | 16 | `--battles --battles-mode space`: six auto-resolved space battles (duels, a planet battle) |
| `replay-quick1-battles-ground-x16` | ×16 | 16 | `--battles --battles-mode ground`: five auto-resolved ground wars on undefended planets |

## Result

All 522 ticks match: every chunk the port models (calendar, money and pools, empires, contacts, relations,
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

The spy recording (`run_replay.py --spies`, `info-messages.md` §3.3) adds 7 spies to the human's roster and 8 to
AI 3's before save 0 (a mission ending on the first day, a catch check against an AI counter-spy, one coming home,
one ranking up 20 times, a double agent its own counter-spy exposes; the AI's money, sabotage, rebellion,
assassination, kidnap and mass-unrest missions against the human and a catch check meeting the human's
counter-spy). All 200 ticks match, with the report queue (`KemBeszolasok`, 26 reports: 20 rank-ups, the
exposure, mission results, catch outcomes, the AI captor's fate, the assassination's victim report) and the
scrolling line holding ids 100, 101, 103, 106, 136 (sabotage: the building's tech and the planet), 139, 140 and
141 (the kidnapped family member's name, `KemCsaladtag_Nev`).

Four more staged spy recordings (`--spies-mode`, all exact) reach the rest of the spy posts: `force1` (the SP
debug switch 0xB77250 = 1, every mission succeeds) the human's spy coming home (130), money stolen (135), its only
ship destroyed (137) and the mass unrest (142); `force2` (every spy is caught; the AI's roster padded so the
captor's loyalty slots are saved, stream 4 reseeded) the AI captor's fates: turned (kind 0x10, 113), double agent
and released (0x12, 101), executed and a failed copy (0x11, 106); `force3` (a recruit every day) an applicant
(kind 2, 102); `lost` (empire 1 revived so three living empires know the AI's spies, duels between combat-1
spies) the kind-4 report and 104. Two findings: the spec had the mission result's id wrong (101 done, not 104;
104 is "We have killed an enemy spy!", the kind-4 report), and the destroyed ship emptied the human's fleet,
which `HajoDelete` doesn't delete but sends as lockstep message 0x1b; `MULTIGET_Starmap` deletes it before the
next tick (`Game::fleet_emptied`, `space-combat.md` §5.3). The first force2 recording also showed that the
captor's loyalty roll reads the spy's slot `k` in the captor's roster even past its end: bytes the save doesn't
hold (re-staged so they are inside it). Kind 0xf (released) never reaches the human in SP: the AI captor draws
fates 2..=4.

Two staged battle recordings (`run_replay.py --battles`, `info-messages.md` §3.2) hand the human's empire to the AI
(`PlayerKi[EN] = 0`: every battle auto-resolves, the human's messages are still posted), switch the AI pass off
(debug switch 0xB77244) and make every fleet visible (`Story_Minden_Flotta_Latszik`), then place attackers one step
from their targets; the starmap loop stops at each battle, so one battle a tick follows. `space`: five duels (lopsided
won and lost, even won and lost, the AI attacking) and an attack on a human planet whose defence fleet fights it,
with the planet's two defence ships on the defender's side (one destroyed) — messages 15, 16, 17, 18. `ground`: the
human's and the AI's planets without defence ships, working guns or a nearby fleet, attacked by fleets of ground
vehicles (6 / 16 / 40): five `GroundWarIndit` wars against the fortress turrets, one lost, two even and two lopsided
wins with the planets captured (`BolygoElfoglal`, tech capture, attackers recalled) — messages 11, 12, 22, 23. All 32
ticks match byte for byte, with `spFlottak`/`SpWarHanyFlotta`/`spKozeliBolygo`/`SpWarKiNyert` (unsaved) compared.
Three findings: the battles run inside the movement loop and the step goes on afterwards (`fleet-movement.md` §4.3;
the port pauses the loop, `Starmap::tick_until_battle` / `resume`); a destroyed defence ship's slot is zeroed and
the list closes up in place, leaving the old last entry (`space-combat.md` §5.3); and the damaged buildings of a
ground war are repaired every tick by `FRISSIT_Epuletek` (exe 0x4746f0, `ground-combat.md` §6.5), which the tick
hadn't ported (no building had been damaged before). Not reached: a lopsided attack that loses (24/25; even 60
vehicles of the weakest design beat the turrets), a won planet battle followed by a landing, real-time routes.

## What comes from the exe


- ~~Unported `FRISSIT_Trade`~~: ported since (`2026-10-16-trade.md`); the recordings were re-made with the market's
  unsaved globals, and two staged ones (purchases, deliveries) added.
- Localised names (`DesignPoenosanElnevez`, the main fleet's name) and the uninitialised bytes of appended
  records (`Game::uninit`, as the AI replay's `fresh_design`) come from the exe's save.
- ~~Unported info charts and messages~~: ported since (`rules/info-messages.md`): `InfoCharts`,
  `InfoChartsSmoothed`, `BattlesWon`, the inbox (`Uzenetek`, `UzenetekRandomKitol`, `UzenetPlaybackek`, the
  message-state fields) and the scrolling line (`ScrollUzenetek*`, `UzenetekDb*`) are compared now; every
  recording's first tick samples the charts (`InfoOldHonap` is unsaved and differs from the save's month), 17
  month changes resample them, and the year's tax message (two recordings) and "enemy fleet spotted" (one)
  match with their texts from the install's Hungarian `uzenet.txt` (the test picks the language whose texts
  begin the recordings' messages). The UI's draining of the line between ticks (`FRISSIT_Scroll2`) is replayed
  from the exe's count (no recording shows one). Skipping the sampler is caught at tick 2, skipping the posts at
  tick 211.
- Chunks only unported systems write are reported, not compared: the starmap view (`StarmapPlanetKering`) and
  per-tick counters in the ground battlefields (`WF[i]+0x8d0/+0x8d4`).
- A recording whose save was loaded before quick.scn ran a tick (Ido2 0 at the load, slower start-up) fails at
  tick 1 on `StoryError` 0xF23A1C (0 → 1 in the exe's first story check; the other recordings load at Ido2 130
  with it already set): kept as `_local/traces/story-err-replay-x16-load-at-ido2-0` for the story engine.

## Mutation check

Skipping `FRISSIT_Epuletek` is caught at tick 3 of the ground recording, skipping the battle's seen-design marking
(`KemkedesUrharcLekezel`) at tick 2 of the space one. Skipping a system in the driver is caught: economy (tick 52), relations, the AI, spies, AI messages, research,
the story tick, `FRISSIT_Diplomacy`, the starmap (tick 2), production (tick 114), the research hand-offs (tick
112), story special 0x0B (tick 282), the battle counter. Not caught (never reached by these runs): trade's
monthly libc draw (no libc draw follows a month change) and `FelfedezBirodalom` (the empire met was 9, which
gets no contact slot).
