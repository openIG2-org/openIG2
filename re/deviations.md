# Deviations

A deviation is a known difference between `ig2-client`/`ig2-sim` and the GOG `ig2.exe`, or an original bug we
noticed. Classes: **open-fix** (a port difference, ours to match), **open-investigate** (cause or exe behaviour
not known yet), **keep** (the difference stays: deliberate, harmless or an original bug kept), **enhanced** (an
opt-in option proposed to the maintainers; classic keeps the original; listed in `docs/ENHANCEMENTS.md`). Rule: fix only what a player would notice;
tiny pixel, timing or RNG differences are **keep** unless they change gameplay. Only live rows are listed; closed
rows (fixed, settled, not reachable, not reproduced) live in git history
(`git log -p re/deviations.md`). Row numbers are stable; a new row takes the next free number (504).

## Open: player-visible

| # | Area | What differs (ours vs exe) | Player-visible? | Spec |
|---|---|---|---|---|
| 489 | starmap, modern HUD and long info | open-fix: the right button does nothing on hot-spots 9, 0xb, 0x5a-0x63, 0x65+ and 0x32; the exe takes either button (`Mouse_Lenyom`/`Mouse_Felenged`) | yes | starmap.md §15.3, §16 |
| 490 | starmap, long info | open-fix: a press on the production button's place (422..466, 432..476) leaves with the production view on any planet; the exe's hot-spot 11 is on only for the player's planet or DEBUGX (`Starmap_MouseInit2`) | yes | starmap.md §15.1 |
| 491 | starmap, long info | open-investigate: the tanks button (hot-spot 10) ends the long info and opens the vehicle transfer (`AktRopIkon` 1) in the exe; ours ignores it (classic and modern); to verify live | yes | starmap.md §15.1 |
| 492 | starmap, long info | open-investigate: a right release on the info box opens the colony screen in the exe; ours ignores it; to verify live | yes | starmap.md §15.1 |
| 493 | starmap, station sale menu | open-fix: the hover width is the design name's length; the exe uses the whole formatted line's length | yes | starmap.md §16 |

## Open: minor

