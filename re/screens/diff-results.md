# Capture comparisons: results

`tools/trace/run_screendiff.py` (ADR 0009 §4): ig2.exe through Frida and `ig2-client --snapshot` on the same state,
800 × 600, compared by `ig2-screendiff` (mean |Δ| per pixel over R, G, B on 0..255, best offset within ±2 px, share
of pixels off by more than 32). Pass = best mean ≤ 6 at an offset within one virtual pixel. Images stay in
`_local/screendiff/`.

## How the states are matched
`screen.js` loads `quick-1.sav` with the exe's own `LoadSave`, pauses the clock, asks for the screen
(`AktKepKell`), waits 240 frames and then writes the exe's own save of that moment (`LoadSave(slot, name, 2)`,
`<name>-state.sav`); the client renders that save (`--ticks 0 --speed 1 --paused`). So both sides show the same
date, money, speed state and sim state. The message line is masked (virtual (268, 3, 328 × 15)).

Lessons for driving the exe in the background (the capture runs without window focus):
- `IsActive` (exe 0x912A38) is clear without focus and `IG2_gameUpdate` then skips `UpdateFrame`, which also
  runs the screen fades: no screen change completes. `screen.js` sets it every frame.
- `MainScr_Kilep` (exe 0x4fecc0) leaves the main menu by looping `UpdateFrame` + `FRISSITES` until its fade counter
  (f32 0xBBB360, from 100, flag 0xBBB22A) runs out, which it doesn't in the background: `screen.js` zeroes it.
- The title video (`title.bik`, `VoltGTLogo` 0xBBB22F) is skipped by setting the flag at spawn.
- With the clock paused, `LoadSave` leaves the game on screen 15; the screen request then goes from there.

## 2026-10-17, quick-1.sav (2327-10-23), client at `ff7d274` + the `--paused` option

| Screen | Region | mean@0 | best offset | mean | > 32 | luma exe / client | Verdict |
|---|---|---|---|---|---|---|---|
| starmap | frame | 27.96 | (0, 0) | 27.96 | 46.7 % | 48.1 / 45.6 | differs |
| | topbar | 7.71 | (0, 0) | 7.71 | 11.6 % | 36.8 / 43.4 | differs |
| | view | 28.50 | (0, 0) | 28.50 | 47.6 % | 48.4 / 45.6 | differs |
| colony 42 | frame | 26.82 | (0, 0) | 26.82 | 50.0 % | 43.9 / 45.9 | differs |
| | topbar | 7.73 | (0, 0) | 7.73 | 11.6 % | 37.2 / 43.8 | differs |
| | planet_panel | 18.91 | (0, 0) | 18.91 | 28.8 % | 52.1 / 53.6 | differs |
| | radar | 10.99 | (0, 0) | 10.99 | 11.5 % | 51.5 / 50.6 | differs |
| | bottom_info | 48.08 | (0, 0) | 48.08 | 95.1 % | 32.8 / 73.0 | differs |
| | buttons | 31.68 | (+1, −2) | 31.38 | 54.3 % | 44.8 / 52.2 | differs |
| | view | 29.33 | (+2, 0) | 29.28 | 55.6 % | 45.3 / 34.7 | differs |
| info | frame | 20.76 | (0, 0) | 20.76 | 31.9 % | 14.5 / 10.1 | differs |
| | topbar | 7.65 | (0, 0) | 7.65 | 11.6 % | 36.8 / 43.4 | differs |
| | race_names | 50.23 | (0, +2) | 50.18 | 83.9 % | 49.6 / 2.5 | differs |
| | chart | 60.83 | (+2, +2) | 59.87 | 95.4 % | 8.7 / 66.7 | differs |
| messages | frame | 7.43 | (0, 0) | 7.43 | 14.0 % | 6.3 / 7.7 | differs |
| | topbar | 7.65 | (0, 0) | 7.65 | 11.6 % | 36.8 / 43.4 | differs |
| | inbox | 32.53 | (−2, −2) | 32.02 | 66.5 % | 21.7 / 38.0 | differs |

