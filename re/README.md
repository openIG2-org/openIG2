# Reverse-engineering knowledge base

What is known about ig2.exe (GOG build) and where each fact lives. Specs: [`formats/`](formats/) (files),
[`rules/`](rules/) (simulation; runtime reports in [`rules/traces/`](rules/traces/)), [`screens/`](screens/) (client),
[`symbols/`](symbols/) (ELF names ported to the exe). [`deviations.md`](deviations.md) lists known differences from the original and original bugs.

## Systems

State: **verified** = matched against the exe at runtime (Frida or capture); **partial** = gaps listed in the spec.

| System | Spec | State | Crate / module |
|---|---|---|---|
| Overview | [`00-overview.md`](00-overview.md) | — | — |
| Container `ecGs`/`neGs`, PVR v3, VFS (ADR 0001) | [`formats/ecgs-container.md`](formats/ecgs-container.md) | verified | `ig2-formats` |
| `.xv` models, `.ani`, colony maps, `.scn`, formations | [`xv`](formats/xv.md), [`ani`](formats/ani.md), [`colony-map`](formats/colony-map.md), [`scn`](formats/scn.md), [`formation`](formats/formation.md) | verified | `ig2-formats` |
| Text tables, dialogue, story fleets, tutorial script; locale | [`formats/text.md`](formats/text.md), [`formats/locale.md`](formats/locale.md) | verified | `ig2-formats` |
| Save games, MP blob; saves as sim state + the whole tick (§8) | [`formats/save.md`](formats/save.md) | verified | `ig2-formats::save`, `ig2-sim::{save, game}` |
| Compiled-in rules tables | [`rules/tables.md`](rules/tables.md) | verified | `ig2-rules` |
| Story data (building/designs/values) | [`rules/story-data.md`](rules/story-data.md) | verified | `ig2-formats`, `ig2-rules` |
| Sim clock, calendar, RNG streams | [`rules/time-and-rng.md`](rules/time-and-rng.md) | verified | `ig2-sim::{time, rng}` |
| Colony economy (`FRISSIT_Planet`) | [`rules/colony-economy.md`](rules/colony-economy.md) | verified | `ig2-sim::colony` |
| Colony side systems (rebellion, conquest, autobuild, money) | [`rules/colony-side.md`](rules/colony-side.md) | verified | `ig2-sim::colony_side` |
| Building placement, build cursor | [`rules/colony-placement.md`](rules/colony-placement.md) | verified | `ig2-sim::placement` |
| Fleet movement, arrivals, radar/visibility | [`rules/fleet-movement.md`](rules/fleet-movement.md) | verified | `ig2-sim::{fleet, radar}` |
| Space combat: auto-resolve (§1–§6), real time (§7) | [`rules/space-combat.md`](rules/space-combat.md) | verified (U17 gaps) | `ig2-sim::{spacewar, spacebattle}` |
| Production and research queues | [`rules/production-research.md`](rules/production-research.md) | verified | `ig2-sim::{research, production}` |
| Design orders (`MULTIGET_Design`) | [`rules/design-orders.md`](rules/design-orders.md) | verified | `ig2-sim::game::design_orders` |
| Diplomacy (AI and human side, deaths, victory) | [`rules/diplomacy.md`](rules/diplomacy.md) | partial (U20) | `ig2-sim::diplomacy`, `game::death` |
| Espionage | [`rules/espionage.md`](rules/espionage.md) | verified | `ig2-sim::spy` |
| AI empire | [`rules/ai-empire.md`](rules/ai-empire.md) | verified (MP untraced, U22) | `ig2-sim::ai` |
| Ground combat: auto-resolve, real time (§11), battle AI | [`rules/ground-combat.md`](rules/ground-combat.md), [`rules/ground-ai.md`](rules/ground-ai.md) | verified (U23 gaps) | `ig2-sim::ground` |
| Trade market | [`rules/trade.md`](rules/trade.md) | verified | `ig2-sim::trade` |
| Info charts, message line, inbox | [`rules/info-messages.md`](rules/info-messages.md) | partial (U25) | `ig2-sim::{info, messages}` |
| Story engine and every campaign script (§8) | [`rules/campaign.md`](rules/campaign.md), [`rules/campaign/`](rules/campaign/) | verified | `ig2-sim::{story, campaign, elem, rnd, …}` |
| New game (campaign, scenario, MP galaxy, skirmish, tutorial) | [`rules/new-game.md`](rules/new-game.md) | partial (load mode 5 unported) | `ig2-sim::game::new_game` |
| Tutorial interpreter | [`rules/tutorial.md`](rules/tutorial.md) | verified | `ig2_sim::tutorial` |
| Game end, high scores | [`rules/game-end.md`](rules/game-end.md) | verified | `ig2-sim::game`, `ig2_formats::highscore` |
| MP per-peer state, real-time battles in lockstep | [`rules/multiplayer.md`](rules/multiplayer.md); internet relay: ADR 0018 | partial (U7) | `ig2-sim::game::realtime`, `ig2_net::rendezvous` |
| Lockstep, QUIC, discovery, port mapping, lobby, sessions | ADR 0005, 0007, 0008, 0011, 0012, 0014 | partial (U7) | `ig2-net` |
| Screen inventory, exe functions per screen | [`screens/README.md`](screens/README.md) | — | `ig2-client` |
| Top bar, faders, brushes, map names | [`screens/hud.md`](screens/hud.md) | partial | `ig2-client::hud` |
| Starmap screen + panels, menus, info panels | [`screens/starmap.md`](screens/starmap.md) | partial (U26) | `ig2-client::starmap*` |
| Colony screen, ground-war UI | [`screens/colony.md`](screens/colony.md) | partial (U27) | `ig2-client::{colony, colony_ui, colony_war}` |
| Research screen | [`screens/research.md`](screens/research.md) | partial | `ig2-client::research` |
| Design screen | [`screens/design.md`](screens/design.md) | partial | `ig2-client::design*` |
| Diplomacy screen | [`screens/diplomacy.md`](screens/diplomacy.md) | partial (U32) | `ig2-client::diplomacy*` |
| Spy screen | [`screens/spy.md`](screens/spy.md) | partial (U21) | `ig2-client::spy_screen`, `spy_scene` |
| Trade screen | [`screens/trade.md`](screens/trade.md) | partial | `ig2-client::trade*` |
| Info and messages screens | [`info`](screens/info.md), [`messages`](screens/messages.md) | partial (U28) | `ig2-client::{info_screen, messages_screen}` |
| SpaceWar screen | [`screens/spacewar.md`](screens/spacewar.md) | verified (accepted) | `ig2-client::spacewar_*` |
| Options, load/save | [`screens/options.md`](screens/options.md) | verified (accepted) | `ig2-client::options` |
| Main menu, new-game setup | [`screens/mainscr.md`](screens/mainscr.md) | partial | `ig2-client::mainscr*` |
| Start / Irc (replaced by our MP room) | [`screens/start.md`](screens/start.md), ADR 0014 | partial | `ig2-client::{mp_lobby, mp_wait}` |
| Help | [`screens/help.md`](screens/help.md) | partial | `ig2-client::help` |
| Tutorial overlay | [`screens/tutorial.md`](screens/tutorial.md) | partial | `ig2-client::tutorial_ui` |
| Overlay backgrounds | [`screens/overlay-background.md`](screens/overlay-background.md), ADR 0015 | partial | `ig2-client::backdrop` |
| Input (keys, hot-spots per screen) | [`screens/input.md`](screens/input.md) | partial | `ig2-client` |
| Render layer (GL, shaders, shadows, lights) | [`screens/render.md`](screens/render.md) | partial | `ig2-client::{render3d, lighting}` |
| Widescreen layout | [`screens/widescreen.md`](screens/widescreen.md), ADR 0010 | verified | `ig2-client::kit` |
| Audio (mixer, music, voices) | [`screens/audio.md`](screens/audio.md), [`screens/sound.md`](screens/sound.md) | partial (U29) | `ig2-client::audio` |
| Video (`.bik` → mp4) | [`screens/video.md`](screens/video.md), ADR 0013 | partial (U30) | `ig2-client::video`, `ig2_formats::mp4` |
| Capture-comparison results | [`screens/diff-results.md`](screens/diff-results.md) | — | `ig2-screendiff` |

