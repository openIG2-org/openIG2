# Deviations

A deviation is a known difference between `ig2-client`/`ig2-sim` and the GOG `ig2.exe`, or an original bug we
noticed. Classes: **open-fix** (a port difference, ours to match), **open-investigate** (cause or exe behaviour
not known yet), **keep** (the difference stays: deliberate, harmless or an original bug kept), **enhanced** (an
opt-in option proposed to the maintainers; classic keeps the original). Rule: fix only what a player would notice;
tiny pixel, timing or RNG differences are **keep** unless they change gameplay. Only live rows are listed; closed
rows (fixed, settled, not reachable, not reproduced) live in git history
(`git log -p re/deviations.md`). Row numbers are stable; a new row takes the next free number (406).

## Open: player-visible

| # | Area | What differs (ours vs exe) | Player-visible? | Spec |
|---|---|---|---|---|
| 404 | ground war, single player | `Game::ground_war` calls `ground_real_time` only when canonical and nothing ticks `WF::MI`/Refresh in single player (`grwar_mp` is canonical-only): an invasion routed to real time lands the vehicles and sits (tutorial chapter 4's ground battle can't be won) | yes | ground-combat.md §2, §11 |
| 396 | colony, building pick | ours picks a building by its footprint cell, the exe by model hit (`GetNearestBuilding` 0x47b350), so clicks on a building's upper part differ | yes | colony.md §11.3 |
| 403 | video, credits | `anim/mpeg4/misc/credits.mp4`'s first 251 frames (~10 s) don't decode with OpenH264 (IDR: bitstream error, then reference lost), so the client's credits video stays black until frame 251; the exe shows it from the start. Cause unknown (perhaps a coding feature OpenH264 lacks); found by `ig2-check` | yes | video.md §6 |

## Open: minor

| # | Area | What differs (ours vs exe) | Player-visible? | Spec |
|---|---|---|---|---|
| 20 | main menu | the fly-out (t 360) runs a little ahead at 4:3; meteor/ship RNG and pose differ | minor | mainscr.md §14 |
| 58 | SpaceWar, input | the mouse wheel and the ControlConfig interpolators (only with `ControlConfig[1]`, off by default) are not routed | minor | spacewar.md §9 |
| 102 | SpaceWar, info box | no starmap planet-info cross-fade at the opening zoom's start | minor | spacewar.md §9 |
| 104 | SpaceWar, radar | radar dots are 2×2 px at full alpha, ~1 px right/lower; the exe's are ~2×1 px at ~203 alpha with a soft vertical blend | minor | spacewar.md §9 |
| 150 | main menu, exit animation | the exe's respawning saucer (RNG) is missing and the meteors' material is fully black (faintly lit in the exe) | minor | mainscr.md §14 |
| 154 | main menu, scenario list | the picture slide transitions at t 820/880 still differ (3D RNG/pose) | minor | mainscr.md §14 |
| 157 | main menu, MP rings | the main menu's remaining MP-ring leftovers (meteor/ship sounds; the local/inet rings are our lobby, ADR 0014) are not done | minor | mainscr.md §14 |
| 213 | SpaceWar, opening zoom | the glow around the fleet marker isn't drawn during the zoom | minor | spacewar.md §9 |
| 380 | SpaceWar, end zoom-out | the battle planet is ~25 levels less red than the exe's; the cursor kind is 3 where the exe shows 2 | minor | spacewar.md §9 |
| 397 | colony, hot-spot 0x14 | with the right button held the exe skips the click and select (`DAT_00db4652`), ours doesn't; the ring timer 0x9964c4 isn't ported | minor | colony.md §11.3 |
| 398 | colony, TankView | the exe keeps TankView and TankViewLastAngle across re-entry (ours restarts); ours lacks the `+0x18c & 4` test in the unit choice | minor | colony.md §11.9 |
| 405 | colony, ground-war lock | `Colony_MouseInit2` (exe 0x467990) switches every hot-spot off in a ground war, the war buttons and view included; ours only the non-war ones (the tutorial never locks during a war) | no | colony.md §11 |

## Keep