| # | Area | What differs (ours vs exe) | Player-visible? | Spec |
|---|---|---|---|---|
| 488 | starmap, modern HUD, long info | open: under the modern HUD the info card's left click and its production button leave the long info on the click's release; the exe leaves on the press of hot-spot 8 / 0xb (`Starmap_Main`) | no | starmap.md §15.1 |
| 494 | starmap, radial menu | open-fix: the menu fires on the left button's release even with the right one still held; the exe waits for the last button (`Mouse_Felenged`) | no | starmap.md §14.2 |
| 495 | starmap, long info | open-fix: `TutorialPaused` doesn't switch off the settings hot-spots 0x5a-0x63 (`Starmap_MouseInit2`) | no | starmap.md §15.3 |
| 20 | main menu | the fly-out (t 360) runs a little ahead at 4:3; meteor/ship RNG and pose differ | minor | mainscr.md §14 |
| 150 | main menu, exit animation | the exe's respawning saucer (RNG) is missing and the meteors' material is fully black (faintly lit in the exe) | minor | mainscr.md §14 |
| 154 | main menu, scenario list | the picture slide transitions at t 820/880 still differ (3D RNG/pose) | minor | mainscr.md §14 |
| 157 | main menu, MP rings | the main menu's remaining MP-ring leftovers (meteor/ship sounds; the local/inet rings are our lobby, ADR 0014) are not done | minor | mainscr.md §14 |
| 507 | colony, info mode under the modern panels | open-fix: the settings drop-downs open on a click and pick on a second click; the exe opens on the press and the held item picks on the release | yes | colony.md §12.2 |
| 508 | colony, modern radar | open: on-screen zoom keys (held = numpad +/-) and a starmap key the exe lacks | no | colony.md §11.9 |
| 509 | colony, info mode under the modern panels | open: a press anywhere off the modern panels is hot-spot 0x20; the exe's 0x20 is the whole screen under its panel's dead 0x1f | no | colony.md §12.2 |

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
| 96 | design, rename in MP | a stored design's rename travels as `Command::RenameDesign` to every peer in multiplayer; the exe writes it in place on its own machine only; single player as the exe | minor | design.md §13, multiplayer.md §5.6 |
| 401 | starmap, story icons 0x10-0x12 | the exe calls `Story_Special`/`FlottaMozgat`/`HajoDelete` straight from the screen; ours sends `Command::StoryIcon` through the lockstep (0x10/0x11 single player only, as the exe) | no | starmap.md §14.5 |
| 412 | MP, full-screen videos | the machine keeps pumping its session and running ticks under a video; the exe's `st_ShowAnim` (0x602bb0) blocks, its network side unobserved | no | multiplayer.md §7.2 |
| 413 | MP, space battle end | a machine without the battle screen up (rejoin, resumed save, battle decided under Options) opens it or sends 0x44d at once (`mp_battle_check`), and the screen's end sends no pause; the exe's behaviour after a rejoin is unobserved | no | multiplayer.md §7.1 |
| 414 | MP order owner checks | in multiplayer every peer refuses starmap orders on another empire's fleet or planet (move 0x15, sell station, settings 1-4 and 8-10, rally, finish heads) and colony building orders (build, demolish, switch, priority, self-destruct, repair); the exe checks no owner (issue #24); single player as the exe | no | multiplayer.md §5.7, starmap.md §10.1, colony.md §9 |
| 415 | automatic messages | an automatic message doesn't pull the player out of a battle (screen 11), a screen ≥ 0xe, a colony at war or a pending screen change; the exe sets `AktKepKell` := 10 over all of them (its behaviour there isn't traced) | minor | info-messages.md §6 |
| 426 | MP, order delay | an order runs on every machine `delay + 1` to `delay + 2` turns after it is given (100-200 ms on a LAN; `delay` from the slowest round trip, chosen when the game starts: ADR 0007 §2); the exe applies a received order at once (its `Multi_Send` packets, no lockstep) | minor | ADR 0007 §2 |
| 424 | MP, dropped, left or kicked player | the host injects `PlayerAway` at once (outside a battle) or after the grace (real-time battle); every peer sets `PlayerKi` 1→2 and starts the AI (`init_dropped`) as the exe's 0x404 does; a rejoin (not a kick) hands the empire back with `PlayerBack` (the exe has no way back). Not mirrored: the "computer" rename, `ProgramMachines`, the red line (our "X has left the game." status line stays) | yes | starmap.md 0x404 row, multiplayer.md §7.1 |
| 429 | starmap, production counts and transfer cells | the hold repeat (`KezdoIdo`, hot-spots 0x1a..0x29 and 0x43..0x52) counts 60 frames a second, not once per rendered frame (ADR 0016) | no | starmap.md §12.4, §13.4 |
| 430 | starmap, waypoint light | `waypointvillogo` steps every second of 60 frames a second, not every second rendered frame (ADR 0016) | no | starmap.md §4.3 |
| 467 | colony, scorch marks | `Koszfolt::Refresh` draws stream 0 (`rndlog(0, 0, 99)`) once a sim tick per scorch mark while the colony screen shows them; the sim doesn't make these draws (they depend on the viewer's screen, which lockstep can't share), so later stream-0 draws (dropship heights) can differ from the exe's in a single-player game while marks are up; the client fades its marks from its own stream | no | colony.md §5 |
| 468 | colony, war effects | the ruins' two lights (`Feny`) and `Detonate`'s type-1 particles aren't drawn; scorch marks of another planet's `Detonate`/`SelfDestruct` aren't drawn on the shown planet (the exe does); tracer and beam jitter come from the client's own random stream (the exe shares libc's) and the debris' model pick from the slot | no | colony.md §5 |
| 432 | window icon | the game and the setup window, the taskbar and the program file show the remake's own drawn icon, not the original's (the project ships no original assets) | minor | - |
| 380 | SpaceWar, end zoom-out | the battle planet is ~29 levels less red than the exe's (red only; not the light, fog, cloud shells, texture or `uv_mul`) during the end zoom-out, which lasts 1.6 s (0.8 s towards options); the close-up matches; a colour tolerance that doesn't change gameplay | no | spacewar.md §9 |
| 407 | colony, building pick | the pointer's unit is decided before the building (press, hold, right button), so the leftovers (faces posed at load, a unit's model in front outside its pick circle, an unfinished building's cut-away top) only change picks on model outlines (4793 of 4800 grid points agree with the exe) | no | colony.md §10 |
| 496 | colony, big ships' charge bar (modern colony screen) | the modern panel fills the bar at 300, the timer's full value; the original divides by 200 and the bar runs past its cell (colony.md §11.1); the original's panels are unchanged | yes | ADR 0070, colony.md §11.1 |
| 497 | colony, modern panels' buttons | the modern colony screen's buttons act on the release over them (a drag off cancels), the original's on the press; the press still runs the original's code, so the order and sound are the same, a moment later | yes | ADR 0070 |
| 498 | colony, modern build menu | a building is picked with one click on its row; the original's pick list is held open under the pointer, and hovering one of its entries drops the build cursor; the end state of a pick is the same | yes | ADR 0070, colony.md §10 |
| 499 | colony, modern panels and the wheel | the mouse wheel over a modern panel doesn't zoom the view (as ADR 0065's starmap panels) | yes | ADR 0070 |
| 501 | info, modern input | the limit follows a press or drag on its bar (the original sets it whenever the pointer rests on the bar, no button needed; same order 0x5c); an empire is picked by a click, not the pointer passing over its column; the panels take the left button only (the original's rows, buttons and menus also the right) | yes | ADR 0072 |
| 502 | info, modern list | the list shows every planet or fleet of the empire, scrolling and sortable by order, name, size or alerts; the original shows four rows at a time with arrows and a scroll bar; a planet row's problem line steps by itself, not shared by four rows; a fleet's detail shows its whole design list (the original's nine) | yes | ADR 0072, info.md |
| 503 | info, modern panels | the 3D frame, its globes, heads and button animation aren't drawn in the modern info screen; off = the original's | yes | ADR 0072 |
| 505 | production view and research screen, modern panels' buttons | the modern production view's and research screen's buttons act on the release over them (a drag off cancels), the original's on the press; the count buttons and the planet list still act on the press and the same code runs, so the orders and sounds are the same | yes | ADR 0071 |
| 506 | production view, modern design list | a slot's design is picked with one click in a list beside the slot; the original's list is held open under the pointer and picks on the release; the order (0x1d(4)) and its checks are the release's | yes | ADR 0071, starmap.md §12.5 |
| 510 | starmap, modern long info | under the modern HUD the long info's exit and production plates act on the release over them, the original's hot-spots 9 and 11 on the press | yes | ADR 0079 |
| 511 | starmap, modern long info | under the modern HUD the stations' sale strips (hot-spots 0x65..) stay live at the original's place with no station list drawn | yes | ADR 0079 |

## Enhanced

The enhanced options (proposed and shipped) are listed in `docs/ENHANCEMENTS.md`, numbered from the same counter.