## Open unknowns

- **U1** Did the remaster change rule constants versus 1999? Blocks nothing in classic (the exe is normative); matters for "1999" claims.
- **U2 (rest)** The tutorial's `PLAYWAV` names (`Ncom_tutp1_*.mfx`) have no file in the install.
- **U6** Mapping of the 1999 `.ani` names to `anim/mpeg4/*.mp4` (`GetAnimFile` returns the current name; the table lives elsewhere).
- **U7** Original MP failure modes; rendezvous service (browser, short codes, hole punching, relay) undecided (ADR 0005). Blocks online play beyond LAN/direct IP/join code/UPnP.
- **U8** Text codepages not checked against the charset glyph images. Blocks correct non-ASCII text.
- **U13** Shipped data defects (broken dialogue refs, unreachable msg scripts, 3 missing diplomacy specials in DE/FR/ES/IT, 3 missing clips): classic vs enhanced fix needs an ADR.
- **U14** Unpinned table fields: hull +0x10..+0x23, weapon +0x40/+0x48/+0x59, tech +28/+36/+60.., building +1/+24/+28 (`rules/tables.md`).
- **U15** Colony side: MP economy steps untraced (the desync soak found no peer-to-peer desync here, `rules/multiplayer.md` §7.1.1); story yields `st_v(0x5FB/0x60F)` in `EpuletNew`; the `+0xba` counter of `BolygoInfoRemove`.
- **U17** Real-time space battle: MP, chain lightning, the victory flight, several battles at once (shared pools), > 2 participants untraced.
- **U18** Production/research: MP paths, MP slow-down, station rally untraced (soak: no peer desync).
- **U20** Diplomacy: reason codes (`MiertValaszolt…`) and `vv_*` flags, `NEWGAME_Diplomacy`, MP, `ProgramPlayers` 18. Blocks AI-to-human message texts (also U25 ids 6/10).
- **U21** Espionage: `EpuletDelete`'s colony-screen/ground-war targets; spy screen sounds.
- **U22** AI empire: MP pace observed in ig2.exe (`ai-empire.md` §14: same as the sim; per-call MP branches still untraced); `ProgramPlayers` 18 overflows (`Kemek`, `MiTervMutato`); `INIT_MI_KIESETT` (dropped player) is used in MP by `PlayerAway` (session tests only, not traced in ig2.exe).
- **U23** Ground war: `GenerateMap` editor mode, `Refresh2` visuals, `UpgradeTanks`, MP (soak: no peer desync), stealth, mines untraced; planet deals don't reach the battlefield.
- **U25** Messages: ids 6, 10 (need U20), 13; a recorded real-time ground war's end; `Grwar8020hoz`; save replays don't run campaign story lines yet.
- **U26–U32** Screen gaps (starmap, colony, info/messages, audio, video, unfinished screens, diplomacy): listed in each screen spec's open items and in `deviations.md`; load (mode 5) is unported in `ig2-sim`.