No screen passes yet. What the heat maps show (in order of size):
- **Top bar** (all screens): layout and text positions match (best offset 0, 0); the client's bar is brighter
  (luma 43 vs 37) and the glyph edges differ. Next: the brush blend/gamma of `DDBrushToScreen3d` and the font's
  sampling (hud.md §1, messages.md §1).
- **Starmap**: stars, planet names and the fleet sprite line up; the background nebula is a different picture
  (the client's clouds don't follow `GenerateStarParticles`' look), and the exe draws what the client lacks: radar
  range discs with red dotted rims, the selection ring on the human's home planet, a fleet marker near (405, 310) the client doesn't draw, the
  minimap panel (`kezel.xvb`) and the planet info panel at the bottom left.
- **Colony 42**: buildings, roads and names in place; the terrain's texture/lighting differs everywhere (sampling,
  mip-mapping, lighting), the bottom-left building info (`Foepulet`, working, damage) and the panel's 3D building are
  missing in the client, the button row is a pixel or two off.
- **Info**: the race names sit right (offset ≤ 2) but the exe draws them on the race frame boxes with portraits;
  the client lacks the screen's whole 3D frame, the empire panel, the planet list and the chart's frame (it draws a
  plain panel), hence the chart and names regions.
- **Messages**: empty inbox on both; the client's plain panel vs the exe's frame.

## 2026-10-17, starmap with its HUD (`starmap-input`)
Same tool and state (`run_screendiff.py … starmap`; the capture selects planet 42), then `ig2-screendiff` on the
same pictures with the HUD regions (virtual pixels):

| Region | mean@0 | best offset | mean | > 32 | luma exe / client | Verdict |
|---|---|---|---|---|---|---|
| frame | 24.67 | (0, 0) | 24.67 | 39.6 % | 48.1 / 44.6 | differs (was 27.96) |
| topbar | 7.71 | (0, 0) | 7.71 | 11.6 % | 36.8 / 43.4 | differs (unchanged) |
| info_box 5,423,162,52 | 14.88 | (0, 0) | 14.88 | 20.7 % | 44.3 / 40.3 | differs |
| info_text 50,425,120,50 | 12.67 | (0, 0) | 12.67 | 15.8 % | 46.7 / 47.1 | differs |
| console 214,364,212,112 | 9.76 | (0, 0) | 9.76 | 6.4 % | 51.7 / 52.3 | differs |
| minimap 274,388,92,65 | 8.01 | (+1, 0) | 7.81 | 3.3 % | 20.8 / 15.4 | differs |
| button 419,430,49,49 | 7.64 | (0, 0) | 7.64 | 7.6 % | 63.2 / 65.2 | differs |

The panel, the button, the minimap's dots and outline and the info texts (name, empire, POP, MOR in Hungarian,
richness and growth icons) sit on the exe's pixels; what is left: the background clouds behind the translucent
parts, the panel's moving coloured lights (drawn as one still warm light), the box's rounded corners, the planet
picture in the info box (not found in the 2D passes), the radar rings, the top bar's brightness (see above).

## 2026-10-17, the starmap's production view and transfer panel (`starmap-panels`)
`run_screendiff.py <install> quick-1.sav production | transfer` (re/screens/starmap.md §12–§13): ig2.exe opens the
panel itself (`run_screen.py --rop 2`, and `--fleets 2 --rop 0xe --rop-fleet 1` for two new fleets of the human
beside planet 71), the client renders the exe's save of that moment with `--panel production` /
`--panel transfer:9,10`:

| Screen | Region | mean@0 | best offset | mean | > 32 | luma exe / client | Verdict |
|---|---|---|---|---|---|---|---|
| production | frame | 19.39 | (0, 0) | 19.39 | 27.8 % | 42.0 / 40.9 | differs |
| production | name_plate 250,30,140,60 | 15.62 | (0, 0) | 15.62 | 29.5 % | 18.9 / 25.4 | differs |
| production | left_panel 5,80,190,292 | 17.11 | (0, 0) | 17.11 | 25.0 % | 37.0 / 30.9 | differs |
| production | right_panel 445,80,191,292 | 17.46 | (0, 0) | 17.46 | 18.9 % | 26.9 / 38.4 | differs |
| production | exit 445,380,190,30 | 14.84 | (−1, 0) | 14.83 | 8.5 % | 48.6 / 39.9 | differs |
| transfer | frame | 14.90 | (0, 0) | 14.90 | 14.8 % | 39.9 / 43.8 | differs |
| transfer | left_panel 28,27,273,334 | 10.90 | (0, 0) | 10.90 | 9.5 % | 42.9 / 42.1 | differs |
| transfer | right_panel 338,27,273,334 | 15.49 | (0, 0) | 15.49 | 14.9 % | 33.1 / 35.6 | differs |
| transfer | scroll_bar 306,29,28,333 | 18.30 | (0, +1) | 18.29 | 23.3 % | 47.3 / 35.6 | differs |
| transfer | exit 445,380,190,30 | 12.12 | (−1, 0) | 12.11 | 5.6 % | 34.0 / 34.9 | differs |

The model (`gyartas.xvb` at frame 12 with its lamps), the faders, the name plate, the transfer rows (models, names,
counts, the full fleet's red vehicle count, the capacities) sit on the exe's pixels at offset 0. What is left: the
background clouds behind the translucent panels; the exe ran in Hungarian ("Kapacitás: NINCS", "Kilépés") while the
client reads the English texts (it doesn't pick the install's language yet); the transfer models' spin phase (the
exe's turned for ~1 s before the grab, the snapshot's not at all); the rounded corners. The panels' fader rule came
out of these pictures: `FadeScreen`'s fourth colour value is a transparency (§12.2).

## 2026-10-17, the info and messages screens with their frames (`info-messages-screens`)
`run_screendiff.py <install> quick-1.sav info | messages` (re/screens/info.md §1.1–§5, messages.md §3–§6); the info
capture now waits 1,500 frames (the frame opens over 150 UI frames and its texts fade in over 100 more: the earlier
capture was mid-opening):

| Screen | Region | mean@0 | best offset | mean | > 32 | luma exe / client | Verdict |
|---|---|---|---|---|---|---|---|
| info | frame | 12.05 | (0, 0) | 12.05 | 10.5 % | 24.9 / 17.1 | differs |
| | heads 20,40,600,90 | 6.46 | (0, 0) | 6.46 | 4.9 % | 26.4 / 25.1 | differs |
| | race_names | 13.37 | (0, 0) | 13.37 | 7.9 % | 57.9 / 50.6 | differs |
| | triangle 230,30,180,110 | 8.82 | (0, 0) | 8.82 | 4.7 % | 49.1 / 44.2 | differs |
| | empire_panel 110,140,420,90 | 18.60 | (0, 0) | 18.60 | 11.7 % | 31.0 / 22.6 | differs |
| | list 0,245,290,235 | 22.51 | (0, 0) | 22.51 | 22.1 % | 27.1 / 11.0 | differs |
| | chart | 14.35 | (0, 0) | 14.35 | 12.2 % | 18.7 / 4.7 | differs |
| | buttons 345,280,55,170 | 12.17 | (0, 0) | 12.17 | 11.1 % | 42.7 / 36.1 | differs |
| messages | frame | 3.05 | (0, 0) | 3.05 | 2.3 % | 6.3 / 3.9 | **match** |
| | panel 0,160,190,300 | 14.89 | (0, 0) | 14.89 | 10.3 % | 28.1 / 14.8 | differs |
| | inbox | 12.29 | (0, 0) | 12.29 | 6.2 % | 21.7 / 10.6 | differs |