| # | Area | What differs (ours vs exe) | Player-visible? | Spec |
|---|---|---|---|---|
| 4 | messages, diplomacy, spy wider than 4:3 | the side strips stay black; the exe shows a last-bound texture there (original bug: `Kepek[0x35]`/`[0x34]` have no texture) | minor | widescreen.md §3, §7.2 |
| 13 | ground war, retreat toggle 0x12f | ours flips the message's planet; the exe the receiver's `AktPlanet` (MP-safe, ADR 0016) | no | colony.md §9.1 |
| 14 | ground war, Detonate 0x12d | the exe reads the detonator's freed record for the burst test; ours assumes it intact | no | traces/2026-10-22-groundwar-orders.md |
| 15 | music | the music picks use a client-side rand stream, not the shared libc state (ADR 0003) | no | audio.md §4 |
| 16 | tutorial script | a quoted name across a line break hangs the exe; ours stops the tutorial (no shipped script has one) | no | tutorial.md §2, §11 |
| 32 | ground war, order 0x23 | sent as the local player; canonical MP refuses another side's unit (ADR 0011 §2) | no | colony.md §11, §9.1 |
| 38 | colony sounds | cosmetic sound picks draw from the client stream `audio::Rand` (ADR 0003) | no | audio.md §6.2 |
| 66 | design | the hull starts turning from another phase (place and size match) | no | design.md |
| 69 | Options, settings file | ours writes `ig2-rework-options.dat` beside the original and never rewrites `options.dat` (ADR 0016) | no | options.md §7, §14 |
| 70 | Options, save slots | ours saves ADR 0007's `save<n>.ig2s` (lists and loads `.sav` too) instead of `save<n>.sav` (ADR 0016) | no | options.md §5.4, §14 |
| 74 | Options, MP | the MP host keeps Save in Options (ADR 0007) | minor | options.md §4 |
| 77 | fullscreen video | the exe's synthetic Alt/Ctrl/Tab on focus gain skip a video; ours has none (not reproduced, harmless) | no | video.md |
| 82 | title video | the logo fade starts a few ms earlier than the exe's (under 0.1 s) | no | video.md |
| 84 | Options, research, design, info | glow styles and shine times come from the client's own rand stream (ADR 0003) | no | hud.md §1.2.1 |
| 98 | colony thunder | the thunder roll runs at 60 per second, not per rendered frame (tick-bound for high refresh, ADR 0016) | no | audio.md §6.2 |
| 99 | message speech, no sound card | headless: a speech lasts its decoded length | no | audio.md §7 |
| 107 | trade cadence | `Trade_Main`, quantity repeat, scroll and drags run per 100 Hz UI tick, not per rendered frame (ADR 0016) | no | trade.md §6.2 |
| 114 | design, tech models | on the first marked frame ours tints the whole model, the exe only the meshes drawn after the mark | no | design.md §12c |
| 123 | diplomacy 3D update | per-frame steps run 60 per second, not per rendered frame (ADR 0016) | no | diplomacy.md §3.1 |
| 127 | ground war, landing | place/unplace clicks travel as lockstep orders; the exe changes the field locally (ADR 0016) | no | colony.md §11.7 |
| 201 | tutorial, LOADSAVEGAME | the effect isn't ported (unused by `main.tut`; the client has no `.sav` slot load) | no | tutorial.md |
| 202 | tutorial, WAITFORATLEPES | the exe's ~10 extra ticks entering research (wall-clock load catch-up) are not modelled | no | tutorial.md |
| 223 | MP generated-galaxy start | the host's ticks seed `NEWGAME_Diplomacy` on every machine; the exe uses each machine's own (ADR 0014 §3) | no | multiplayer.md |
| 229 | every screen | an input's 2D effect shows one rendered frame earlier than in the exe (ADR 0016 §5) | no | input.md, design.md §8 |
| 235 | MP, host gone | a successor host takes over (ADR 0012); the session status lines are ours, in English | minor | game-end.md §3, multiplayer.md §7.3 |
| 248 | all screens, loads | the exe refuses keys during a screen's load catch-up ticks; ours has no such ticks (ADR 0016 §5) | no | input.md §1.1 |
| 250 | screen changes | the opening sound plays at the key, 1-2 UI ticks before the exe's (ADR 0016 §5) | no | input.md §1.1 |
| 259 | top bar | the remaster toolbar layout (`IG2_TOOLBAR`) isn't modelled; ours draws the classic bar (ADR 0016 §6) | no | input.md §6 |
| 266 | colony rename | the original copies an unset `IOStringBackup` back (last starmap rename lands on this planet); ours restores the entry's own start name | minor | colony.md (`ColonyKisInfo` ELF 0x4edb40) |
| 272 | every screen, key V | the exe draws "Not Relevant Anymore" at (10, 470) for 1 s; ours eats V and draws nothing | minor | spacewar.md §9 |
| 273 | SpaceWar, small info box | the box fader is drawn static; the exe's shines at libc-rand times | no | spacewar.md |
| 279 | all screens, loads | tooltips show during a screen's `Belep`; the exe shows none (ADR 0016 §5) | no | input.md §8 |
| 309 | messages, speech | message speech stays English under another UI language, as the exe (no localised files) | no | audio.md §5 |
| 319 | starmap, right-click order | the debug flag `DAT_0144c4cc` and the hostile-fleet owner != 0xff test aren't ported (harmless) | no | starmap.md §10.3 |
| 330 | `.xvm` material word | the exe reads uninitialised heap at +0x30 in five files; ours reads 0 (original bug) | no | xv.md |
| 356 | SpaceWar → Options → SpaceWar | the exe's ~100 starmap load ticks are skipped | no | spacewar.md §4.1.2, §9 |
| 359 | Options back after design | the design-return flags aren't modelled (no reader; research keeps its wireframe state via Messages/Help over Design) | no | options.md §3.1 |
| 369 | colony, building particles | kind 52/53 sizes and colours come from a hash, not the libc rand stream | no | colony.md §3.1 |
| 378 | MP wait overlay | no joining question (mode 2): the door admits returning humans by name and refuses newcomers | minor | input.md §9 |
| 384 | colony, rain | the exe's GL lines give ~2× the streak pixels of our thin quads (same lines, length, direction) | no | colony.md §2.6 |
| 395 | MP rename | a fleet/planet rename travels as `Command::Rename` to every peer; the exe renames on its own machine only | minor | multiplayer.md §5.6 |
| 401 | starmap, story icons 0x10-0x12 | the exe calls `Story_Special`/`FlottaMozgat`/`HajoDelete` straight from the screen; ours sends `Command::StoryIcon` through the lockstep (0x10/0x11 single player only, as the exe) | no | starmap.md §14.5 |