## Hard facts (don't re-derive)

### General
- `ig2.exe` = Gyroscope 2016/17 remaster wrapping the 1999 Hungarian C code (`FRISSIT_*` = update, `Belep`/`Kilep` = enter/exit, `Talalmany` = tech, `Epulet` = building, `Flotta` = fleet, `Kem` = spy, `MI` = AI).
- The unstripped Linux ELF `ig2` (x64, -O0) is the symbol oracle; 1,704 names ported in [`symbols/`](symbols/). Some ported names/addresses are wrong (the specs list corrections); helper addresses often differ from the ported names.
- The exe is ASLR-relocated: rebase every address from 0x400000.
- The **exe is normative**, not the ELF: where they differ in precision (f32 vs f64), the exe wins. Literal constants are often doubles in the exe (`(f32)((f64)x * 0.3)`); check `mulsd` vs `mulss` (and the operand order) in the asm when a port is 1 ulp off. The decompile's product grouping can be wrong.
- The exe inlines many helpers the ELF keeps separate; functions missing from the ported table can be found via the source line numbers they push to `rndlog`.
- Rules come from the exe's compiled-in tables (ADR 0004, `rules/tables.md`); `.scn` TALALMANY values are ignored. Live tech table 0xB9C0C8 (276 B/tech).
- Globals: `Ido` 0xBAE04C, `Ev/Honap/Nap` 0xBAE040/44/48, `Ido2` 0xBAE050, `IdoSpeedSzorzo` 0xBAE25C, `IdoPause` 0xBAE264, `AktKep` 0xBAE0FC, `RandSeed` 0xBAF2D0, `PlayerFaj` 0xBAE574, `EN` 0x912A3C, `MultiPlayerMode` 0xBAE2C0, `ProgramPlayers` 0x912ACC (read `ProgramPlayers2` live: 18 after quick.scn, 16 in the image).
- Unsaved globals the sim reads come from a live dump (`replay.js` `VOLATILE`), e.g. `BirodalomMIzik` 0x915F58 (without it the AI skips everyone), `BirodalomNev` 0xBB6730.
- Appended records (fleets, ships, designs, building blocks grown by 20) carry uninitialised bytes the code never writes: modelled by `Game::uninit`; compare whole raw records.