The frames sit on the exe's pixels (offset 0, 0). Lighting the info frame from both sides (its faces point away from
the parallel light) halved every region. What is left: the exe runs in Hungarian (the panel's labels), the planet
list (planet models and texts, not drawn), the panels' blue insides and the messages panel's warmer red (the coloured
side lights the shader doesn't have), the race-name box's gradient, the top bar's brightness (see above).

## 2026-10-17, the colony screen driven (`colony-input`)
`run_screendiff.py <install> quick-1.sav colony:42` (the client now gets `--colony-live`: the exe's state save is
written on the colony screen, so the camera is the live `ColonyXStart…` chunks and the entry's selection stands):

| Screen | Region | mean@0 | best offset | mean | > 32 | luma exe / client | Verdict |
|---|---|---|---|---|---|---|---|
| colony 42 | frame | 18.09 | (0, 0) | 18.09 | 25.3 % | 43.9 / 53.3 | differs |
| | topbar | 7.71 | (0, 0) | 7.71 | 11.6 % | 37.2 / 43.8 | differs |
| | planet_panel | 16.03 | (0, 0) | 16.03 | 19.8 % | 52.1 / 59.2 | differs |
| | radar | 10.72 | (0, 0) | 10.72 | 8.5 % | 51.5 / 53.6 | differs |
| | bottom_info | 20.97 | (0, 0) | 20.97 | 36.9 % | 31.4 / 45.6 | differs |
| | buttons | 21.03 | (0, 0) | 21.03 | 36.2 % | 44.8 / 60.2 | differs |
| | view | 19.23 | (0, 0) | 19.23 | 25.5 % | 45.3 / 49.4 | differs |

Before (same exe capture, the client at `663f1f2`): frame 27.0, view 29.3, buttons 31.7, bottom_info 53.6. The view
now sits at offset 0 (the live camera); the selection marker (`epkij.bmp`) lies on the exe's over the colony hub, the
panel's turning building and the building's short info ("Working: 100%", the damage bar) are where the exe has them;
the bottom faders are `KirakFader`'s. What is left: the exe ran in Hungarian (building names, the info's texts); the
terrain and models are brighter in the client (lighting); the small turning model in the building info isn't drawn;
the panel model's turn phase. The pick list, the build cursor and the demolish window have no exe capture yet (the
capture tool can't press buttons in the exe: `ig2-client --colony-ui pick|demolish|build` renders them).

## 2026-10-17, the diplomacy screen (`diplomacy-screen`)
`run_screendiff.py <install> quick-1.sav diplomacy` (re/screens/diplomacy.md): ig2.exe opens screen 6 and waits 1000
frames (its opening animation takes 2.8 s of 10 ms UI ticks; a 300-frame grab catches the menus still fading in),
the client renders the exe's save of that moment with `--screen diplomacy` (the screen at rest). Two runs (a fresh
capture and an earlier one) gave the same numbers:

| Region | mean@0 | best offset | mean | > 32 | luma exe / client | Verdict |
|---|---|---|---|---|---|---|
| frame | 11.43 | (0, 0) | 11.43 | 15.5 % | 20.7 / 21.2 | differs |
| topbar | 7.65 | (0, 0) | 7.65 | 11.6 % | 36.8 / 43.4 | differs (unchanged) |
| info 455,30,172,100 | 11.00 | (0, 0) | 11.00 | 12.3 % | 23.6 / 24.1 | differs |
| menu 21,349,155,101 | 22.60 | (0, 0) | 22.60 | 19.7 % | 29.6 / 30.0 | differs |
| name 60,215,155,20 | 22.88 | (0, 0) | 22.88 | 38.1 % | 41.8 / 44.6 | differs |
| ring 180,95,280,290 | 24.92 | (0, 0) | 24.92 | 36.9 % | 56.1 / 54.8 | differs |
| left_heads 15,140,170,200 | 10.79 | (0, 0) | 10.79 | 13.2 % | 11.6 / 13.1 | differs |
| right_heads 455,140,180,200 | 9.08 | (0, 0) | 9.08 | 10.4 % | 4.7 / 6.3 | differs |

The info panel, the menu's frame and rows, the name under the head, the portrait and the round frame sit on the
exe's pixels at offset 0; the panels' fills match after texturing them with `dt.bmp` (the exe's fills are half as
bright as the plain fader). What is left: the language (the exe ran in Hungarian, the client reads English; this is
most of the menu region), the animated parts (the leader's video frame, the frame's turning rings, the cages' random
spin and the side lights' angle), the cages' wire colours (the original's light model on wireframes is inferred).

## 2026-10-17, the spy screen (`spy-screen`, re/screens/spy.md)

`run_screen.py --kep 7 --spies MODE --wait 1200` on quick-1.sav (the exe in Hungarian; the client with `--lang hu`,
`--ui-frames 1200`): five staged spies (at home with skill points, on a mission, travelling, captured, at home) and
one report; the exe's own save of the moment rendered by the client.

| State | frame | topbar | carousel | left_panel | right_panel | ring |
|---|---|---|---|---|---|---|
| roster (the planner) | 6.58 | 7.65 | **3.30 match** | **4.34 match** | **2.79 match** | 16.72 |
| report (a finished empire report) | 8.12 | 7.65 | **4.08 match** | **4.02 match** | **5.24 match** | 22.75 |
| verdict (an AI spy caught) | 7.00 | 7.65 | **3.35 match** | **4.34 match** | **5.92 match** | 16.61 |
| applicant (a new recruit) | 7.58 | 7.65 | **3.55 match** | **4.02 match** | **5.21 match** | 21.32 |

Best offsets within one pixel everywhere. The carousel (figures, pedestals, names, status icons, the corner
brackets and the wireframe sphere) and both panels (texts, buttons, the 0.8-transparent fills) match. The ring
differs by what runs on the clocks: the head video's frame and the ring's spin (the exe counts drawn frames, the
client UI frames); its brightness and colour match (luma 38.2/37.9). The top bar is the shared difference of every
screen (above). Found on the way: the client's texts were re-encoded as Latin-1 (Hungarian ő/ű became "?"); they are
the files' own bytes now (`UiTexts::get`, `ui::hu_bytes`).


## 2026-10-17, the research screen (`research-screen`)
`run_screendiff.py <install> quick-1.sav research` (re/screens/research.md): ig2.exe opens the screen on the save's
selection (turrets, the machine-gun turret) and is grabbed after 300 frames; the client renders the exe's save of that
moment with `--screen research --ui-frames 240`, in Hungarian like the exe (`IG2_LANG=hu`, new):

| Region | mean@0 | best offset | mean | > 32 | luma exe / client | Verdict |
|---|---|---|---|---|---|---|
| frame | 9.49 | (0, 0) | 9.49 | 9.2 % | 15.6 / 16.6 | differs |
| topbar | 7.65 | (0, 0) | 7.65 | 11.6 % | 36.8 / 43.4 | differs |
| info_box 175,25,300,55 | 13.17 | (0, 0) | 13.17 | 17.1 % | 45.6 / 39.7 | differs |
| start_double 100,20,440,62 | 9.92 | (0, 0) | 9.92 | 11.6 % | 41.7 / 37.3 | differs |
| left_list 3,81,135,106 | 17.45 | (0, 0) | 17.45 | 16.4 % | 25.9 / 37.7 | differs |
| right_list 500,81,135,106 | 13.39 | (0, 0) | 13.39 | 7.8 % | 16.9 / 29.7 | differs |
| labs 0,235,80,60 | 4.33 | (0, 0) | 4.33 | 2.5 % | 19.9 / 19.2 | match |
| labs_right 560,235,80,60 | 4.27 | (0, 0) | 4.27 | 3.0 % | 20.3 / 19.8 | match |
| wheels 60,330,520,150 | 10.11 | (0, 0) | 10.11 | 12.1 % | 23.8 / 21.4 | differs |
| ball 150,70,340,300 | 14.65 | (0, 0) | 14.65 | 12.5 % | 9.8 / 11.0 | differs |

The console (`resbck.xvb` at its open frame), the three wheels, the start/stop and double-money buttons with their
textures, the lab gadgets and numbers, the info box's texts and glass, the lists with their colours and the chosen
row, the ball's wireframe and the category's models (owned solid, the rest wireframe, the chosen one large in the
middle) sit on the exe's pixels at offset 0; the two lab regions pass. What is left: the side panels' faders come
out ~1.6× brighter than the exe's (fill (0, 28, 48) vs (0, 17, 28), edge (0, 65, 94) vs (0, 42, 61)) although they
follow `KirakFader`'s values [inf: something darkens them in the exe; the starmap's `fader` rule doesn't explain
it]; the chosen row's frame is a plain box (`DesignKeret1` animates); the ball's and models' turn phase (the exe's
turned for 300 drawn frames, the snapshot for 240 UI frames); the models' lighting (one light for the original's
four); the wheels' textures' sampling; the top bar (see above).