## Enhanced proposals (owner decides)

| # | Area | What differs (ours vs exe) | Player-visible? | Spec |
|---|---|---|---|---|
| 5 | campaign story 834 | proposal: empire 1's siege fleets start at planet 8340, not at planet −1's (0, 0) (enhanced ruleset only; classic stays bit-exact) | minor | campaign/elem.md §8.14 |
| 96 | design, rename in MP | proposal: the design rename travels as an order (the exe writes it in place: an MP desync); worth making the MP default | minor | design.md §13 |
| 133 | colony camera | ours zooms with the mouse wheel; the original has no wheel zoom (classic: none) | minor | colony.md §11.9 |
| 146 | game end, high scores | proposal: a working high-score entry after a won scenario (dead code in the exe; score = Ido − scenario_kezdesi_ido [guess]) | minor | game-end.md §6 |
| 159 | all screens, V key | a V hotkey for vsync (the original has none); also a "Vertical sync" row in Options (row 402) | minor | input.md |
| 169 | starmap, wheel | proposal: wheel zoom (and right-drag pan) on the starmap; classic has neither | minor | starmap.md |
| 197 | videos, skip | ours skips a video in any window mode; the exe only in fullscreen (`anyEvent` 0x7329d0) | minor | video.md §4 |
| 263 | overlay screens, background | the default redraws the starmap/colony live under the overlay (`LiveBackground`); classic = the exe's frozen `SaveScreen` picture (ADR 0015) | minor | overlay-background.md |
| 322 | title video | proposal: music only with the intro, not over the logos (ours matches the exe today) | minor | video.md §4 |
| 402 | options, game page | six "Remake" rows under the original's seven (display mode, resolution, vsync, 4:3 letterbox, live background, one-language names), saved in `ig2-rework-options.dat`; their names and the mode/resolution labels are English only (the game has no strings for them); captures and `--snapshot` stay classic | minor | options.md §14 |