### Clock and RNG ([`rules/time-and-rng.md`](rules/time-and-rng.md))
- 20 Hz sim tick, 100 Hz UI tick, 500 ms clamp (≤ 9 catch-up ticks); `Ido` 50/day, 30-day months; speeds SP 1/4/16, MP 4/8/16; planet economy when `Nap % 6 == 1`.
- 21 rotated-MSVC-LCG streams, `.scn` RandSeed → streams 1–20. Unseeded libc `rand()` drives ground combat, diplomacy texts, trade and generation (MP desync source; ADR 0003).
- `SetIdoLelassit` (0x541270) changes `IdoSpeedSzorzo` mid-tick: the same tick's movement and counters see the new speed.

### Saves ([`formats/save.md`](formats/save.md))
- `LoadSave` 0x543c80: 80-byte header `VERS0002`, then zlib of the whole buffer **capacity** (zero tail); `LoadSaveVariables` 0x544220 writes `[i32 len][bytes]` chunks in a fixed order; same stream on Win32 and x64. `GetSavedGame` 0x543670 = the MP blob.
- **Every `LoadSave`, also writing a save, ends with `FlottakUjraSzamol()`**: the replay calls `Game::saved()` after each traced save.
- Whole-game save/load round trips run in the play-through harness (`docs/BUILDING.md` → Play-through harness).

### Colony, placement, fleets, radar
- `FRISSIT_Planet` 0x474930 (not in the ported table); planet event codes are slots 1..=5 at +0x16a (not +0x168); planet `+0x23` = colony map. `Bolygok` 0xB85EA8 (0x188 B, 1-based).
- `BolygoFellazad` 0x53bc40 runs mid-planet (after the unrest counter, before construction); `BolygoElfoglal` 0x53b720.
- Placement grid 200×200 from mask A; mask B = obstacles for passability layers 1..4; heights i16 (`h/16 − 700 + (200 − b)`); new fleet/ship/vehicle slots take the **last** free one.
- Fleets 0xBB40A4 (0x80 B); `FRISSIT_Starmap` 0x594ae0 sends 0x1b for the player's empty fleets in a later frame. Radar refresh on `Ido2 % 10 == 1`; empire 2 (literal index) gets +1 radar level; AI radar ×1.5 unless allied with a human; only the booster sum is f64.

### Combat
- `SpaceWarIndit` 0x5a7670: no human side or ratio > 5 / < 0.2 → auto-resolve (`SimulateSpBattle` 0x5a5e60, integer, stream 7). `TervUjraSzamol` 0x5423b0 uses f64 for shields/engines/race bonus.
- Real-time battle: slots 0xE417C0 + 0x2D9C·b, stream 6 only (libc cosmetic); projectile pools `fegyveralakok` 0x9256F8 are shared by all slots (trace one battle at a time); the UI writes unit +0x184..+0x190 between ticks.
- Ground: battlefield `WF(p)` = 0x9C3244 + p·0x8EC, unit 0x290 B; 10 hp attrition per pass; 5:1 routing in f64 → f32; stream 2 + libc. The end countdown `WF+0x148` only runs on the colony screen (off-screen → a later `Simulate` hangs). Pod 0x9f transporter partners crash the exe. The first empty projectile slot is taken, so cosmetic debris orders later shots: every libc draw on screen must be replayed.
- `WF::MI` 0x486d40 on stream 13; dropships run on the 100 Hz UI clock (every 5th `ColonyCounter`, all planets).

### Diplomacy, espionage, AI ([`rules/diplomacy.md`](rules/diplomacy.md), [`rules/espionage.md`](rules/espionage.md), [`rules/ai-empire.md`](rules/ai-empire.md))
- Daily order: `FRISSIT_Planet` → `FRISSIT_Viszony` 0x50df60 → `FRISSIT_MI` 0x50a6e0 → `FRISSIT_Spy` 0x584b30 → `MI_DIPLOMACY` 0x4c7060.
- `Viszony` 0xDABF38 `[19][19]` × 0x26; +0x24 = **trade agreement**, +0x25 = level (1 war … 5 alliance). Decisions use stream 9 only; libc only picks texts. `ln(power ratio)` is rounded to f32 in the exe.
- The human's diplomacy, starmap, research, colony, design, info, spy, trade and battle orders go through the lockstep queue even in SP (`MULTIGET_*` handlers = `ig2_net::Command`).
- `DiplomaciaMikorHivtaUtoljara` 0xDAF520 `[9][9]` indexed `a·9+b` overflows into `MiTervMutato` 0xDAF668; contact-list slot −1 (0xA41DD4) overlaps `AktDipUzenet`.
- Espionage: stream 4 only; `HanyKemVan` is **0xBB23D0**; success chance f32, monthly XP ×1.5 in f64.
- AI: streams 10 and 4; mostly f32, the colonisation distance factor and `MenekulesSuly`'s far branch are f64.

