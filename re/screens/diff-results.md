# Capture comparisons: results

`tools/trace/run_screendiff.py` (ADR 0009 §4): ig2.exe through Frida and `ig2-client --snapshot` on the same state,
800 × 600 unless `--size` says otherwise, compared by `ig2-screendiff` per region (virtual 640 × 480 pixels): mean
|Δ| per pixel over R, G, B on 0..255 at the best offset within ±2 px, plus the share of pixels off by more than 32.
**Pass = best mean ≤ 6 at an offset within one virtual pixel.** Images stay in `_local/screendiff/`. Old run logs:
git history.

## How the states are matched
- `screen.js` loads the save (usually `quick-1.sav`) with the exe's `LoadSave`, pauses the clock, asks for the
  screen (`AktKepKell`), waits (240 frames; longer openings more: info 1,500, diplomacy 1,000) and writes the exe's
  save of that moment (`<name>-state.sav`), which the client renders (`--ticks 0 --speed 1 --paused`). The message
  line is masked (268, 3, 328 × 15). `<state>.ui.json` carries the unsaved UI clocks (clouds, `futofeny`, `acttime`,
  the faders' phase `--faders`, spins).
- Background driving: `screen.js` sets `IsActive` (0x912A38) every frame (else `UpdateFrame` and the fades skip),
  zeroes `MainScr_Kilep`'s fade counter (f32 0xBBB360), skips the title video via `VoltGTLogo` (0xBBB22F). With the
  clock paused `LoadSave` leaves the game on screen 15. Both sides use the exe's language (here Hungarian).
- Grabs: `--gl-grab` reads the still in the game at the next `SwapBuffers`, like `--script` grabs. PrintWindow grabs
  include the OS pointer and tooltip and come back pure white (2,785-byte PNG) with the display off. Script grabs
  before UI tick 100 fire late.
- Sequences: `--script tools/trace/scripts/<name>.json` (UI ticks; the exe draws one frame per UI tick). Stored exe
  runs: `--exe-png`/`--exe-state` (stills), `--exe-seq` (sequences). Held frames: `--anim-frame F [--anim-closing]`,
  `--anim-mouse X,Y` (spin globals pinned on both sides).
- Other entries: `--boot` (main menu), `--via-starmap F` (overlays with their `SaveScreen` backdrop; `--size` always
  passed, the install's `ig2.settings` may say 1920 × 1080), `--size WxH [--letterbox]` (widescreen.md §7),
  `run_spwc.py --grab-ticks` + `--sim-phase` (SpaceWar), `run_video.py`, `run_screen.py --tutorial`.
- Noise, not port bugs: the faders' random shine/flash and sparks (libc `rand()`, row 84 keep), undumped video and
  spin phases, the exe's pointer.
- Sweeps: re-run every still and script on the stored exe captures, new exe runs only for missing/stale ones, bisect
  regressions. The nine starmap screens use `_local/screendiff/bg-gl/<screen>-{exe.png,state.sav}`.

## Current state per screen
Open = rows still open in `re/deviations.md` (keep rows marked).

| Screen | Capture(s) | Latest score / verdict | Open |
|---|---|---|---|
| Main menu | `main --script mainscr`, `mainscr-long`, `-desc`, `-race`, `-scenario`, `-multi` (`--boot`) | menu t 5–700 2.6–5.3, fly-out 7.8; description lines match (static 2.2–3.6, fades 9–13); race rings 5.3–6.7, t 1420 ~16 (random galaxy), exit zoom 15.6/26.5; scenario static 4.3–5.2, slides 8–11; multi 6–11 (name box) | 20, 150, 154, 157 |
| Starmap + HUD | `starmap` (`bg-gl`); `starmap:66 --client-new-campaign` | frame 2.59, view 2.66; new campaign 3.10: match | none |
| Starmap panels | `production`, `transfer` (`bg-gl`) | every region match | none |
| Starmap menus, long info | `rop`, `planets`, `planetinfo`, `tax`/`autobuild`/`priority` (`bg-gl`) | every region match (planetinfo planet 2.63) | none |
| Fleet / vehicle panels | `fleetinfo`, `tanksinfo`, `fleetbox` | frames 3.1–5.2, models 3.0–3.4, lists 3.0–4.6: match | none |
| Overlay backgrounds | `--via-starmap 120` on info, messages, research, options, design, trade, diplomacy | 0.6–3.4 (options backdrop 3.2–4.5); research 5.8, diplomacy 8.6 = their own kinds | 263 keep |
| Top bar + drop-downs | every screen; `topbar-menus.json` | bar 0.05–0.2 (SpaceWar 2.56); menus 2.2–4.7: match | none |
| Colony | `colony:42`, `--colony-info [tax --info-mouse 500,322]`, `--self-destruct 380`, `colony-weather.json` | frame 8.7 differs (planet_panel 6.6, bottom_info 8.0, view ~8.9, radar 5.6); info mode frame 7.4; self-destruct 9.9 | 397, 398; 369, 384 keep |
| Ground war | `grwar:17 [--land]`, `colony-sweep-war.json` | frame 12.8, view 6.4, texts 5.2, radar 9.3, bottom_info 14.3, buttons ~52 (the exe's terrain shadow band): differs | |
| Research | `research`, `--anim-frame 12…60`, `--anim-mouse`, `--script research`, `research-lines`, `research-fx9` | sequences frame 1.8–2.6, ball 2.7–3.5, lines bit-exact; held ≤ ~5.5; still 5.57, wheels 6.1 and ball ~11 (turn phase) differ | none |
| Design | `design`, `design --script`, `design-edit`, `design-pods` | match but the exe's fader flashes; hull 0.9–5.7 | 114 keep |
| Diplomacy | `diplomacy`, `--anim-frame 20/50/70`, `--script diplomacy`, `diplomacy-fx9` | still frame ~8, menu 3.5, heads ~6/5, name ~17, ring ~20 (leader video, spin, cage wires); closing t 600 ~11: differs | none |
| Spy | `spy:roster/report/verdict/applicant`, held opening/closing, `--spy-mouse` | carousel, panels, tooltip match; frames 5.0–6.3; ring 14–20 (video + spin) | none |
| Trade | `trade`, `trade-buy`, `trade-pods` | 0.0–4.4, hull 0.7–1.0; purchase, sounds identical: match | none |
| Info | `info`, `info:IDS [--fleets N] [--starlog N]`, `info:9 --select …`, `--info-mouse` | every region ≤ 6; race_names 6.5–7.5 vs a stale exe shot (refresh needed; a fresh `--gl-grab` came back near-black) | none |
| Messages | `messages`, `messages-open`/`-esc` (`replay-quick1-messages-x16/230.sav`), `messages-close-press` | still ≤ 0.3; sequences ≤ 1.9, `mesframe` equal: match | none |
| Options | `options --script options`, `options-load` | match but `buttons` 6.5–11 (sparks/shine), `slots` t 500/560 18–21 (client shine, cause unknown) | 84 keep |
| Help | `help --script help` | text 1.8–3.4, topics 3.0–4.9: match; opening not compared | none |
| SpaceWar | `spacewar_fx8 --exe-seq`, `spacewar_fx7`, `spacewar_info` (1920 × 1080), stills | **accepted**: frame 3.4–4.2, view 3.7–4.8, regions ≤ 5.3 but the exe's box shine | 58, 102, 104, 213, 380; 273 keep |
| Video | `title.bik`, message clips (`--video-at 201,204`) | 0.02–1.9 at 800 × 600 and 1280 × 720: match | none |
| Tutorial overlay | `run_screen.py --kep 2 --script tutorial --tutorial 1`; `--tutorial-window` | window 0.77–0.88: match; frame 6.2–6.8 (drift) | 141, 174, 198 |
| Widescreen | `--size 1920x1080 / 2520x1080 / 3840x2130 / 1280x1024 [--letterbox]` | in-frame = 800 × 600 baseline within ~1 level; strips match; SpaceWar, Help, credits, demo, tutorial not compared | 4 keep |

## Commands per screen
`rsd` = `_local/venv/Scripts/python.exe tools/trace/run_screendiff.py "$IG2_INSTALL_DIR"`; `S` = `_local/screendiff`;
scripts live in `tools/trace/scripts/`. 800 × 600 and `quick-1.sav` unless stated. "Stored" = rerun the client only
with `--exe-png S/<dir>/<name>-exe.png --exe-state S/<dir>/<name>-state.sav` (sequences: `--exe-seq <dir>`).

- **Main menu**: `rsd main --script tools/trace/scripts/mainscr.json` (exe booted with `--boot`, no save); likewise
  `mainscr-long.json`, `mainscr-race.json`, `mainscr-scenario.json`, `mainscr-multi.json`, `mainscr-desc.json --exe-seq …`.
- **Starmap family** (`starmap`, `planets`, `rop`, `planetinfo`, `tax`, `autobuild`, `priority`, `transfer`,
  `production`): `rsd quick-1.sav <screen> --gl-grab --out S/bg-gl`; stored baseline `S/bg-gl/<screen>-{exe.png,state.sav}`
  (not the older `S/bg/` or top-level `S/*-exe.png`). New campaign: `rsd newgame-c1-d2-s4242.sav starmap:66
  --client-new-campaign 1,2,4242,96277` (exe save from `run_newgame.py`).
- **Fleet / vehicle panels**: `rsd quick-1.sav fleetinfo|tanksinfo` (planet 71, wait 420, `--faders`; latest stored:
  fleetinfo `S/fx13g/fresh`, tanksinfo `S/tanks-0.png`; transfer with `--spin-frames` against `S/transfer2-0.png`);
  `rsd quick-1.sav fleetbox --fleets 2 --rop-fleet 1`.
- **Overlay backgrounds**: `rsd quick-1.sav info|messages|research|design|trade|diplomacy --via-starmap 120` (classic
  backdrop), `rsd options --script tools/trace/scripts/options.json --via-starmap 120`; out `S/overlay-bg`.
- **Top bar drop-downs**: `rsd quick-1.sav topbar-menus --script tools/trace/scripts/topbar-menus.json
  --client-arg=--top-menu --client-arg=screens:630|research|research+2 --client-arg=--top-mouse --client-arg=X,Y` (1920 × 1080).
- **Colony**: `rsd quick-1.sav colony:42` (client gets `--colony-live`); `colony:42 --colony-info`; `colony:42
  --colony-info tax --info-mouse 500,322`; `colony:42 --self-destruct 380`; rain: `run_screen.py quick-1.sav --script
  scripts/colony-weather.json` on colony 6 (stills come back white). Sounds only: `scripts/colony-info.json`.
- **Ground war**: `rsd quick-1.sav grwar:17` (stored: `--exe-png S/grwar17-exe.png --exe-state
  S/fx17b3/grwar17-state.sav`); `grwar:17 --land`; `grwar:17 --script tools/trace/scripts/colony-sweep-war.json`.
- **Research**: still `rsd quick-1.sav research` (stored `S/topbar/research-exe.png`, exe `--wait 1000`); held frames
  `rsd quick-1.sav research --anim-frame 12|30|45|55|60`, `--anim-frame 40 --anim-closing`, `--anim-frame 60
  --anim-mouse 507,50` / `40,210`; sequences `research --script tools/trace/scripts/research.json`, `research-lines.json`,
  `research-fx9.json`.
- **Design**: `rsd quick-1.sav design`; `design --script` (viewing, 14 grabs; the log doesn't name the file, `design-sounds.json`
  holds its sounds), `design --script tools/trace/scripts/design-edit.json`, `design-pods.json`.
- **Diplomacy**: `rsd quick-1.sav diplomacy` (fresh at `S/render-fresh`); `diplomacy --anim-frame 20|50|70`;
  `diplomacy --script tools/trace/scripts/diplomacy.json`, `diplomacy-fx9.json`.
- **Spy**: `rsd quick-1.sav spy:roster|report|verdict|applicant` (exe `run_screen.py --kep 7 --spies MODE --wait 1200`;
  roster fresh at `S/render-fresh`); `spy:roster --anim-frame 12|30|45`, `--anim-frame 20 --anim-closing`; tooltip
  `--spy-mouse 480,275`.
- **Trade**: `rsd quick-1.sav trade` and `trade-buy` (scripts `trade.json`, `trade-buy.json`); `trade --script tools/trace/scripts/trade-pods.json`.
- **Info**: `rsd quick-1.sav info` (stored `S/topbar/info-exe.png` + `S/lang/info-state.sav`, stale: refresh); list
  `S/infolist`; `info:9`, `info:31×8+12 --fleets 2`, `info:31+31+31 --fleets 2`, `info:15`, `info:15+35+35 --starlog 20`
  (`S/infodetail`); `info:9 --select 26,150,340,2|27,160,300,4|28,150,370,3`; `info --info-mouse 370,290`.
- **Messages**: `rsd quick-1.sav messages`; `rsd replay-quick1-messages-x16/230.sav messages --script
  tools/trace/scripts/messages-open.json` / `messages-esc.json`; `messages --script messages-close-press.json --via-starmap 60` (exe only).
- **Options**: `rsd options --script tools/trace/scripts/options.json` (over a save); `rsd options-load --script
  tools/trace/scripts/options-loadonly.json` (`--boot`).
- **Help**: `rsd quick-1.sav help --script tools/trace/scripts/help.json`.
- **SpaceWar**: `rsd spacewar --script spacewar_fx8.json --exe-seq _local/traces-fx8b/fx8` (16 grabs t 800–2300);
  `spacewar --script spacewar_fx7.json --exe-seq S/spacewar-fx7/…-exe`; `spacewar --script spacewar_info.json`
  (1920 × 1080, `S/spacewar-info2`); stills `rsd S.sav spacewar:<grab>.json` (grabs from `run_spwc.py --grab-ticks`).
- **Video**: `run_video.py` against client `--snapshot --screen video:title --video-ms T` (T 2, 70, 1002, 3001, 6002,
  20002; 800 × 600 and 1280 × 720); clips `run_video.py --open NAME` against `--screen video:NAME --video-at 201,204
  [--video-loop]` (`misc/race_01_loop`, `misc/end_1`, `misc/race_05_wait` at 1280 × 720).
- **Tutorial overlay**: `run_screen.py quick-1.sav --kep 2 --script scripts/tutorial.json --probe probes/tutorial.js
  --tutorial 1`; client `--snapshot --tutorial-window <record>` (test `tutorial_ui::tests::chapter_1_window_matches_the_exe`).
- **Widescreen**: `rsd quick-1.sav <screen> --size 1920x1080|2520x1080|3840x2130|1280x1024 [--letterbox]`, the 20
  still screens plus the main menu and Options scripts at 16:9; out `S/wide/<size>/`.
