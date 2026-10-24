# Deviations ledger

Every known difference between `ig2-client`/`ig2-sim` and the original GOG build, and every original bug we come
across. During the prototype (roadmap M0–M5) this list only grows: fix a difference only when it blocks play. In
M6 every entry gets a decision: **fix** (match the original), **enhanced** (an opt-in option, with an ADR), or
**keep** (the original's behaviour stays, documented).

Tags: `input` (keys, mouse, gestures), `layout` (position, size, widescreen), `render` (colour, filtering,
effects), `timing` (animation, pacing), `sound`, `feature` (missing or extra behaviour), `bug` (an original bug,
kept for fidelity unless decided otherwise). Original bugs the sim keeps on purpose for bit-exactness are already
listed in the specs ("quirks kept"); add one here only when a player would notice it.

## Triage (M6, 2026-10-24)

Every row below got a class. **fix**: a port difference, ours to match the original. **enhanced**: an
original bug or limitation worth an opt-in option (proposal; the owner decides, then an ADR). **keep**: the original's
behaviour stays, or the difference is harmless or deliberate. **investigate**: the cause or the exe's behaviour is not
known yet; settle it inside its fix group before changing code. **closed**: already done. Size: S small, M medium,
L large. The fix groups (FX1..FX16) are the queue in `docs/roadmap.md` §6.
A new row is **untriaged** until it is added to the table below with a class.

| Class | Rows |
|---|---|
| fix | 129 |
| investigate | 31 |
| enhanced | 6 |
| keep | 18 |
| closed | 38 |
| total | 222 |

| Group | Fix/keep | Investigate | Effort (rough) | Contents |
|---|---|---|---|---|
| FX1 | 10 | 2 | ~5 | Global input: `DoKeyboard` F-keys and gates, the extra letter keys out, top bar and screen-select menu on every screen, key repeat, Options/Help F-keys (rows 63, 72, 73, 109, 122, 158, 159, 160, 161, 162, 163, 184) |
| FX2 | 9 | 0 | ~2 | Pointer and tooltips in the kit: MOUSE.BMP pointer on every screen and in snapshots, hand shape, hot-spot tooltips and their 20-tick delay (rows 10, 11, 19, 48, 111, 132, 140, 171, 192) |
| FX3 | 2 | 4 | ~1 | Right button acts as left (`Mouse_Lenyom` any button): one probe, then every screen (rows 87, 176, 183, 186, 188, 193) |
| FX4 | 10 | 1 | ~3 | Starmap input: camera keys, PgUp/PgDn stepping, `c` colonise, console buttons, minimap drag, edge scroll, panel keys and scroll bar, rename, hero designs (rows 42, 43, 44, 45, 46, 164, 165, 166, 167, 168, 170) |
| FX5 | 16 | 4 | ~4 | Colony and ground war: keys, groups, camera drag, rename, pick, edge scroll, info-mode render gaps, war banner and route crosses (rows 28, 29, 31, 33, 35, 36, 37, 39, 128, 129, 130, 131, 139, 142, 143, 172, 173, 175, 177, 178) |
| FX6 | 12 | 2 | ~5 | Tutorial wiring: the screens' `TutorialCheckEvent` sites, gate flags, effects, WAITFORATLEPES, recorded keys, F-key blocks, Options layout (rows 74, 97, 141, 174, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207) |
| FX7 | 19 | 4 | ~4 | SpaceWar: render looks, keys, held rates, cross-fade, console stations, fx5 strategic clock (rows 21, 22, 24, 26, 49, 51, 55, 56, 57, 58, 102, 104, 115, 119, 120, 125, 136, 194, 195, 210, 211, 212, 213) |
| FX8 | 8 | 2 | ~2 | Design: hull-drag centre, name entry keys, held +/-, Esc with tech models, tooltips, timing (rows 62, 88, 89, 90, 92, 95, 112, 114, 181, 182) |
| FX9 | 11 | 3 | ~2 | Research, diplomacy, info, messages, spy: keys, drags, timing, fader thresholds, cage alpha (rows 6, 7, 8, 9, 85, 86, 121, 124, 145, 179, 180, 185, 187, 189) |
| FX10 | 6 | 1 | ~1 | Options and trade: re-enter the screen before, load into a war colony, slot drag, save camera, trade glass and frame phase (rows 70, 71, 108, 110, 135, 190, 191) |
| FX11 | 9 | 1 | ~2 | MainScr looks and timing: race ring, portraits, scenario list, blurb, exit zoom, name box, widescreen billboard (rows 20, 148, 149, 150, 151, 152, 153, 154, 155, 214) |
| FX12 | 8 | 4 | ~2 | Video and Help: video input edges, message clip opening, Help gates/keys/return screen/toolbar, credits glyph scale (rows 76, 78, 79, 81, 215, 216, 217, 218, 219, 220, 221, 222) |
| FX13 | 3 | 2 | ~2 | Render phases and looks: the faders' rand phase from the exe, panel models' lighting, exit plate, trade hull faces, font glyph (rows 27, 47, 134, 208, 209) |
| FX14 | 2 | 0 | ~1 | Sound leftovers: the space-war speaker rest, SkipAll stop, thunder flash, campaign start voice, other empires' build sounds (rows 12, 40) |
| FX15 | 4 | 0 | ~2 | Multiplayer: the lobby (M5), game-end teardown, MP gates and owners in the ground war, chat/console (rows 32, 34, 147, 157) |
| FX16 | 1 | 1 | ~1 | Sim and story: the campaign story tick (in progress), the tutorial start trace (rows 106, 156) |

Proposed enhanced options (awaiting the owner; classic keeps the original's behaviour):

- **#5**: Campaign 834: the siege fleets of empire 1 start at planet 8340, not at planet −1's (0, 0) (sim; enhanced ruleset only, classic stays bit-exact).
- **#96**: Design rename travels as an order (fixes the original's MP desync; worth making the MP default).
- **#133**: Colony: mouse-wheel zoom (the original has none).
- **#146**: Scenario high-score entry after a won scenario (dead code in the original; score = Ido − scenario_kezdesi_ido [guess]).
- **#159**: A V hotkey for vsync (the client's V today; the original has none; else move it to Options).
- **#169**: Starmap: right-drag pan and wheel zoom (classic: the right-click order on the press, no pan, no wheel).
- **#197**: Skip videos in any window mode (the original skips only in fullscreen).

Deliberate keeps that still need an ADR line: tick-bound rates for high refresh rates (#98, #107, #123); MP-safe
local actions (#13, #127); our own settings and save files beside the original's (#69, #70).

| # | Class | Size | Group | Note |
|---|---|---|---|---|
| 1 | closed | - | - | ADR 0010 |
| 2 | closed | - | - | umbrella; superseded by the per-screen rows 158-222 |
| 3 | closed | - | - | main menu start done; `--save` stays a dev option |
| 4 | keep | - | - | decided 2026-10-24 (black strips) |
| 5 | enhanced | M | EN | original bug; classic keeps it bit-exact |
| 6 | investigate | S | FX9 | static reading says ours is right; trace the PgUp/PgDn order first |
| 7 | fix | S | FX9 |  |
| 8 | fix | S | FX9 | with #184 |
| 9 | fix | S | FX9 | one-tick phase |
| 10 | fix | S | FX2 | kit tooltip delay: 20 UI ticks |
| 11 | fix | M | FX2 | draw the game pointer in snapshots (with #132) |
| 12 | fix | M | FX14 | the listed remaining sound sites |
| 13 | keep | - | - | ours deliberate (MP-safe); note it in an ADR |
| 14 | keep | - | - | exe reads freed memory; ours is the intended path |
| 15 | keep | - | - | ADR 0003 (cosmetic rng) |
| 16 | keep | - | - | ours stops instead of hanging; no shipped script hits it |
| 17 | closed | - | - | MainScr part 2; the MP rings are #157 |
| 18 | closed | - | - |  |
| 19 | fix | S | FX2 | same as #192/#193 |
| 20 | fix | M | FX11 |  |
| 21 | fix | M | FX7 | panel side lights |
| 22 | fix | S | FX7 |  |
| 23 | closed | - | - |  |
| 24 | fix | S | FX7 | depth test |
| 25 | keep | - | - | spin phase only (cosmetic) |
| 26 | fix | S | FX7 |  |
| 27 | investigate | S | FX13 | shared font glyph or a texture |
| 28 | investigate | S | FX5 | unverified held-button repeat |
| 29 | fix | M | FX5 | re-check after colony war part 2; overlaps #172/#173 |
| 30 | closed | - | - |  |
| 31 | fix | S | FX5 |  |
| 32 | fix | S | FX15 | MP: send as each unit's owner |
| 33 | fix | M | FX5 | selection groups into the save (ADR 0007 chunk) |
| 34 | fix | S | FX15 | MP gate; TutorialPaused with FX6 |
| 35 | fix | S | FX5 |  |
| 36 | fix | S | FX5 | depth-tested route crosses |
| 37 | investigate | S | FX5 | colony war part 2 drew most; check what is left (dropships?) |
| 38 | keep | - | - | ADR 0003 (cosmetic rng) |
| 39 | investigate | S | FX5 | not traced |
| 40 | fix | S | FX14 |  |
| 41 | closed | - | - |  |
| 42 | fix | S | FX4 |  |
| 43 | fix | S | FX4 |  |
| 44 | fix | S | FX4 | same as #165 |
| 45 | fix | M | FX4 | fleet/planet rename (IOMode) |
| 46 | fix | M | FX4 | needs `Story_Special(0x19)` |
| 47 | fix | M | FX13 |  |
| 48 | fix | S | FX2 | starmap tooltips (with #171) |
| 49 | fix | S | FX7 | port the writes, stale pointer included |
| 50 | closed | - | - | answered by #126 |
| 51 | investigate | S | FX7 | [inf] |
| 52 | closed | - | - |  |
| 53 | closed | - | - |  |
| 54 | closed | - | - |  |
| 55 | fix | S | FX7 | apply the held rates per UI tick |
| 56 | investigate | S | FX7 | [u] |
| 57 | fix | S | FX7 | typed character, not position |
| 58 | fix | M | FX7 | with #194 |
| 59 | closed | - | - |  |
| 60 | closed | - | - |  |
| 61 | closed | - | - |  |
| 62 | fix | S | FX8 |  |
| 63 | fix | M | FX1 | part of #158 |
| 64 | closed | - | - |  |
| 65 | closed | - | - |  |
| 66 | keep | - | - | turn phase only (cosmetic) |
| 67 | closed | - | - |  |
| 68 | closed | - | - |  |
| 69 | keep | - | - | deliberate: the original's file is never rewritten |
| 70 | keep | S | FX10 | ADR 0007 format kept; fix: write the camera chunks back |
| 71 | fix | M | FX10 | leave and re-enter the screen before |
| 72 | fix | M | FX1 |  |
| 73 | fix | S | FX1 |  |
| 74 | fix | M | FX6 |  |
| 75 | closed | - | - |  |
| 76 | fix | S | FX12 |  |
| 77 | keep | - | - | focus-gain skip not reproduced (harmless) |
| 78 | fix | S | FX12 |  |
| 79 | fix | M | FX12 | needs the message screen's opening |
| 80 | closed | - | - |  |
| 81 | investigate | S | FX12 | UV mapping unverified |
| 82 | keep | - | - | under 0.1 s |
| 83 | closed | - | - |  |
| 84 | keep | - | - | inherent (ADR 0003) |
| 85 | fix | S | FX9 |  |
| 86 | fix | S | FX9 |  |
| 87 | investigate | S | FX3 | one probe for every right-as-left row |
| 88 | investigate | S | FX8 | ELF only; read exe DoKeyboard |
| 89 | fix | S | FX8 |  |
| 90 | fix | S | FX8 |  |
| 91 | closed | - | - |  |
| 92 | fix | S | FX8 |  |
| 93 | closed | - | - |  |
| 94 | closed | - | - |  |
| 95 | investigate | S | FX8 | frame timing |
| 96 | enhanced | S | EN | original MP desync |
| 97 | fix | S | FX6 |  |
| 98 | keep | - | - | deliberate tick-bound rate (high refresh); ADR to write |
| 99 | keep | - | - | headless only |
| 100 | closed | - | - |  |
| 101 | closed | - | - |  |
| 102 | fix | S | FX7 | planet-info cross-fade |
| 103 | closed | - | - |  |
| 104 | fix | S | FX7 | with #22 |
| 105 | closed | - | - |  |
| 106 | fix | L | FX16 | in progress: story tick 1/4-4/4 |
| 107 | keep | - | - | deliberate tick-bound rate; ADR to write |
| 108 | investigate | S | FX10 | probably done by trade part 2; re-check |
| 109 | fix | S | FX1 |  |
| 110 | fix | S | FX10 |  |
| 111 | fix | S | FX2 |  |
| 112 | fix | S | FX8 |  |
| 113 | closed | - | - |  |
| 114 | fix | S | FX8 |  |
| 115 | fix | S | FX7 |  |
| 116 | closed | - | - |  |
| 117 | closed | - | - |  |
| 118 | closed | - | - |  |
| 119 | fix | S | FX7 |  |
| 120 | fix | M | FX7 |  |
| 121 | fix | S | FX9 |  |
| 122 | fix | S | FX1 | with #159 |
| 123 | keep | - | - | deliberate tick-bound rate; ADR to write |
| 124 | fix | S | FX9 |  |
| 125 | investigate | M | FX7 | strategic ticks during the scripted run |
| 126 | closed | - | - |  |
| 127 | keep | - | - | deliberate (MP lockstep orders); note it in an ADR |
| 128 | fix | S | FX5 |  |
| 129 | fix | S | FX5 |  |
| 130 | fix | M | FX5 |  |
| 131 | fix | S | FX5 |  |
| 132 | fix | M | FX2 | the game pointer on every screen |
| 133 | enhanced | S | EN | classic: no wheel zoom |
| 134 | fix | M | FX13 | roadmap item (material transparency) |
| 135 | fix | S | FX10 |  |
| 136 | fix | S | FX7 | calibrated size and colour |
| 137 | closed | - | - |  |
| 138 | closed | - | - |  |
| 139 | fix | M | FX5 |  |
| 140 | fix | S | FX2 |  |
| 141 | fix | S | FX6 |  |
| 142 | fix | S | FX5 |  |
| 143 | investigate | S | FX5 | [inf] |
| 144 | closed | - | - |  |
| 145 | fix | S | FX9 |  |
| 146 | enhanced | S | EN | dead code in the original |
| 147 | fix | M | FX15 |  |
| 148 | fix | M | FX11 |  |
| 149 | fix | S | FX11 |  |
| 150 | fix | M | FX11 |  |
| 151 | fix | S | FX11 |  |
| 152 | fix | S | FX11 | size-1 font accents |
| 153 | fix | S | FX11 |  |
| 154 | fix | S | FX11 |  |
| 155 | fix | S | FX11 |  |
| 156 | investigate | M | FX16 | needs a tutorial-start trace |
| 157 | fix | L | FX15 | M5 lobby |
| 158 | fix | L | FX1 |  |
| 159 | fix | M | FX1 | remove the extra letter keys (V: see the enhanced list) |
| 160 | fix | M | FX1 |  |
| 161 | fix | L | FX1 |  |
| 162 | investigate | S | FX1 | [u] exe key repeat |
| 163 | fix | M | FX1 | MP parts with FX15 |
| 164 | fix | M | FX4 |  |
| 165 | fix | S | FX4 |  |
| 166 | fix | S | FX4 |  |
| 167 | investigate | M | FX4 | [u] real hit areas |
| 168 | fix | M | FX4 |  |
| 169 | enhanced | M | EN | classic: order on the press, no pan/wheel |
| 170 | fix | S | FX4 |  |
| 171 | fix | M | FX2 |  |
| 172 | fix | S | FX5 |  |
| 173 | fix | S | FX5 |  |
| 174 | fix | S | FX6 |  |
| 175 | fix | S | FX5 |  |
| 176 | investigate | S | FX3 | [inf] |
| 177 | fix | M | FX5 |  |
| 178 | fix | M | FX5 |  |
| 179 | fix | M | FX9 |  |
| 180 | fix | S | FX9 |  |
| 181 | fix | S | FX8 |  |
| 182 | fix | S | FX8 |  |
| 183 | investigate | S | FX3 | [inf] |
| 184 | investigate | S | FX1 | [u] |
| 185 | investigate | S | FX9 | [u] |
| 186 | fix | M | FX3 |  |
| 187 | fix | S | FX9 |  |
| 188 | fix | S | FX3 |  |
| 189 | investigate | S | FX9 | effect of AktIkonKell 0x25 not read |
| 190 | fix | S | FX10 |  |
| 191 | fix | S | FX10 |  |
| 192 | fix | S | FX2 |  |
| 193 | investigate | S | FX3 | [u] needs a press/release capture |
| 194 | fix | S | FX7 |  |
| 195 | fix | S | FX7 |  |
| 196 | closed | - | - |  |
| 197 | enhanced | S | EN | classic: skip in fullscreen only |
| 198 | fix | M | FX6 |  |
| 199 | fix | L | FX6 | blocks tutorial chapters |
| 200 | fix | L | FX6 |  |
| 201 | fix | M | FX6 |  |
| 202 | fix | S | FX6 |  |
| 203 | fix | S | FX6 |  |
| 204 | fix | S | FX6 |  |
| 205 | investigate | S | FX6 | remapKeyCodes not decompiled |
| 206 | investigate | S | FX6 |  |
| 207 | fix | S | FX6 |  |
| 208 | fix | M | FX13 | dump the exe's fader structs into .ui.json |
| 209 | investigate | S | FX13 |  |
| 210 | fix | S | FX7 |  |
| 211 | fix | S | FX7 |  |
| 212 | investigate | S | FX7 |  |
| 213 | fix | S | FX7 |  |
| 214 | investigate | S | FX11 | cause unknown |
| 215 | investigate | S | FX12 | read `TTF_RenderUTF8_8Bit_Wrapped` |
| 216 | fix | S | FX12 |  |
| 217 | fix | S | FX12 |  |
| 218 | fix | S | FX12 |  |
| 219 | fix | M | FX12 |  |
| 220 | investigate | S | FX12 | probe the top bar over Help |
| 221 | fix | S | FX12 |  |
| 222 | investigate | S | FX12 |  |
| 223 | keep | S |  | ADR 0014 §3 |


## Ledger

| # | Tag | Where | What differs (ours vs the original) | Evidence | Decision |
|---|---|---|---|---|---|
| 1 | layout | all screens | the original remaster anchors/stretches some elements at 16:9; we letterbox the 800 × 600 layout | owner, 2026-10-18; roadmap §4; `re/screens/widescreen.md` | **fix** (ADR 0010, 2026-10-18): the remaster's rule is the default; at 16:9/21:9 the client already matched (2D in the 4:3 frame, 3D full width). The screen kit (`ig2-client::kit`, 2026-10-18) closed the rest: the exe-exact layout (`kit::Layout`), windows narrower than 4:3 letterboxed top and bottom, the offscreen 3D targets as wide as the window, the `--letterbox` option. **Closed** |
| 2 | input | all screens | inputs differ from the original in places (not yet listed per screen) | owner, 2026-10-18 | open (M4 audit; add entries as found) |
| 3 | feature | client start | the client starts from a save; the original starts at the title and main menu | `re/screens/README.md` §4 | partly closed 2026-10-22: the window starts on the main menu (`re/screens/mainscr.md` §14); the title video plays since 2026-10-22 (`re/screens/video.md`), a new game for `InitNewGame` (rows 17–18) |
| 4 | render | messages, diplomacy, spy wider than 4:3 | the side strips beside the 4:3 frame stay black; ig2.exe shows a dark tiled texture there. **Original bug**: `Copy3hatterkep` draws `Kepek[0x35]`/`[0x34]`, flagged loaded but without a texture (handle −1), so the quad takes the last-bound texture (the same on the three screens and at every size; no install file) | `re/screens/widescreen.md` §3, §7.2; Frida probe of exe 0x4cb690 (2026-10-24) | keep (black) |
| 5 | bug | campaign story 834 | the original's (and ours, kept) siege fleets of empire 1 appear at planet −1's record, (0, 0), and fly from there to planet 8340 (`st_fNewEx(…, 2, −1)`) | `re/rules/campaign/elem.md` §8.14 part 2e | open |
| 6 | input | research screen | PgDn: ours goes to the next category in `Terulet_Melyik` order (turrets 11 → radars 3), the exe to the previous one (turrets 11 → tanks 10); PgUp likely the reverse too. The static reading of `ResearchKeyboardHandler` (ELF 0x5e9aa0 lines 61-138) gives PgUp previous, PgDn next in `ResearchAktTeruletSorszam` order, as ours: check that order against the trace before changing either | sequence diff `research.json` t 400→420 (exe `ResearchTerulet` 11 → 10), 2026-10-22; re/screens/sound.md §4 | open |
| 7 | input | research screen | Esc: ours closes the screen with the closing animation; the exe opens screen 13 at once (no animation, no sound) | script run 2026-10-22 (`AktKep` 4 → 13 within 3 ticks of Esc) | open |
| 8 | input | research screen | the top bar's research spot (600, 0, 40 × 20) closes ours; a press there does nothing in the exe | script run 2026-10-22 (press at (620, 10): the exe stays open) | open |
| 9 | timing | research screen | the opening runs one UI tick behind: ours starts `resframe` after the first tick (`started` set by the first drawn frame's step), the exe from the first tick (4.0 at tick 10 vs our 3.6; open at tick 150 vs 151) | sequence diff 2026-10-22, re/screens/sound.md §4 | open |
| 10 | timing | research screen | the urgent button's tooltip shows 20 ticks after the pointer arrives in the exe (t 320), later in ours (the kit's 400 ms rest) | sequence diff 2026-10-22 (t 320, 340) | open |
| 11 | render | all screens (snapshots) | the exe draws the pointer in its frame; our snapshots draw none | sequence diff 2026-10-22 (t 320) | open |
| 12 | sound | all screens | resolved for the starmap, colony, info, spy, diplomacy and typer sites (audio part 2); resolved too (audio part 3): the starmap and colony speakers, message speech, alien voices, thunder; resolved too (2026-10-23): the ground war's own lines (`WF::PlaySpeechGrWar`) and the 0x12f retreat voice; open: the space-war speaker (#52), the right click's SkipAll stop, the thunder's light flash, the new campaign's start voice, other empires' builds' C[3] (`re/screens/audio.md` §8) | sound tracer 2026-10-22 | partly resolved (audio part 3) |
| 13 | bug | colony ground war, `MULTIGET_Colony` 0x12f | the retreat toggle flips the flag on the battlefield of `AktPlanet` (the receiving machine's shown planet), not the message's planet p; the port uses p (every machine shows p during a war) | `re/screens/colony.md` §9.1 | open |
| 14 | bug | ground war, Detonate (0x12d) | the victim-burst "on screen" test reads the detonator's record after `RemoveTankGroup` freed it; a reused heap block skips the burst and its libc draws; the port assumes the record intact | `re/rules/traces/2026-10-22-groundwar-orders.md` (1 of 31 detonations) | open |
| 15 | rng | music (audio part 1) | the music's libc `rand()` picks (first main track by race, ground-war track) come from a client-side MSVC-LCG stream; in the exe they advance the same libc state as gameplay draws (ADR 0003). Cosmetic | `re/screens/audio.md` §4 | open |
| 16 | bug | tutorial script, `FIND*` / `SHOW PLANET` / `WAITFOREVENT` / `TEXTURE` | a quoted name that runs across a line break loops forever in the exe (the inner copy stops on CR without advancing); the port stops the tutorial (`Effect::Hang`). No shipped script has one | `re/rules/tutorial.md` §2, §11 | open |
| 17 | feature | main menu, the rings behind it | campaign (race choice, difficulty), scenario / skirmish list, tutorial parts, multiplayer name / IP / IRC entry and the local / inet rings, credits video, exit animation with the loading plates: the exe runs them (re/screens/mainscr.md §2.2, §6.3, §6.5, §9); ours logs the request and the rings stay | input-audit 2026-10-22 | open (MainScr part 2) |
| 18 | feature | main menu, load game | exe `AktIkon` 0x25: the Options screen in load-only mode (`OptionsCsakLoadLehet`, `AktKep` 0xd); ours starts the game from `--save`'s save | mainscr.md §6.3 | fixed 2026-10-22 (options.md §14) |
| 19 | input | main menu | no hand pointer over a ring (`Mouse_Tipus` 1) and no tooltips on the box rings (`GetString(15, 1, line)`; whether the generic tooltip draws here is unverified); a box ring fires on any release over it (the exe's `Mouse_Szoveg` press/release pairing not read); the debug Enter quick start (`DEBUGLEHET` only) is not handled | input-audit 2026-10-22 | open |
| 20 | render | main menu | the top-left picture (`texcsor`) reads greener, the galaxy mesh's arms sharper and ~4 % bright, the small bottom galaxy ~10 px off, the dust points don't overlay exactly, the fly-out (t 360) runs a little ahead; frames 6.6–8.0 mean from t 180 (pass ≤ 6), 10.0 at t 360 | sequence diff 2026-10-22 (mainscr.md §13.1) | open |
| 21 | render | SpaceWar, control panel | ours lights the panel with ambient light only; the exe's panel changes colour with the moving side lights (`smMozgatMainFenyek`) | `_local/screendiff/spacewar/` 2026-10-22, `re/screens/spacewar.md` §9.3 | open |
| 22 | render | SpaceWar, planet, weapon column, radar | the planet and the weapon buttons are a little brighter in ours, the radar frustum line is fainter | `_local/screendiff/spacewar/` 2026-10-22, `re/screens/spacewar.md` §9.3 | open |
| 23 | render | SpaceWar, grid | the grid's look differs (a hard banded slab, too visible; the sequence's biggest view difference, 2026-10-23) | `_local/screendiff/spacewar/` 2026-10-22, `re/screens/spacewar.md` §9.3; fixed (2026-10-23, spec §9.6) | closed |
| 24 | render | SpaceWar, selection brackets and hp bars | ours are not depth tested (`line3` is, in the exe); the brackets are now textured (`szar.bmp`) and 1 px thin (spec §9.6) | `_local/screendiff/spacewar/` 2026-10-22, `re/screens/spacewar.md` §9.3 | open |
| 25 | timing | SpaceWar, info box model | the model's spin phase is not reproduced (not in the dump) | `_local/screendiff/spacewar/` 2026-10-22, `re/screens/spacewar.md` §9.3 | open |
| 26 | feature | SpaceWar, top bar | the speed buttons' battle state is not drawn | `_local/screendiff/spacewar/` 2026-10-22, `re/screens/spacewar.md` §9.3 | open |
| 27 | render | SpaceWar, info box text | the Hungarian "g" glyph differs ("egység" shows a Q-like g); possibly the shared font (a guess, to check) | `_local/screendiff/spacewar/` 2026-10-22, `re/screens/spacewar.md` §9.3 | open |
| 28 | input | colony ground war, held buttons | the exe likely re-runs GetNearestBuilding each frame while a button is held (Shift-box accumulates); the client acts once per press [unverified] | exe 0x4e0440, 0x47b350 | open |
| 29 | input | colony keys | numpad digits also recall groups in the client; S outside a war opens the spy screen; `c` closes the colony instead of cycling camera presets; End, arrows, numpad +/- missing | exe 0x4628c0 | open |
| 30 | input | colony ground war, cursor | cursor shapes by hover/Ctrl/Alt/armed strike | exe 0x467990; ported 2026-10-23 (colony.md §11.10) | closed |
| 31 | input | colony ground war, unit hit test | the hit test uses the hull's base point for GetObjCenter's mesh centre | colony.md §11 | open |
| 32 | input | colony ground war, order 0x23 | sent as the local player, not each unit's owner (the handler ignores the sender) | colony.md §11, §9.1 | open |
| 33 | input | colony ground war, selection groups | the groups aren't saved (the exe keeps them in bytes +0x13c..) | colony.md §11 | open |
| 34 | input | colony ground war, MP | the MP `local_22` gate of the war buttons isn't ported; TutorialPaused is read as false | colony.md §11 | open |
| 35 | render | colony ground war, attack banner | the banner starts when the screen first sees the war, not at StartGrWar; the MP banner text isn't ported | colony.md §11 | open |
| 36 | render | colony ground war, route crosses | drawn as projected 2D lines (no depth test) | colony.md §11 | open |
| 37 | render | colony ground war, missing graphics | the unit picture in the unit info, shadows, the landing zone and the dropship models aren't drawn | diff-results 2026-10-22 grwar:17 | open |
| 38 | rng | colony (audio part 2) | the cosmetic sound picks (entry wind, ambients, build, repair, demolish; audio part 3: the speakers' and alien voice picks; the ground war's hit/death/order voices) draw libc `rand()` from a client stream `audio::Rand`; in the exe they share the libc state with gameplay. Cosmetic | `re/screens/audio.md` §6.2 | open |
| 39 | input | colony | the client selects a building on a left press over the 3D view and clicks then; the exe compares the building under the pointer each frame (`Colony_Main` hot-spot 0x14), so the click's moment may differ | not traced | open |
| 40 | sound | colony | `Build3dBuilding`'s `Colony_Hangok[3]` sounds play for any building made on the shown planet in the exe; the client only for the human's own builds (its order events) | `re/screens/audio.md` §8 | open |
| 41 | feature | starmap fleet/vehicle sale menu | the release of an allowed row sends nothing to the sim (`MULTIGET_Design` 0x34/0x35 not ported); the menu, texts and the order's arguments are the exe's | starmap.md §19.7, `Starmap_Main` ELF 0x6f50a0, `MULTIGET_Design` 0x50c430 | fixed 2026-10-22: `Game::design_order` carries out 0x34/0x35 (`re/rules/design-orders.md`), the panels send `Command::Design`. **Closed** |
| 42 | input | fleet info panel, arrow keys | `StarmapKeyboardHandler` (ELF 0x704580) moves `ActInfoItem2` by 1 (left/right) / 2 (up/down) with `Kezdet` following; the port has no panel keys | starmap.md §19, ELF 0x704580 | open |
| 43 | input | starmap camera under the panels | the exe ignores the camera keys while `StarmapFleetMode`/`StarmapZoomMode`; the port's keys, wheel and right-drag pan stay live under the panels | ELF 0x704580 | open |
| 44 | input | vehicle panel, PgUp/PgDn | the exe cycles the player's planets with the panel open (First/Selected 0); the port's PgUp/PgDn tilt the camera | ELF 0x704580 | open |
| 45 | input | info box, both buttons | the fleet/planet rename (`IOMode`, id 0x16/0x15) isn't ported | `Starmap_Main` ELF 0x6f50a0 | open |
| 46 | feature | sale menu, story hero designs (`GetTervEladasOszto` < 0) | the exe shows `GetHosElkuldesStr` and calls `Story_Special(0x19, AktFleet, d)` (SP); the port shows "unable to sell" and sends nothing | starmap.md §19.7 | open |
| 47 | render | starmap info panels' models | the fleet preview's yaw is the exe's `InfoAlapRotation` mapped to pi - t [obs], its pitch and the models' material/lighting still differ (warmer, darker; the panel lights are guessed); the colony ship's cell model is smaller | diff-results.md 2026-10-22 | open |
| 48 | feature | starmap tooltips | the cells' and hot-spot 9's tooltips (`GetString(2, 1, 0x26..0x28)`) aren't drawn (general starmap gap) | `Starmap_MouseInit2` ELF 0x7024b0 | open |
| 49 | input | SpaceWar, infoszamol | the sim writes of `infoszamol` (side flag A; the Hajok +4 counter of the last ship through a stale pointer, an original bug) are not ported | `re/screens/spacewar.md` §8.7.1 item 5 (2026-10-22) | open |
| 50 | feature | SpaceWar, battle start | with a formation choice the exe stays paused until the start button and the client now matches (2026-10-23); without one the battle starts at the end of the opening zoom [inf], who unpauses is unknown | spec §8.7.5, §9.5 (2026-10-23) | open |
| 51 | input | SpaceWar, retreat window | answered on the next left release after the opening click [inf] | spec §8.7.5 (2026-10-22) | open |
| 52 | sound | SpaceWar, orders | no voice acknowledgements on select/move/attack, only the click | spec §7.1.2, §7.1.3 (2026-10-22) | **fix** (part 5, 2026-10-23): `SpaceWarSpeaker`'s lines on select, box select, move and attack (§7.4) [obs: ui5 t701 vs t703, fx t651/t713 vs t653/t714]. **Closed** |
| 53 | feature | SpaceWar, hot-spots | the hot-spot tooltips are not shown | spec §4.2 (2026-10-22) | **fix** (part 5, 2026-10-23): the shared tooltip with §4.2's strings [obs: ui5 "3. formacio" t470, "Radar" t660 at the exe's places]. **Closed** |
| 54 | input | SpaceWar, radar | ours moved the look-at only while a button was held; the exe updates on hover too (exe 0x57b9e0, no button test); the radar now follows the hover | spec §9.5 (2026-10-23) | closed |
| 55 | timing | SpaceWar, held camera buttons | yaw 0.1, tilt 0.0314 and the zoom rate are applied per rendered frame: faster than the original at high refresh rates (the screen tick itself now runs every 100 Hz UI tick like the exe, 2026-10-23; the held-button rates were not re-checked, left open) | `ig2-client` spacewar_ui, 2026-10-22 (2026-10-22) | open |
| 56 | input | SpaceWar, flag-1 hot-spots | act on the press edge (exe semantics assumed) [u] | spec §9.4 (2026-10-22) | open |
| 57 | input | SpaceWar, letter keys | a and r are mapped by physical position, not the typed character (keyboard layouts) | spec §9.4 (2026-10-22) | open |
| 58 | input | SpaceWar, keys and mouse | only Space and Esc of `DoKeyboard` are mapped; top-bar clicks, speed keys and the ControlConfig interpolators are not routed; the mouse wheel is ignored [u] | spec §9.4 (2026-10-22) | open |
| 59 | input | SpaceWar, camera targets | the ±186.7 clamp seen in grab4 was `SpaceWar_Autoscroll` with the real cursor off the window; now ported (bit-exact test `autoscroll_matches_the_exe`) | spec §2.6, §8.7.5 (2026-10-23) | closed |
| 60 | render | SpaceWar, live ship model names | taken from the exe's `tipusnevek` table; the setup's renamed station types 8/9 are not known live | spec §9.4 (2026-10-22) | **fixed** (2026-10-24): the station types 8..11 take their hull tech's research model as `terv2tipus` writes it (spec §9.10, row 144) |
| 61 | input | design (viewing) | Esc: the exe goes to the options screen at once (AktKep 0xd, no closing, no sound); the client has no options screen there yet and returns to the starmap at once | `re/screens/design.md` §4 (`DesignKeyboardEscPressed` exe 0x4b17a0) | fixed 2026-10-22 (Esc opens Options, options.md §14) |
| 62 | input | design (viewing) | the hull-drag circle: the exe measures 160 px from the hull's projected centre (GetObjCenter+TransformPoint) and refuses right after a messages/help/options return (`desMessagesUtaniLegelsoBejoves`); the client uses a fixed centre (320, 233.6) and no such test | `re/screens/design.md` §6, §14 (`Design_Main` exe 0x4b2190) | open |
| 63 | input | design | DoKeyboard's F1, F3, F4, F6–F10, p, o and m (screen changes, pause, options, messages) aren't handled on the client's Design screen (only F2, F5, Space and 1/2/3) | `re/screens/design.md` §4 (`DesignKeyboardHandler` exe 0x4b1840) | open |
| 64 | render | design | the opening/closing lasers and sparks aren't drawn (part 3) | `re/screens/design.md` §9, §14 | closed 2026-10-23, part 3 (lasers and sparks draw) |
| 65 | render | design (viewing) | the viewing-mode pod-row selection draws no DesignKeret2 frame (part 3) | `re/screens/design.md` §9, §14 | closed 2026-10-23, part 3 (the DesignKeret2 frame draws) |
| 66 | render | design | the hull starts turning from a different phase than the exe (cosmetic; place and size match) | diff-results 2026-10-22 design | open |
| 67 | input | design (editing) | part 2's unaudited input: the editing modes, buttons 5–8, 0x1f/0x20/0x33..0x52, the popups, 0x55 PodKivalaszto, the name field 0x56 (in the exe it edits the stored name directly without a `Multi_Send`: an MP desync candidate), the R/Enter keys, the 0x33/0x34 orders | `re/screens/design.md` §4, §6, §14 | audited: see 87–97 (closed) |
| 68 | render | Options, buttons and boxes under the pointer | the faders' hover glow is not drawn: `KirakFader` starts it on entering a box (`+0x3e0` = 1, a random style 1..5), `FaderDesignFrissito` fades it by 0.01 a tick, `FadeKerettelKerekitDesign` (ELF 0x545b60) draws it (the lit fill and the lines across the screen); also the slot block's running shine. Shared by every screen with `KirakFader` boxes | screendiff 2026-10-22 (options.md §14: `buttons` 6.8–11.2) | done 2026-10-23 (`kit::fader_design`, hud.md §1.2.1) |
| 69 | feature | Options, settings file | the exe writes its whole settings map back to `options.dat`; ours writes the keys the screen sets to `ig2-rework-options.dat` beside it (same format, read over the original's) and never rewrites the original's file | options.md §7, §14 | open |
| 70 | feature | Options, save slots | the exe writes `save<n>.sav` (via `safesave.sav`); ours saves ADR 0007's own format as `save<n>.ig2s` with the slot's title after the state, lists it over a `save<n>.sav` of the same slot and loads both. A saved game keeps the camera it was loaded with (the camera chunks aren't written back) | options.md §5.4, §14 | open |
| 71 | timing | Options, back to the screen before | the exe leaves and re-enters that screen (its `_Kilep`/`_Belep`: opening animations, enter sounds); ours drops Options over the screen as it was | options.md §3 | open |
| 72 | input | Options, F1..F11, M, the top bar's screen select | the exe allows them on Options unless load only or a battle runs (`GetScreenCanBeChanged` 0x550990, `DoKeyboard`'s F-key and M blocks); ours takes only Esc, O and the name entry's keys | input-audit 2026-10-22 | open |
| 73 | input | Esc on the starmap with a panel up, on the colony with its planet info | the exe closes the panel (`AktIkonKell` 0x27 / 0x3d); ours opens Options | `MainCallBackEventHandler`, options.md §5.1 | open |
| 74 | feature | Options, tutorial and battle states | the tutorial layout (wider quit, continue tutorial, hot-spot ids + 1), the multiplayer states and `OptionsHarcFolyik` (no save during a battle; Enter after a save stays when opened from a space battle) aren't wired: Options opens only from the starmap screens yet | input-audit 2026-10-22, options.md §4 | open |
| 75 | render | credits video | the remaster's credits text overlay ("REMASTERED BY GYROSCOPE...") isn't drawn (platform font) | `re/screens/video.md` | closed (2026-10-24: drawn by `video/credits.rs`; the glyph scale is fitted, #215) |
| 76 | input | fullscreen video | the client swallows the mouse/finger release while a fullscreen video plays; the exe passes 0x402/0x701 on to `FUN_004df9b0` | exe 0x7329d0; `video.md` | open |
| 77 | input | fullscreen video, focus gain | the exe's focus-gain handler injects synthetic LAlt/RAlt/LCtrl/RCtrl/Tab down+up, which skip a fullscreen video; the client has none | `video.md` | open |
| 78 | input | window close during a video | closing the window during a video quits the client; the exe's SDL_QUIT first closes the video | exe 0x4e2df5; `video.md` | open |
| 79 | render | message talking-head clip | the clip's opening wait (`mesframe` 15 to 30) and the hiding of the frame's sub-mesh 5 aren't done (the client's message screen has no opening animation) | `video.md` §5.3 | open |
| 80 | sound | message clip end | the end-of-message sound `messages_end` and the speech-ended wait switch: done in audio part 3 | `video.md`, `audio.md` §7 | resolved |
| 81 | render | message clip interlace | the interlace texture's UV mapping over the clip is assumed stretched, not verified | `video.md` | open |
| 82 | timing | title video logo | the exe's title logo fade starts a few ms later than the client's (mean diff 2.15 at the 70 ms grab; under 0.1 s) | diff-results.md 2026-10-22 video (title) | open |
| 83 | render | starmap HUD/panels/menus, fleet+transfer+tanks info | the faders' shine and texture drift (`starf`/`fifo`) aren't drawn | hud.md §1.2.1 | closed: `starmap_panels::StarmapFaders`, the exe's index map |
| 84 | render | Options, research, design, info | the glows' styles and the shine times come from the client's own rand() stream (the exe shares one stream with everything), so they differ from the exe's run to run | hud.md §1.2.1, diff-results.md 2026-10-23 | inherent |
| 85 | render | info screen | `info_screen.rs` draws the exe's `FadeKerettelKerekit` boxes 0x127/0x128 (scroll bar 295,267,10,180 and its thumb) and the 0x85 row boxes with `kirak_fader` (texture scale 0.005) where the exe uses scale 0; KirakFader(1, infof)'s thresholds are `155F+100`/`225F+30` (F = 1 - infoFelfadeles/100) where the client uses 100/30 | found 2026-10-23, hud.md §1.2.1 | open |
| 86 | input | research | the KirakFader press/release edges come from the held flag at the last drawn frame (no `Mouse_Felenged` edge) | found 2026-10-23 | open, minor |
| 87 | input | design (editing), right button | `Design_Main` tests `Mouse_Bal` only for spots 9 and 0x54, so a right press on the other buttons acts like a left one in the exe (to confirm in IG2_gameUpdate 0x4e0440); the client's right button only drags the hull | `Design_Main` 0x509140; window.rs | open |
| 88 | input | design name entry, Enter in the viewing mode | the ELF DoKeyboard ends the entry (`DesignEntertNyomott`) then falls through to DispatchKeyboard(0xd) → ModifyPlan; the client only ends the entry (ELF only; exe DoKeyboard 0x4db1a0 not read) | design.md §4, §14.1 | open |
| 89 | input | design name entry, other keys | in the exe F-keys, Space, 1/2/3 and Up/Down still act during the entry; the client's entry swallows them | design.md §14.1 | open |
| 90 | input | design name entry at 16 characters | the exe inserts and drops the last character, rejects 0x81; the client ignores the key | design.md §14.1 | open |
| 91 | input | design, deferred to part 3 | the 3D pod pick (0x55 tooltip, PlaySoundGlobal(2)), the tech models (SubZoomDes/SubZoom2Des stay 0), the 3D button press frames | design.md §12, §14.1 | closed 2026-10-23, part 3 |
| 92 | timing | design | the hull reload (be1) after the new design's release and after its OK comes one tick later in the exe (460/530 vs 459/529); with it the console's slide (its z follows AddZoomDes) | design-edit.json sounds, diff-results 2026-10-23 | open |
| 93 | render | design (viewing and editing) | the tank hulls (models 19–24) draw ~1.7× too large and at a steeper pitch than the exe (x 300..525 vs 210..605 at 800×600) | diff-results 2026-10-23 | closed: design.md §14.3 (2026-10-23) |
| 94 | render | design (editing) | the hull's POD boxes are hidden while editing; the exe draws them faint under the pods' lights (part 3) | design.md §14.1 | closed 2026-10-23, part 3 (the boxes draw) |
| 95 | render | design list frame | after a deletion / a cancelled redesign the client draws the chosen row's DesignKeret1 frame where the exe shows none at that grab (t620, t760 of design-edit.json; frame timing) | diff-results 2026-10-23 | open |
| 96 | input | design | the viewing mode's name entry writes the stored design's name in place with no order, as the exe (an MP desync: a rename order would be needed) | design.md §14.1 | open |
| 97 | input | design | TutorialCheckEvent 0xb (laser on hull tech 5) and 0xc (OK on AktAlap 6) aren't raised by the client's screen | design.md §14.1 | open |
| 98 | sound | colony thunder | the exe draws `random(400)` once per rendered frame (the rate follows the frame rate); the client draws 60 times a second (a 60 Hz display's rate), deliberate for high refresh rates | `audio.md` §6.2 | open |
| 99 | sound | message speech without a sound card | a found message speech lasts its decoded length (headless) | `audio.md` §7 | open |
| 100 | render | SpaceWar, cursor | the cursor shapes are not drawn (exe only; `MousePointerType`) | spec §9.5 (2026-10-23); the colony's pointer code (`Pictures::load_named`/blit, `Rules.mouse_hotspot`) can be reused | **fix** (part 5, 2026-10-23): `MOUSE.BMP`'s cells by `MousePointerType` (§7.4); the hover type during the zoom differs (§9.7). **Closed** |
| 101 | render | SpaceWar, box select and brackets | box rectangle drawn; brackets textured with `szar.bmp`, sizes within ~1 px (see row 24) | spec §9.5; fixed (2026-10-23, spec §9.6) | closed |
| 102 | feature | SpaceWar, info box | the small info box has no hover text (the exe shows the hovered ship) and no starmap planet-info cross-fade at the zoom's start | spec §9.5 (2026-10-23) | part 5 (2026-10-23): `kisinfokiir` reads the selection only, no hover (§7.4): that half was a misreading; the planet-info cross-fade at the zoom's start stays open |
| 103 | render | SpaceWar, opening zoom | the starmap's fade-out during the zoom (`starmapszarok`) is not drawn | spec §9.5 (2026-10-23) | **fix** (part 5, 2026-10-23): `battle_fade` (§7.4, e matches the exe's at t100/t300); the names and the radar sketch: row 138. **Closed** |
| 104 | render | SpaceWar, radar | the radar frustum is fainter than the exe's | spec §9.5 (2026-10-23) | open |
| 105 | sound | SpaceWar, sim sounds | the sim's weapon, hit and death sounds now play at the exe's ticks; voice lines stay row 52 | spec §9.5; fixed (2026-10-23, spec §9.6) | closed |
| 106 | feature | new game (`Game::new_campaign`) | the campaign stories aren't wired into `Game`'s story tick; scenario, skirmish and MP starts aren't ported | `re/rules/new-game.md` | open |
| 107 | input | trade screen cadence | `Trade_Main` and the right-button quantity repeat, the scroll and row drags run once per 100 Hz UI tick in ours, once per rendered frame in the exe | `trade.md` §6.2 | open |
| 108 | input | trade screen, Esc | goes to the options screen and back to the starmap; the exe returns to the trade screen (`Trade_Belep` with OldKep 0xd, no animation). Part 2 | `trade.md` | open |
| 109 | input | trade screen, F-keys | only F2, F5, F8 and Esc are mapped while it is up (F5 closes it animated; pressing F5 again opens design); the exe's `DoKeyboard` asks for any AktKep 1..10 | `trade.md` | open |
| 110 | render | trade screen, glass | the panels (material Racsszin, transparency 0.65) are drawn opaque at opacity 0.35 so the backdrop doesn't show through (the client's glass pass writes no depth; the exe's result is a flat racsszin x 0.35 there); exact over black, the wire sphere doesn't show through them while opening | `trade.md` §13 | open |
| 111 | input | design part 3 | `Mouse_Tipus = 1` (hand cursor) over a pickable pod / a tech model isn't drawn (the client draws no cursor shapes) | design.md §14.2 (2026-10-23); reuse the colony's pointer (colony.md §11.10) | open |
| 112 | input | design part 3 | the tech-model tooltip 0x0c (the tech's name, 30 x 30 round the pointer, `KijelolTalalmany` exe 0x4b47d0) isn't shown | design.md §14.2 (2026-10-23) | open |
| 113 | render | design | the Destroyer hull (`romb6_2`) draws an extra `Object05` body and lacks the shell's lit windows | design.md §14.2 (2026-10-23) | closed: design.md §14.3 (2026-10-23) |
| 114 | render | design part 3 | the tech models' tint differs slightly (the exe is more saturated) | screendiff t355, design.md §14.2 (2026-10-23) | open |
| 115 | render | SpaceWar, trail puffs | the trail puffs are smaller and whiter than the exe's faint grey ones | spec §9.6 (2026-10-23) | open |
| 116 | render | SpaceWar, lightning | lightning (`villam`) is not drawn | spec §6.7.4, §9.7 (2026-10-23) | **fix** (part 5, 2026-10-23): `spacewar_fx::Bolt` (§6.8.4), drawn as an added ribbon and flare. **Closed** |
| 117 | sound | SpaceWar, looped weapon sounds | the looped weapon sounds 9..12 are not played | spec §6.7.3, §9.7 (2026-10-23) | **fix** (part 5, 2026-10-23): `Fx::frame_loops` (§6.8.3); the loops start with the exe's one-shots (Paralyzator0 t2115 vs the exe's t2120, the harness's one-tick phase). **Closed** |
| 118 | render | SpaceWar, effects | debris, shield rings, fighters, planet-gun markers and puff kinds 6/7 are not drawn | spec §9.7 (2026-10-23) | part 5 (2026-10-23): debris, shield rings and kinds 6/7 drawn (§6.8); fighters and the gun markers drawn in part 5's second commit (spec §9.8). **Closed** |
| 119 | render | SpaceWar, effect particles | the effect particles are not depth tested | spec §6.7.2 (2026-10-23) | open |
| 120 | render | SpaceWar, console | the exe's orbital stations under the console and the lower-right round button are missing on the battle screen | spec §9.6 (2026-10-23) | open |
| 121 | timing | Diplomacy, closing | the closing starts one tick earlier in the client; the exe applies F2 via `AktKepKell` a frame later | screendiff t600 (2026-10-23), diplomacy.md §3.1 | open |
| 122 | input | Diplomacy, D key | the client's D key toggles the diplomacy screen; the exe has no such key (F6 opens it, F2 leaves) | diplomacy.md §6 (2026-10-23) | open |
| 123 | timing | Diplomacy, `Diplomacy3DUpdate` | the per-frame steps run at 60 per second in the client; frame-rate bound in the exe | diplomacy.md §3.1 (2026-10-23) | open |
| 124 | render | Diplomacy, cages | the client maps the cages' 1→0.5 transparency to wire alpha 0→1 (calibrated to the rest look) | diplomacy.md (2026-10-23) | open |
| 125 | sim | SpaceWar, fx5 capture | on a planet battle with weapons 9..12 (`run_spwc.py --planets --fuzz-designs --weapons 0x1a,0x1b,0x1c,0x1d`) the client's live battle loses no ship by t2200 while the exe's loses ships from t1600; the client's strategic clock and money also move (9:20 → 9:27, $100000 → $90000) while the exe's stand | spec §9.8 (2026-10-23) | open: check the scripted run's strategic ticks against the exe's |
| 126 | input | SpaceWar, start | without a formation phase (fewer than 6 of the human's ships, or MP) nothing on the battle screen unpauses the battle: the top bar's pause button does; the client started it at the zoom's end | spec §7.4 (2026-10-23) | **fix**: the client now waits for the pause button. **Closed** |
| 127 | input | colony ground war, landing | the landing clicks (PlaceTank / UnplaceTank / PlaceAllTanks) travel as our own lockstep orders (`GroundOrder::{Place, Unplace, PlaceAll}`, message kinds 0x10000..=0x10002); the exe changes the battlefield locally with no message (deliberate, for MP) | colony.md §11.7 (2026-10-23) | ours |
| 128 | input | colony 3D view, pick | a ray that misses the ground keeps the last cell in the port; the exe zeroes the pick point (cell `Real2Grid(0, ·)`, exe 0x49f130 at 0x772) | colony.md §11.7 (2026-10-23) | open |
| 129 | input | colony edge scroll | runs only while the pointer is over the window; the exe keeps scrolling from the last physical position | colony.md §11.9 (2026-10-23) | open |
| 130 | render | colony unit info | the turning unit model is drawn ~36 px lower and smaller than the exe's (the camera-fixed property 0x2000000's placement is not fully specified); the building's small model (`ColonyInfoMod` 2) is still not drawn | colony.md §11.10, screendiff grwar:17 bottom_info 15.31 (2026-10-23) | open |
| 131 | input | colony camera, `c` mode 3 | the TankView mode keeps the normal camera | colony.md §11.9 (2026-10-23) | open |
| 132 | render | pointer | the game's pointer (MOUSE.BMP) is drawn on the colony screen only; the other screens keep the window's pointer | colony.md §11.10 (2026-10-23) | open |
| 133 | input | colony camera, mouse wheel | the port's mouse wheel zooms; the original has no wheel zoom in the colony | colony.md §11.9 (2026-10-23) | open |
| 134 | render | Trade, hull model | the hull's faces turned from the light are mid-grey in the client, near-black in the exe (not the self-illum pass, not unrotated normals: both tried); model region 4.4–4.9 | trade.md §13 part 2, `trade-pods` (2026-10-23) | open |
| 135 | state | Trade, row frame | the row frame's first-morph phase (`dkm` 0xB34C0C, `dkmcel` 0x903548, `GfxNovelo` 0xB34BAC) is one global in the exe, never reset between screens; the client keeps it per screen instance and carries it only trade↔options | trade.md §13 part 2 (2026-10-23) | open |
| 136 | render | SpaceWar, fighters | the fighter model's size is a calibration (a quarter) and the point colour 0x2c2c2c | spec §9.8 [obs fx5 t1000] (2026-10-23) | open |
| 137 | sim | SpaceWar, formation | picking a formation did not re-place the side (exe `loadfelallas` -> `felallit`, stream-6 draws) | spec §9.7, §9.8 (2026-10-23) | **fix** (2026-10-23): `Battle::pick_formation`, bit-exact on 29 exe picks (`formation_pick_matches_exe`), spec §9.9. **Closed** |
| 138 | render | SpaceWar, opening zoom | the starmap names and the radar's starmap sketch are not drawn during the zoom (exe t100) | spec §9.7, §9.8 (2026-10-23) | **fixed** (2026-10-24): the names, the short info and the minimap faded as `StarmapGfx2D_1`/`_3` do (spec §9.10) |
| 139 | render | Colony, info mode, bottom-left short info | the planet picture is not drawn (shared gap with the starmap's info box) and the fill is darker than the exe's (the colony's faders keep no FaderDesign shine/hover state; in the info mode the short info also takes the fader index one higher); bottom_info 17.0-17.6 | colony.md §12, `colony:42 --colony-info` (2026-10-23) | open |
| 140 | input | Colony, info mode | no tooltips for hot-spots 0x15..0x20 (GetString(3,1,0x10..0x1a), exe Colony_MouseInit 0x4680e0), and none on the colony screen at all | colony.md §12 (2026-10-23) | open |
| 141 | input | Colony, info mode | no TutorialCheckEvent(0x24/0x25) for tax -/+ and the tax/auto-build picks; tutorial opcode 5 (Colony_BELEP_ABBA_A_KIBASZOTT_PLANETINFOBA, exe 0x4666d0) has no entry point | colony.md §12 (2026-10-23) | open |
| 142 | input | Colony, info mode | a right-button release over an open menu after the pointer moved does not reach the menu (window.rs forwards a right release only without a move); the exe takes any release | colony.md §12, window.rs (2026-10-23) | open |
| 143 | input | Colony, key I | the exe's ColonyKeyboardHandler (0x4628c0) tests the character (0x49/0x69); a scripted SDL key I did not toggle the exe's mode [inf]; the client toggles on every I key press including auto-repeat | colony.md §12 (2026-10-23) | open |
| 144 | render | SpaceWar, planet stations | the planet's stations (ship types 8..11) draw no model and can't be hovered: the exe fills `tipusnevek[race·12 + 8..11]`' names at load (compiled "TSCtablazatabol"; seen `ur_kurbazis.xvb`/`ur_urbazis.xvb`, the tech models 0x4c/0x4d), the writer is unknown; the client takes the compiled names | spec §9.9, §9.7 (2026-10-23) | **fixed** (2026-10-24): the writer is `terv2tipus` (exe 0x582010), copying `TTeruletek[tech − 1]` for hull techs 0x4d..0x50; `spacewar_screen::type_model` (spec §9.10) |
| 145 | sim | Messages, disabled answer | the exe retries a disabled answer every frame while `VoltUjValasz` stays 1; the port tries once per order | info-messages.md §7.3 (2026-10-23) | open |
| 146 | feature | Game end, high scores | the original never opens the high-score name entry after a won scenario (dead code: `scenario_nevbevitelkell` 0xBAD10C never set, `scenario_score` 0xBAD110 never written); ours neither. A working entry with score = Ido - scenario_kezdesi_ido [guess] is an enhanced option | game-end.md §6 (2026-10-23) | open, decision open |
| 147 | feature | Game end, multiplayer | `ExitToMainScreen`'s session teardown (nam:, stat:0, sockets) is not ported in the client's game-end path | game-end.md (2026-10-23) | open |
| 148 | render | MainScr, race choice | the head-ring frame is flatter and darker than the exe's lit stone ring; the ring label NORMAL (difficulty) is overdrawn by its ring | mainscr.md §13.2, diff t 610 (2026-10-23) | open |
| 149 | timing | MainScr, race blurb | the exe types the blurb 2-4 characters ahead of the client at the same tick | mainscr.md §13.2 (2026-10-23) | open |
| 150 | render | MainScr, exit animation | mid-zoom (t 1650-1700): the galaxy and the meteor/ship lighting differ and one ship is missing; the `forgaspont` pivot is a guess with no visible effect | mainscr.md §13.2 (2026-10-23) | open |
| 151 | render | MainScr, race portrait | keps 65..71 (`_ESIG_*`) have no remaster file; the client draws `textures/main/fejek2/<race>.bmp`, the same head but larger than the exe's | mainscr.md §13.2, §9.3 (2026-10-23) | open |
| 152 | render | MainScr, scenario list | the size-1 font's accented letters (exe draws "JATEKOS"/"BOLYGO" without accents) are garbled in the client | mainscr.md §13.2 (2026-10-23) | open |
| 153 | render | MainScr, scenario list | the selected row's frame looks stronger in the client | mainscr.md §13.2 (2026-10-23) | open |
| 154 | timing | MainScr, scenario list | the picture slide runs a few ticks off the exe's (in ~5 ticks ahead, out ~2 behind); transitions t 820-880, 1050, 1420-1600 at 7.8-11.4 | mainscr.md §13.2, diff-results.md (2026-10-23) | open |
| 155 | timing | MainScr, multiplayer name entry | after Enter the exe keeps the empty name box, its title fading out, for ~150 ticks; the client closes it at once. Typing itself is not compared (the exe's name started empty: the user's registry `MultiplayerNev`) | mainscr.md §13.2 (2026-10-23) | open |
| 156 | sim | MainScr, tutorial start | `Game::start_tutorial` itself is an untraced composition of verified parts; the script and overlay now run on start (tutorial.md, 2026-10-23) | mainscr.md §14, tutorial.md §7 (2026-10-23) | open |
| 157 | sim | MainScr, MP rings | the local/inet rings log (M5 lobby not built); no high-score name entry, IP/IRC entry or disconnected box, no meteor/ship sounds, no demo | mainscr.md §14 (2026-10-23) | open |
| 158 | input | all game screens, F-keys | `DoKeyboard` asks for `AktKep = key − 0x6f` on F1..F11 (the global part of #63, #72, #109); the client now maps F3..F10 the way its letter keys go, but F1 (help, not built), F2 from the starmap/colony/research/info/messages/spy screens, F11 (battle, only with a battle-pending flag) and the gates (not the screen it is on, `Belep`/`Atlepes` pending, the per-VK tutorial table `DAT_00f66ebd`, the messages screen only when idle, design/trade keep their own) are not mirrored | exe `DoKeyboard` 0x4db1a0 lines 191-272; window.rs `key` (2026-10-23, input audit) | open |
| 159 | input | all game screens, letter keys | the client opens screens with S, K, I, C, D, cycles the info chart with Tab and toggles vsync with V; the exe has none of these as global keys (its letters go to the screen's handler; V only stamps `DAT_00b772c0`), and they shadow the screens' own letters (colony `i`/`e`/`a`/`s`, messages `d`, starmap `c`) | exe `DoKeyboard` 0x4db1a0 lines 52-136, `FUN_004db040` 0x4db040; window.rs `key` (2026-10-23, input audit) | open |
| 160 | input | all game screens, top bar | the pause and speed hot-spots (`AktIkon` 2..5) act on every game screen but the messages and options screens in the exe; the client has them on the starmap only (and SpaceWar's own pause) | exe `IG2_gameUpdate` 0x4e0440 lines 143-181; window.rs `starmap_press` (2026-10-23, input audit) | open |
| 161 | input | all game screens, screen-select menu | missing: hot-spot 6 (left press opens the drop-down, 126 px wide, 16-px rows from y 20, screens 1..N without 11 and 12; release picks, refused by `DAT_00f66f2c`; a right press asks for the starmap), hot-spot 7 (opens the messages screen), hot-spot 8 (the MP player menu, `Multi_Send` 0x29/0x2a), the history drop-down and the bar's tooltips | exe `MOUSE_SelectScreens` 0x555470 lines 23-238 (2026-10-23, input audit) | open |
| 162 | input | all game screens, key repeat | the client takes OS key-repeat presses as new presses on every screen but SpaceWar (a held O toggles the options screen, held arrows repeat); whether the exe's event queue (`FUN_004e3660` 0x4e3660) repeats is not traced [u] | window.rs `window_event` KeyboardInput; exe `DoKeyboard` 0x4db1a0 (2026-10-23, input audit) | open |
| 163 | input | all game screens, other `DoKeyboard` keys | `0` (`FUN_00614700`), the backquote console (`ExecuteString`, with `DEBUGLEHET` or in MP), Enter's chat entry (MP), the MP pause/speed messages 0x406/0x407 with their host gate, and the key-up calls to the screens' handlers are not mirrored | exe `DoKeyboard` 0x4db1a0 lines 115-134, 276-315 (2026-10-23, input audit) | open |
| 164 | input | starmap, camera keys | the exe holds ←/→ (yaw ∓0.02 a 10 ms frame), ↑/↓ (pitch ∓0.00628318, clamped to [−1.560795, −0.2]), numpad + / − (zoom `±(640 − Zoom)/50`, [2, 580]) and `=`/`-` (a 20-frame step) in `StarmapKeyboardDoModifies`; the client turns with Q/E and tilts with PgUp/PgDn instead | ELF `StarmapKeyboardHandler` 0x704580, `StarmapKeyboardDoModifies` 0x705630; window.rs `key` (2026-10-23, input audit) | open |
| 165 | input | starmap, PgUp/PgDn | the exe steps the selection to the previous/next own planet (any known planet with Shift) or own fleet and centres the view on it (`StarmapKozepreAllit`), not while a menu or panel is up; the client tilts the camera | ELF 0x704580 (0x21/0x22); window.rs `key` (2026-10-23, input audit) | open |
| 166 | input | starmap, `c` | the exe's lower-case `c` sends the colonise order 0x16 (kind 1) for a free planet with a colony ship in the pool and none on the way; the client's C opens the colony screen | ELF 0x704580 (`param_2 == 99`); window.rs `key` (2026-10-23, input audit) | open |
| 167 | input | starmap, console buttons | hot-spots 1-6 (top view, rotate held, side view, zoom held, pitch held; left/right pick the direction) are not wired, only drawn; their rectangles in `Starmap_MouseInit` are placeholders, the real hit areas are not found [u] | ELF `Starmap_Main` 0x6f50a0 lines 163-257, `Starmap_MouseInit` 0x6feb70 (2026-10-23, input audit) | open |
| 168 | input | starmap, minimap and edge scroll | the exe retargets the view on the minimap press and every frame while held (a drag, 4-frame tween); ours acts once on the release, unmoved, at once. `Starmap_Autoscroll` (within 8 px of the edge, no button, not in the long info) is missing on the starmap | ELF `Starmap_Main` lines 259-285, 0x6fe890; window.rs `starmap_press`/release (2026-10-23, input audit) | open |
| 169 | input | starmap, right button and wheel | the exe gives the right-click order (0x15, rally 0x400) on the press; ours on the release without movement, because the right drag pans the camera and the wheel zooms, neither with a known exe counterpart (`StarmapGestureHandler` 0x705a30's caller not found) [u] | ELF `Starmap_Main` lines 725-911, 0x705a30; window.rs `mouse` (2026-10-23, input audit) | open |
| 170 | input | starmap panels, scroll bar 0x57 | the exe re-evaluates the transfer, fleet and tank panels' scroll bar every frame while held (a drag); ours once on the press | ELF `Starmap_Main` lines 1825-1880; starmap_panels.rs `press`/`hold_frame` (2026-10-23, input audit) | open |
| 171 | input | starmap, tooltips | the hot-spot tooltips and the radial icons' names (`StarmapRopIkon_Nev`) are not drawn | ELF `Starmap_Main` 0x6f50a0; starmap.md §9, §14.4 (2026-10-23, input audit) | open |
| 172 | input | colony, End | End toggles the building names (`DAT_00996468`, `SetOptions`, saved) in the exe; the client has no End key and always draws them | exe `ColonyKeyboardHandler` 0x4628c0 line 284; colony.rs `names` (2026-10-23, input audit) | open |
| 173 | input | colony, digit groups | 4..9 (Ctrl saves) act by virtual key in or out of a war; the client reads the character (numpad digits count) and only in a war | exe 0x4628c0 line 26; window.rs `colony_war_key` (2026-10-23, input audit) | open |
| 174 | input | colony, tutorial locks | `a`, `c` and End stop at the tutorial lock `DAT_00f66ff9`; E (hot-spot 0x11) checks `DAT_00f66f2a`; PgUp/PgDn raise `TutorialCheckEvent(0x1b)` and test Shift (any known planet) and the placing gate `FUN_0047be90`; the client tests none | exe 0x4628c0 lines 132-199, `Colony_Main` 0x4666d0 line 316 (2026-10-23, input audit) | open |
| 175 | input | colony, E with a fortress turret | the exe sends 0x132 for the selected fortress turret's building (`OnlyOneFortressSelected`, also enabling 1, 2 and 0x11); the client needs a selected building | exe 0x4628c0 line 74, `Colony_MouseInit2` 0x467990 line 43; colony_ui.rs `Key::Priority` (2026-10-23, input audit) | open |
| 176 | input | colony, right button on hot-spots | `AktIkon2` is set by any held button and only hot-spot 4 tests left/right, so a right press on 1, 2, 5, 6, 8, 10, 0xb..0x13 acts like a left one in the exe [inf, with #87]; the client acts on the left only | exe `IG2_gameUpdate` 0x4e0440 line 61, `Colony_Main` 0x4666d0; colony_ui.rs `press`, colony_war.rs (2026-10-23, input audit) | open |
| 177 | input | colony, planet rename | both buttons held on the short info (hot-spot 4) start the planet name entry (`IOAzonosito` 0x1f, 15 characters, own planet, not under the tutorial lock); the client has no rename on the colony screen | exe `Colony_Main` 0x4666d0 lines 64-125 (2026-10-23, input audit) | open |
| 178 | input | colony, camera drag | the exe turns and tilts with both buttons held (`−dx/200`, `+dy/200`) and a left drag only box-selects in a war; the client turns with the right drag (which also gives the war order on the press) and pans with the left drag on peaceful planets | colony.md §2.1, §10, §11.3; window.rs `cursor_moved`, colony_ui.rs `press` (2026-10-23, input audit) | open |
| 179 | input | research screen, drags | the ball spins with the mouse while both buttons are held within 160 px of it (`resGyX/Y = 0.9·old + (MouseOld − Mouse)/30·0.1`, ×0.8 a frame; the idle spin stops), and a press on the chosen model grabs it (`(MouseOldX − Mouse_x)/100`) until release; the client has only the idle spin and the selection | ELF `Research3DUpdate` 0x5e4e10 lines 453-466, `GombfolottLenyomva` 0x5e2190, `TalalmanyKivalasztva` 0x5e24a0, `KellMozgatni` 0x5e36e0; research.rs (2026-10-23, input audit) | open |
| 180 | input | research screen, held Enter/D | the exe's `oldEnterlenyomva`/`oldDlenyomva` are rewritten by every handler call, so a held Enter or D fires again after any other key event; the client fires once a press | ELF `ResearchKeyboardHandler` 0x5e9aa0 lines 139-140 (2026-10-23, input audit) | open |
| 181 | input | design (editing), held +/− | the exe repeats only on frames where `AktIkon2 == id`, with four counters (0xa41be8, 0xa41bec, 0xa41bf0, 0xa41bf4, each reset by a press of its own kind), and keeps the 3D button pressed (`DesMelyikGombot`/`DesGombNew`) on every held call; the client keeps one counter, repeats after the pointer leaves and re-triggers the button only on repeat steps (design.md §6.1's "one shared counter" is wrong) | exe `Design_Main` 0x4b2190 (0x4b25xx-0x4b2d); design_edit.rs `edit_held`, `push_button` (2026-10-23, input audit) | open |
| 182 | input | design (editing), Esc with the tech models out | `DesignKeyboardEscPressed` also folds the models (`SubZoomDirDes = −1` or `SubZoom2Des = 1000`, `InitiateDelay = 0`, `RemoveHozzavaloTalalmanyok`, the pod-select modes 0); the client only cancels the edit | exe 0x4b17a0; design_edit.rs `escape` (2026-10-23, input audit) | open |
| 183 | input | diplomacy, right button on the menus | `DiplomGfx2D_1`'s menu edges are `Mouse_Lenyom`/`Mouse_Felenged` (any button) and only the cascade pick needs `Mouse_Bal`, so a right press opens a menu [inf]; the client ignores the right button there | exe `DiplomGfx2D_1` 0x4b89e0; window.rs `mouse` (2026-10-23, input audit) | open |
| 184 | input | diplomacy and research, top bar's research spot | a press on (600, 0, 40 × 20) opens the research screen from the diplomacy screen in the client; whether the exe's top bar is live on screen 6 is not checked [u] (#8 is the research side) | window.rs `top_bar_research` (2026-10-23, input audit) | open |
| 185 | input | spy screen, Esc and top bar | spy.md §8 says Esc closes the screen; the client opens the options screen (`ESC_Pressed`), unchecked against the exe; the top bar's pause/speed buttons are unreachable while it is up (spy re-uses ids 3/4 for kill and recall) [u] | spy.md §8; exe `Spy_MouseInit` 0x58f0f0; window.rs `key`, `mouse` (2026-10-23, input audit) | open |
| 186 | input | info screen, buttons | `Mouse_Lenyom` is any button, so a right press on 9..=12, 14..=25, 30/31, 35/36 and 37 acts in the exe; ours reaches only the triangle and the menus. The scroll bar 34 follows the pointer every frame while held (ours: on the press); the star-log arrows 35/36 step every frame the pointer is over them (ours: once a press); the arrows animate on hover | exe `Info_Main` 0x4e81c0 (0x22..0x24 branches); info_screen.rs `press`, window.rs `info_button` (2026-10-23, input audit) | open |
| 187 | input | info screen, held hot-spot and list height | `Mouse_Megfogva` pins `AktIkon2` during a triangle or limit drag, so passing over a race column doesn't switch the empire; hot-spots 13 and 37 are `56·min(n, 4)` high (ours: 4 rows, so 37 fires below the last row when n < 4); no hot-spot tooltips (`GetString(9, 1, k)`) | info.md §5; info_screen.rs `hover`, `spots` (2026-10-23, input audit) | open |
| 188 | input | messages screen, right button | a right press in the text box also sets `SkipAll` (`DAT_00d90dc4 = 1`), and a right press on an answer, arrow, row or leave spot works as a press (`Mouse_Lenyom`); the client takes the left button only | exe `Messages_Main` 0x508560, `FUN_00509d10`; window.rs `mouse` (2026-10-23, input audit) | open |
| 189 | input | messages screen, Esc and the answer spots | when idle, Esc sets `AktIkonKell = 0x25` (effect not read; ours leaves the screen); hot-spots 4 and 6 are on only at `mesframe` 30 (ours: always) | `MainCallBackEventHandler` (ELF), exe `Messages_MouseInit2` 0x508940; window.rs `key` (2026-10-23, input audit) | open |
| 190 | input | options, Load | the exe loads straight into the colony screen of a planet with `GetWarfareStatus == 1`; ours always goes to the starmap. Id 1 fires without the `Mouse_Lenyom` test the save and quit ids have (no visible difference known) | exe `Options_Main` 0x51a320; window.rs `options` load path (2026-10-23, input audit) | open |
| 191 | input | options, slot list held | the slot pick runs every frame while the button is held and sets `Mouse_Megfogva` (AktIkon stays 4; dragging out keeps picking by y, clamped 1..24); ours picks only on moves over the list. `DEBUGX` lets Save work in a battle | exe 0x51a320 (id 4), 0x4e0440 lines 51-64, `Options_MouseInit2` 0x51a8a0; options.rs (2026-10-23, input audit) | open |
| 192 | input | MainScr, rings | the pointer becomes the hand (`Mouse_Tipus = 1`) over a ring; the rings and boxes carry `GetString(15, 1, n)` tooltips; neither is in the client | exe `Mainscreen_Update3D` 0x500270, `MainScr_MouseInit` 0x4ff980, mainscr.md §6.1-§6.2 (2026-10-23, input audit) | open |
| 193 | input | MainScr, buttons and timing | the box actions (scenario list, race heads, difficulty rings) test only `Mouse_Lenyom`, so the right button works too (the client drops it); the box rings (`Mouse_Szoveg_Set(.., 1, 1)`) may fire on the press, not the release as mainscr.md §6.3 and the client have it, and 0x27 may toggle every held frame [u: needs a press/release capture] | exe `MainScr_Main` 0x4fefc0, 0x500270, design.md §2; mainscr.rs `press`/`release` (2026-10-23, input audit) | open |
| 194 | input | SpaceWar, `DoKeyboard` keys | 1/2/3 also set the speed and unpause (sound 10) besides the group pick; P pauses; O opens the options screen; the exe has no Esc on this screen (ours leaves with the closing zoom); with `DEBUGX` F1..F11 and M work. #58's "Space and Esc of `DoKeyboard`" is wrong: Esc isn't one | exe `DoKeyboard` 0x4db1a0, `SpaceWarKeyboardHandler` 0x561ef0; window.rs `spacewar_key` (2026-10-23, input audit; screen, reported only) | open |
| 195 | input | SpaceWar, handler keys | g/G (sets or negates `+0xc98`) and ScrollLock (`e41798`) aren't mapped; with `DEBUGX`, q (`initgfx`) and u (AI control of my side) neither; r is gated on `DAT_00f66f2a` (ours `TUTORIAL_PAUSED`) and Backspace blows up once per ship (`+0x1b0`) of each selected unit (ours one order per unit) [u] | exe 0x561ef0 lines 213-269; spacewar_ui.rs (2026-10-23, input audit; reported only) | open |
| 196 | input | help screen (F1) | built (2026-10-24, `help.rs`, F1 in window.rs): F1, hot-spots 1-6 (`Help_MouseInit` 0x4d7640), the scroll bar 0x22, back (6). The line-scroll arrows 0x1f/0x20 are dead in the exe too (nothing reacts); there is no help keyboard handler. The open input differences are #200-#206 | exe `Help_Main` 0x4d7510, `Help_Belep` 0x4d7060; `re/screens/help.md` (2026-10-23, input audit; screen) | closed |
| 197 | input | videos, skip | the exe skips a video only in fullscreen (`anyEvent` 0x7329d0); ours in any window mode | video.md §4; window.rs video input (2026-10-23, input audit; reported only) | open |
| 198 | input | tutorial, live input and gates | while a tutorial runs the exe drops the live mouse and modifier keys and plays the recorded ones (`Mouse_Tutorial_Kezel` 0x510930, the pointer parked at (320, 200)), blocks F-keys by `DAT_00f66ebd` and `KepernyokLetiltva`, greys the screen-select entries, raises events 0x1c (F3) and 3/4 (Enter on the starmap), takes `t`/`o`/`j`, and reads the starmap, colony and building locks; the ground-war buttons and the retreat window get `tutorial_paused: false` | tutorial.md §5-§6; window.rs, render.rs, ui.rs (2026-10-23, input audit; overlay, reported only) | partly fixed 2026-10-24: the overlay drops the live pointer, buttons and modifiers, plays the recording, takes `t`/`o` (tutorial.md §3); the F-key/screen blocks, greyed entries, events 0x1c/3/4, locks and `tutorial_paused` stay open (#199–#201) |
| 199 | sim | Tutorial, screens | the screens' `TutorialCheckEvent` call sites listed in tutorial.md §4 are not raised; a chapter stops at the first WAITFOREVENT on one | tutorial.md §4 (2026-10-23) | open |
| 200 | sim | Tutorial, gates | the gate flags (rules/tutorial.md §5: disabled screens, buildings, split, production, sell ships, exit button, starmap/colony locks, demolish, new spies, spy mission) are not read by the client's screens | tutorial.md (2026-10-23) | open |
| 201 | sim | Tutorial, effects | not carried out: new fleets (KILLER/KOLONIZALO), destroyers to trade, IRRITATEME, SEND MESSAGE, LOADSAVEGAME, DOUBLEGOMB, KURVAANYAD planet info, GOTO SCREEN, music ducking (GlobalMusicVolumeCsokkento), the overrun debug flags | tutorial.md (2026-10-23) | open |
| 202 | sim | Tutorial, WAITFORATLEPES | the client reports no screen transition (Atlepes/Belep) | tutorial.md (2026-10-23) | open |
| 203 | render | Tutorial, pointer | the replayed pointer is drawn as MOUSE.BMP cell 0, not the hot-spot's Mouse_Tipus shape | tutorial.md (2026-10-23) | open |
| 204 | input | Tutorial, recorded keys | key-ups (kind 2) are dropped and only characters + Backspace/Enter/Esc/PgUp/PgDn/arrows/F1-F12 downs are mapped (held-key logic sees one-shot presses) | tutorial.md (2026-10-23) | open |
| 205 | input | Tutorial, t/o keys | the exe's extra guards (FUN_004c99a0 window-active and IOMode 0xbae2c3 == 0) and t/o while on the options screen are not modelled; remapKeyCodes (exe 0x4e4670) is not decompiled, so the code space of t/o/j and the recorded slots is unconfirmed | tutorial.md (2026-10-23) | open |
| 206 | input | Tutorial, wheel | the wheel is dropped while the script plays (the exe's wheel path under the tutorial is unverified) | tutorial.md (2026-10-23) | open |
| 207 | render | Options, tutorial layout | the options screen's tutorial layout (wider quit, "continue tutorial") is not drawn | tutorial.md (2026-10-23) | open |
| 208 | render | starmap fleet/transfer/tanks info panels | the `fifo` faders' shine and texture drift phase isn't the exe's (its rand() stream isn't replayed; at ui-frame 240 fader 0's offset is (149, 64)), so stills differ by the phase: transfer left_panel / fleetinfo list / tanksinfo list ~10.7 vs ~5.1 phase-free (s pinned 0.9, offset 0 brings every record back); fix: screen.js dumps the exe's fifo/starf `faderstruktura` into the .ui.json for the client | M3 sweep 2026-10-24 (bisected to 741e3e7) | open |
| 209 | render | starmap fleet info, exit plate | the exit plate is darker than the exe's (luma exe 40.6, client 35.1): exit 6.42, frame 6.99 (recorded 2.97 / 4.70 came from another build; not reproducible from the stored capture at any commit) | M3 sweep 2026-10-24 | open |
| 210 | input | SpaceWar, Space during the zoom | the exe's Space pauses only while `nemlehetidotallitani` (exe 0xBAD0D4) is 0, so not during the opening zoom; the client's Space pauses at any time (only the top bar's pause button waits for the zoom) | exe `DoKeyboard` 0x4db1a0; window.rs `key`, spacewar_live.rs (2026-10-24, input audit) | open |
| 211 | input | SpaceWar, camera cluster | the exe turns the hot-spots 0x46..0x4b (and their tooltips) on only while `ControlConfig[0] == 0`; the client always has them | spec §4.2; spacewar_ui.rs (2026-10-24, input audit) | open, minor |
| 212 | render | SpaceWar, ship poses after an order | in `spacewar_fx7` t1047 two own ships that have just turned show as bright points (seen nose-on, the trail puffs over them) where the exe shows them side-on | spec §9.10 [obs fx7] (2026-10-24) | open |
| 213 | render | SpaceWar, opening zoom | the starmap short info's planet picture (the exe's grey globe at (10..35, 545..580)) and the glow around the fleet marker are not drawn during the zoom; the short info's third line shows a problem where the exe's shows the morale (the rotation step) | spec §9.10 [obs ui5 t100] (2026-10-24) | open |
| 214 | render | main menu (and Options' load-only mode over it) wider than 4:3 | the `texcsor.bmp` background billboard ends inside the left side strip in the client; in ig2.exe it reaches the window's left edge (strip luma 5–6 vs 1.3 at 16:9; the right strip matches) | `re/screens/widescreen.md` §7.4; `_local/screendiff/wide/1920x1080/main-seq-*` (2026-10-24) | open, cause unknown |
| 215 | render | credits video | the credits text overlay's glyph scale is fitted to the capture, not derived: em 1.25·(H>>5), x 0.84; `TTF_RenderUTF8_8Bit_Wrapped` 0x72d2f0 not read; the rows equal the exe's, the text sits 3 px left of it | `video/credits.rs`; evidence `_local/screendiff/credits`, `probes/credits_text.js`; `re/screens/video.md` (2026-10-24) | open |
| 216 | input | Help (F1), F-key gate | `DoKeyboard` lets F1 through unless `Belep`/`Atlepes`/`AktKepKell` is pending, on the space battle (0xb, except `DEBUGX`), on a colony with a ground war (`AktKep` 3 and `GetWarfareStatus==1`), on Options with `OptionsCsakLoadLehet` or `OptionsHarcFolyik`, on Messages unless `AktValasztasok==0 && KiirtaMar`, or when `KepernyokLetiltva[1]`; the client refuses only the main menu, Options (always) and the space battle, and has none of the other gates | exe `DoKeyboard` ELF 0x57fd20, `GetScreenCanBeChanged` ELF 0x550990; window.rs `open_help` (input-audit 2026-10-24) | open |
| 217 | input | Help, F1 from Options | the exe opens Help from the options screen (not load-only / battle states) and returns to `OptionsElottiKep`; the client does not open it from Options | exe `Help_Belep` 0x4d7060; window.rs `open_help` (input-audit 2026-10-24) | open |
| 218 | input | Help, return screen | `HelpElottiScreen` = the screen before Help, but from Messages (10) `MessagesElottiScreen` and from Options `OptionsElottiKep`; the client returns to the screen it opened over | help.md §1; `Help_Main` 0x4d7510; window.rs `help_game_frame` (input-audit 2026-10-24) | open |
| 219 | input | Help, keys other than Esc | `DoKeyboard` stays live on `AktKep` 1: F2..F10 leave Help for that screen, O opens Options, M Messages, P / Space toggle the pause and 1/2/3 set the speed (sound 10), Enter / backquote open the chat line in MP or debug; the client swallows every key but Esc | exe `DoKeyboard` 0x4db1a0 (ELF 0x57fd20); window.rs help key arm (input-audit 2026-10-24) | open |
| 220 | input | Help, top bar | the top bar's common hot-spots (pause, speed, screen menu, message line) likely stay active over Help (not probed); the client sends every left press to the Help screen | exe `IG2_gameUpdate` 0x4e0440, `MOUSE_SelectScreens` 0x555470; window.rs (input-audit 2026-10-24; confirm with a probe) | open |
| 221 | input | Help, mouse access | the screen-select menu lists Help (entry 1) and the remaster toolbar's help button (`help_callback` 0x4d8650 / `help_press_f1` 0x4d8b40, `help_new` overlays) opens/closes it; the client has F1 only | exe `MOUSE_SelectScreens` 0x555470, `help_callback`; help.md §11 (input-audit 2026-10-24) | open |
| 222 | input | Help, hot-spot overlap | spots 3 (topic list, y 44..402) and 6 (exit, y 401..426) share rows 401-402; the port lets the lowest id win, the exe's order not read | exe `Help_MouseInit` 0x4d7640; help.rs `spot_at` (input-audit 2026-10-24; low) | open |
| 223 | multiplayer | generated-galaxy start, diplomacy reseed | the host's `SDL_GetTicks` for `NEWGAME_Diplomacy`'s libc reseed is shared with every machine; the original lets each machine use its own, so the machines' states differ there; lockstep needs one state | exe `FRISSIT_Start` 0x5bc590 / `ReceiveStartData` 0x5bc9d0 → `InitNewGame(seed,2,0,1)`; ADR 0014 §3 | deliberate, kept |