### Story and campaign ([`rules/campaign.md`](rules/campaign.md))
- Stories are C code, not data: `Story_Check` 0x5c5610 runs the scripts, then clears the 17 event flags 0xF2F8AC and drains the queues with nested checks; `FRISSIT_Story` 0x5c1ff0 every tick after research.
- The daily check runs when `OldIdo + 50 < Ido` (every 51 ticks at ×1); record at ×1 too, at ×16 `Ido` never lands exactly 50 past.
- `WIN` 0x5c9770, `MEGNYERES` 0x53f6b0, `MEGHALAS`/`BirodalomMeghal` 0x53b630; `MegnyeresEllenoriz` 0x53f8c0 runs every tick after the starmap.
- `InitNewGame` 0x4e1130: start state = the exe image + the start's writes; `LoadScenario` mode 1 loads neither RandSeed nor relations.

### Client and render ([`screens/render.md`](screens/render.md), [`screens/hud.md`](screens/hud.md) §1.1)
- The exe's 2D pass is gamma-space GL: brushes blend in stored gamma space (`Blend::GammaAlpha`/`GammaAdditive`); 3D screens go through the remaster's GL layer (`Scene3d::gamma`). Tint colours into an sRGB frame need `srgb_blend`.
- Colour words are 0xAABBGGRR with A = transparency; `line2d` takes BGR words.
- Engine lights: [`screens/research.md`](screens/research.md) §2.1. `.xvm` +0x30 = object transparency when non-zero, else the first material's ([`formats/xv.md`](formats/xv.md)).
- Texts and glyph order follow the remaster's `TextServer::language` (`ig2.settings`, `locale.txt`, system locale); the 1999 `Lang` 0x912724 stays 1. Compare captures in the same language on both sides; `BirodalomNev`/`Moral_Nev` come from the Hungarian `txt17` in every language.
- `_Main` hot-spot handlers run only on button frames, never on hover; the exe takes input after the next render.
- Performance (`docs/perf.md`): `ig2-sim` `Memory` writes must stay in place; offscreen passes reuse a `Target`.

## Tracer lessons

Full flags per tracer: [`../tools/README.md`](../tools/README.md).
- Frida hooks don't fire inside our own `NativeFunction` calls: hook the game's own calls, or run staged calls through a stub on `IG2_gameUpdate`'s frame call (0x4e0523).
- Pin libc `rand()` with `srand(S)` on the game thread before a staged call; the save replay pins `srand(seed + Ido2)` per tick.
- `Multi_Send` only queues for a later frame: to run a lockstep handler at a chosen moment, patch its `Multi_Get` calls to a stub returning the staged message (byte stores for one-byte fields; respect `Multi_GetNextTipus`'s type range; end the list with 0 in a size-marker slot).
- Record byte-region diffs (init + pre/post) and compare whole raw records.
- Undo temporary fuzz writes after the call (newest first) but keep the pre-undo "known" state, so the next record's diff carries it.
- Run recordings in the foreground (a background shell stalled the game), under `--seconds 560`; record to `pending-*` and rename when done.
- Several ig2.exe instances can record in parallel; if one fails, take turns.
- A silent tracer may mean the game left the scenario: `--debug` samples every thread's stack from a second script.
- Added warships fight the human in real time and stop the clock on the battle screen: hand the human's empire to the AI (`--ships`) or set the always-simulate switch 0xB77230.
- Freeze a battle capture with `IdoSpeedSzorzoBattle` 0 (0xBAE260); `SetIdoPause` leaves the battle screen.
- Crashes: fuzzing a design slot with another hull's design; conquering a free building-less planet.
- Setting `StoryAdat+0xb0/+0xb8/+0xbc` for a whole day resets the game: set them only inside `FRISSIT_MI_FLOTTAK`.
- `rndlog` needs a source path with a backslash; x87 `st0` returns need an `X86Writer` thunk with `fstp`; `serde_json` needs `float_roundtrip` for exact f64.
- A still grab comes ~12 drawn frames after `screen.js`'s `frames` counter reaches `at + wait`.
