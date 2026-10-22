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

## 2026-10-17, the spy screen's animation and tooltips (`spy-anim`, re/screens/spy.md §1.4, §6.1)

The same four states re-run after the opening/closing animation, the side lights as parallel `0xBBGGRR` lights, the
sphere's `MoveObj` at rest and its face culling:

| State | frame | topbar | carousel | left_panel | right_panel | ring |
|---|---|---|---|---|---|---|
| roster | 6.48 | 7.65 | **3.22 match** | **4.11 match** | **2.54 match** | 16.67 |
| report | 7.96 | 7.65 | **3.91 match** | **3.78 match** | **4.99 match** | 22.86 |
| verdict | 6.71 | 7.65 | **3.11 match** | **4.11 match** | **5.74 match** | 16.52 |
| applicant | 7.39 | 7.65 | **3.37 match** | **3.78 match** | **5.00 match** | 21.30 |

The opening and closing held at a frame in both (`--spy-frame F [--spy-closing]`, roster):

| Frame | frame | carousel | left_panel | right_panel | ring |
|---|---|---|---|---|---|
| opening 12 (telescopes out, panels swinging) | 2.98 | **0.21** | 7.29 | **5.91** | **3.35** |
| opening 30 (the ring in, the sphere opaque) | 4.26 | **1.56** | **2.70** | **2.00** | 12.60 |
| opening 45 (the carousel dropping in) | 5.01 | **3.55** | **1.82** | **1.88** | 12.27 |
| closing 20 (the sphere flying out) | 4.39 | **0.75** | 7.34 | 6.82 | 8.67 |

The geometry matches at every frame (the panels' swing, the telescopes, the sphere's flight and turn: the sphere
region at closing 20 is 4.96, a match, once only the triangles facing the camera draw). What differs: the side
lights' tint on the frame while it opens (the exe's frame is greyer: opening 12's left panel, closing 20's panels),
and at closing 20 the exe shows no disc (a held closing never ran a flip; in a real closing the disc fades out).
The ring at 30/45 is the head video and the spin, as at rest. The tooltip (`--spy-mouse 480,275` on hot-spot 0x10):
text "Küldetés választása", place and box match; the exe's text is a little brighter (the tooltip region 9.19, with
the exe's pointer in it).


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

## 2026-10-17, the starmap's radial menu, the planet's long info and the select menus (`starmap-menus`)
`run_screendiff.py <install> quick-1.sav rop | planetinfo | tax | autobuild | priority | planets` (re/screens/starmap.md
§14–§17): ig2.exe holds the state itself (`run_screen.py --rop-menu 320,240`, `--rop 0xf` for the long info, `--menu
KIND@X,Y` with `--mouse X,Y` for the hover; the staging runs in `StarmapGfx2D_1`'s hook because `Starmap_Main` doesn't
run while the clock is paused), the client renders the exe's save with `--rop-menu`, `--planet-info`, `--menu`,
`--mouse` and `IG2_LANG=hu` like the exe's run (the pointer and the hot-spot tooltip beside it are masked):

| Screen | Region | mean@0 | best offset | mean | > 32 | luma exe / client | Verdict |
|---|---|---|---|---|---|---|---|
| rop | icons 240,175,135,135 | 14.61 | (0, 0) | 14.61 | 21.1 % | 66.6 / 65.3 | differs |
| planetinfo | panel 319,33,302,326 | 12.30 | (0, 0) | 12.30 | 9.6 % | 29.6 / 38.0 | differs |
| planetinfo | planet 0,40,300,330 | 44.58 | (−2, −2) | 44.26 | 58.7 % | 79.3 / 100.0 | differs |
| planetinfo | exit 445,380,190,30 | 8.78 | (0, 0) | 8.78 | 5.9 % | 35.5 / 33.2 | differs |
| tax | panel | 12.29 | (0, 0) | 12.29 | 9.5 % | 28.7 / 37.3 | differs |
| tax | menu 416,275,130,108 | 8.25 | (0, 0) | 8.25 | 9.2 % | 19.6 / 16.0 | differs |
| autobuild | menu 416,297,130,121 | 8.99 | (0, 0) | 8.99 | 9.2 % | 20.3 / 16.7 | differs |
| priority | menu 416,319,130,43 | 6.34 | (0, 0) | 6.34 | 6.5 % | 13.6 / 11.2 | differs |
| planets | menu 294,88,200,60 | 13.29 | (0, 0) | 13.29 | 16.0 % | 13.2 / 17.4 | differs |

The four icons, the long info's panel, its button frames, every value text (surface with richness, race with
growth, population, morale, income, workers/energy and their needs in red, tanks, the problem line, the tax /
auto-build / priority values) and the menus' boxes, items and highlighted row sit on the exe's pixels at offset 0
(the heat maps light up only the background). What is left: the planet sphere is drawn ~25 % larger than the exe's and without its cloud layers (§3.3's size is
inferred); the background behind the translucent boxes; the radial icons' gaps show the background clouds.

## 2026-10-17, both sides in the same language (`client-lang`, re/formats/locale.md)
The client now picks the language ig2.exe picks on the same machine (`ig2_formats::locale::exe_language`:
`ig2.settings`' `language=`, a `locale.txt`, the system locale; `--lang` / `IG2_LANG` override it), so the screens
no longer need `IG2_LANG=hu`; the inbox's sender and subjects and the Hungarian `txt17` names went through a Latin-1
conversion (ő/ű → "?") and now keep their code page. Fresh captures of quick-1.sav (`--out _local/screendiff/lang`),
both sides in Hungarian (checked on the images: "Kapacitás: NINCS", "Kilépés", "Tárgyal", "Köztársaság"):

| Screen | Region | before | now | Verdict |
|---|---|---|---|---|
| starmap | frame / topbar / view | — | 24.69 / 7.71 / 25.14 | differs |
| colony 42 | frame / planet_panel / bottom_info / buttons / view | — | 17.71 / 15.55 / 18.86 / 21.03 / 18.74 | differs |
| colony 42 | radar | — | 10.44 | differs |
| info | frame / race_names / empire_panel / list / chart | — | 11.42 / 12.59 / 13.82 / 22.52 / 14.71 | differs |
| info | heads / triangle / buttons | — | 6.13 / 7.71 / 12.15 | differs |
| messages | frame | — | **3.05** | match |
| messages | panel / inbox | — | 14.89 / 12.29 | differs |
| production | name_plate / left_panel / right_panel / exit | — | 15.72 / 16.71 / 17.19 / 11.43 | differs |
| transfer | left_panel / right_panel / scroll_bar / exit | — | 10.55 / 14.52 / 30.02 / 8.17 | differs |
| diplomacy | frame / info / **menu** / name | 11.43 / 11.00 / **22.60** / 22.88 | 10.49 / 8.53 / **9.59** / 23.13 | differs |
| diplomacy | ring / left_heads / right_heads | 24.92 / 10.79 / 9.08 | 24.45 / 10.90 / 9.04 | differs |

All best offsets (0, 0); the top bar (7.65–7.80) is the shared brush/gamma difference. "Before" is the English client
of the diplomacy run above (the earlier screens' runs used other client states, so only diplomacy compares one to
one). The diplomacy menu's error fell by 58 %: what is left there and elsewhere is no longer the language (the
glyph edges, faders, backgrounds and animated parts listed per screen above).

## 2026-10-17, the 2D pass in gamma space (`topbar-gamma`, re/screens/hud.md §1.1)
ig2.exe's brushes and glyphs are plain-RGBA GL textures on a non-sRGB frame, sampled bilinearly on their one level;
the client tinted and blended them in linear light (sRGB textures and frame) with mip chains. `Blend::GammaAlpha` /
`GammaAdditive` now draw them through the frame's raw view. Same exe captures and state saves as the language run
above (`--exe-png`, `--exe-state`; research and spy:roster captured afresh, research with `--wait 1000`), client
rebuilt (`--out _local/screendiff/topbar`):

| Screen | Region | before | now | Verdict |
|---|---|---|---|---|
| every screen | **topbar** | 7.65–7.80 | **0.05–0.21** (research 0.06) | match |
| starmap | frame / view | 24.69 / 25.14 | 24.36 / 25.01 | differs |
| colony 42 | frame / planet_panel / radar / bottom_info / buttons / view | 17.71 / 15.55 / 10.44 / 18.86 / 21.03 / 18.74 | 16.96 / 13.18 / 8.31 / 17.25 / 19.79 / 18.65 | differs |
| info | frame / **heads** / race_names / triangle / empire_panel | 11.42 / 6.13 / 12.59 / 7.71 / 13.82 | 10.97 / **5.74** / 11.86 / 6.73 / 12.39 | heads match |
| info | list / chart / buttons | 22.52 / 14.71 / 12.15 | unchanged | differs |
| messages | frame / panel / inbox | 3.05 / 14.89 / 12.29 | **2.85** / 14.89 / 12.29 | frame match |
| production | frame / name_plate / left_panel / right_panel / exit | 19.34 / 15.72 / 16.71 / 17.19 / 11.43 | 18.89 / 13.64 / 16.57 / 17.01 / 10.63 | differs |
| transfer | frame / left_panel / right_panel / scroll_bar / exit | 14.55 / 10.55 / 14.52 / 30.02 / 8.17 | 14.09 / 10.26 / 14.22 / 30.01 / 7.00 | differs |
| diplomacy | frame / **info** / **menu** / name | 10.49 / 8.53 / 9.59 / 23.13 | 9.81 / **5.27** / **4.52** / 19.86 | info, menu match |
| diplomacy | ring / left_heads / right_heads | 24.45 / 10.90 / 9.04 | 24.45 / 10.62 / 9.04 | differs |
| research | frame / info_box / start_double / left_list / right_list | 9.49 / 13.17 / 9.92 / 17.45 / 13.39 | 8.76 / 9.58 / 7.97 / 15.54 / 12.79 | differs |
| research | labs / labs_right / wheels / ball | 4.33 / 4.27 / 10.11 / 14.65 | **3.91 / 3.96** / 9.94 / 14.06 | labs match |
| spy:roster | frame / carousel / left_panel / right_panel / ring | 6.58 / 3.30 / 4.34 / 2.79 / 16.72 | 6.08 / 3.42 / **1.87** / **2.13** / 16.62 | panels match |
| rop | frame / icons | 24.23 / 14.61 | 23.80 / 12.87 | differs |
| planetinfo | frame / panel / planet / exit | 24.61 / 12.30 / 44.26 / 8.78 | 23.70 / 10.42 / 44.26 / 7.67 | differs |
| tax | frame / panel / **menu** | 24.71 / 12.29 / 8.25 | 23.74 / 10.30 / **5.46** | menu match |
| autobuild | frame / panel / **menu** | 24.71 / 12.40 / 8.99 | 23.73 / 10.55 / **5.98** | menu match |
| priority | frame / panel / menu | 24.71 / 12.32 / 6.34 | 23.81 / 10.43 / **5.01** | menu match |
| planets | frame / menu | 19.09 / 13.29 | 18.57 / 11.81 | differs |

No region got worse except the spy carousel (3.30 → 3.42, still a match at a one-pixel offset; its research and
spy "before" are the earlier runs with their own captures). The top bar's remainder is a few glyph-edge pixels.

## 2026-10-17, the faders and the other 2D draws in gamma space (`gamma-2d`, re/screens/hud.md §1.2)
The faders now follow the exe's `FadeScreen` path (`PutDarkeningFace` → `polygonsfader`): rounded fill and frame,
opacity `(255 − t)/255`, tinted by `fadertexture` (`dt.bmp`: across `KirakFader`'s fill at 0.005 per pixel since
`GLOBALKELLDESIGN` is set, its corner texel for the frames and the direct `FadeKerettelKerekit`s), colours × s = 0.9,
on `Blend::GammaAlpha` (the `srgb_blend` compensation is gone). Also on the gamma blends: the `FillToScreen`-style
palette fills (colony radar dots and view, hit-point stripes, the research cursor and row frame, the info name frame,
the messages cursor), the minimap's dots and view, research's glass and wireframes, the diplomacy background picture
and the starmap's fleet sprites. Same exe captures and state saves as the top-bar run (`--exe-png`, `--exe-state`);
"before" is the 2026-10-17 `main` (4a8534c + 51f1237) rebuilt on the same captures (`--out _local/screendiff/base`),
"now" `--out _local/screendiff/gamma2d`:

| Screen | Region | before | now | Verdict |
|---|---|---|---|---|
| colony 42 | frame / **bottom_info** / radar | 16.96 / 17.25 / 8.31 | 16.77 / **12.42** / 8.28 | differs |
| info | frame / empire_panel | 10.97 / 12.39 | 10.92 / 11.93 | differs |
| production | left_panel / exit | 16.57 / 10.63 | 15.90 / 9.44 | differs |
| production | right_panel | 17.01 | 17.76 | differs (background) |
| transfer | frame / right_panel / scroll_bar / **exit** | 14.09 / 14.22 / 30.01 / 7.00 | 13.89 / 13.87 / 29.28 / **5.08** | exit match |
| transfer | left_panel | 10.26 | 10.96 | differs (background) |
| diplomacy | frame / **info** / **menu** | 9.81 / 5.27 / 4.52 | 9.67 / **3.95** / **3.51** | info, menu match |
| research | frame / left_list / **right_list** / wheels / ball | 8.76 / 15.54 / 12.79 / 9.94 / 14.06 | 7.49 / 8.05 / **3.36** / 9.81 / 12.71 | right_list match |
| research | info_box | 9.58 | 9.64 | differs |
| spy:roster | right_panel | 1.87 | 1.69 | match |
| planetinfo | frame / panel | 23.70 / 10.42 | 23.16 / 9.48 | differs |
| planetinfo | exit | 7.67 | 9.89 | differs (background) |
| tax / autobuild / priority | panel | 10.30 / 10.55 / 10.43 | 9.47 / 9.76 / 9.64 | differs |
| tax / autobuild / priority | **menu** | 5.46 / 5.98 / 5.01 | **2.89 / 3.56 / 2.95** | match |
| planets | menu | 11.81 | 12.21 | differs (background) |

Unchanged (±0.03): starmap, messages, info's other regions, rop, the top bar everywhere. The regions marked
"background" got worse because the boxes are now right: across their edges the exe's and the client's pixels fit
inside = A + B·outside with the same B (0.37–0.40) and A (e.g. (0, 14.7, 24.1) vs the exe's (0, 14.3, 23.9) on the long
info), but the starmap behind them is brighter in the client (outside the long info: luma 50 vs 15; the clouds are laid
out differently and the exe's radar circles are missing), which the old linear blend partly hid. research's
info_box (+0.06) is its wireframes' colours.

Tried and left as they were: the starmap's clouds and star glows on `GammaAdditive` (view 25.0 → 30.7: the darker
clouds no longer stand in for the exe's different cloud layout, which is its own task); the 3D frames and models
composited from render targets (`screen_frames`, the console, the production/research models, the colony radar
picture: neutral, messages' frame worse); the info screen's lines and chart (`line2d` is plain GL too, but the
port's line colours are [inf] and dimmer than the exe's: heads/triangle/chart +0.15 under the gamma blend; the info
screen's own faders, `PutFaders` ELF 0x55b2c0, aren't drawn yet).

## 2026-10-17, the research screen's animation, presses, row frame and tooltips (`research-anim`)
The opening/closing (`resframe`), the presses, `DesignKeret1`, the lasers and the tooltips (research.md §5) against
the at-rest capture of the top-bar run (`--exe-png _local/screendiff/topbar/research-exe.png --exe-state …`; the
client now draws the screen animated: the info box and the wire sphere where `MoveObj` puts them, the row frame).
"before" = `main` after the gamma-space faders (fd89549, the table above):

| Region | before | now | Verdict |
|---|---|---|---|
| frame | 7.49 | 7.05 | differs |
| topbar | 0.06 | 0.05 | match |
| info_box / start_double | 9.64 / ≈7.97 | 9.56 / 7.82 | differs |
| **left_list** / right_list | 8.05 / 3.36 | **4.09** / 3.19 | match |
| labs / labs_right | ≈3.91 / ≈3.96 | 3.61 / 3.66 | match |
| wheels / ball | 9.81 / 12.71 | 9.60 / 12.39 | differs |

The left list matches with the real `DesignKeret1` frame (its border's running light depends on `GfxNovelo`'s phase
since the game started: not reproducible, judged loosely). Moving the wire sphere to `resInitPos1` at rest helps
frame, labs, wheels and ball (before the merge: frame 8.65 → 8.40, labs 3.91 → 3.60); culling its back faces as the
spy screen's sphere changes nothing measurable (left off). The opening's frames and the tooltip could not be
captured in this session: every exe grab (also the starmap's) came back pure white and a desktop copy black (the
display was off); `run_screendiff.py research --anim-frame 12|30|45` and `--anim-mouse 507,50` (the urgent button,
text (4, 1, 8)) are ready for the next run.

## 2026-10-17, the starmap's background (`starmap-background`, re/screens/starmap.md §6)
The background follows the exe: the generator with the exe's f32/f64 rounding and `AddColor`, 30 of the 300 fog
clumps lit (the `+0x48` backup quirk), `OptionsFogResolution·50` clouds drawn and drifting on `GomolygasCounter`, the
far stars twinkling and the system stars white and pulsing on `futofeny`, the radar ranges (dots and the discs'
union), the sky sphere `gomb.xvb`, all on the gamma blends. The capture tool now reports the two unsaved UI clocks
(`<state>.ui.json` → `--clouds`, `--futofeny`), so every screen was captured again (`--out
_local/screendiff/bg`); "before" is the 2026-10-17 `main` (fd89549) client on the same captures and state saves
(`--out _local/screendiff/bg-before`). The transfer panel's snapshot now uses the exe's zoom 590 (§13.1).

| Screen | Region | before | now | Verdict |
|---|---|---|---|---|
| starmap | frame / **view** | 24.36 / 25.01 | 5.36 / **5.50** | match |
| production | frame / name_plate / left_panel | 18.91 / 13.64 / 15.91 | 5.46 / 6.52 / 6.38 | frame match |
| production | **right_panel** / **exit** | 17.76 / 9.44 | **5.18** / **3.68** | match |
| transfer | **frame** / **left_panel** / **right_panel** / **exit** | 13.88 / 10.97 / 13.88 / 5.08 | **5.44 / 5.17 / 4.69 / 3.21** | match |
| transfer | scroll_bar | 28.81 | 20.90 | differs (the bar itself) |
| rop | frame / **icons** | 24.13 / 13.28 | 5.19 / **3.78** | match |
| planetinfo | frame / **panel** / **exit** | 23.72 / 9.75 / 11.55 | 17.05 / **4.10** / **3.18** | panel, exit match |
| planetinfo | planet | 44.57 | 41.52 | differs (the planet model) |
| tax / autobuild / priority | **panel** | 9.60 / 9.88 / 9.78 | **3.89 / 4.18 / 4.02** | match |
| tax / autobuild / priority | menu | 2.91 / 3.72 / 2.74 | 1.90 / 2.18 / 2.46 | match |
| planets | frame / **menu** | 18.79 / 11.23 | 5.41 / **3.27** | match |

The top bar is unchanged (0.09–0.12). The long info's frames stay at ~17: its planet model (the zoomed-in planet
with its clouds, `HideClouds`) is the rest. Tried and left: the background particles on the linear additive blend
with the sky (view 9.5), a gain on the clouds (1.25: 5.84, 1.5: 8.5), the sky lit 0.25/0.25 with the light from
(10, 10, −10) (5.0, but the colony's light direction and the scene's own levels fit as well at 5.5).

## 2026-10-17, the research screen's opening, closing and tooltips captured (`research-anim-captures`)
The exe captures the previous run couldn't take (research.md §5, §7): `run_screendiff.py <install> quick-1.sav research
--anim-frame F` for F = 12, 30, 45, 55, `--anim-frame 40 --anim-closing`, and the open screen `--anim-frame 60` with
`--anim-mouse 507,50` (the urgent button's tooltip, `GetString(4, 1, 8)`) and `--anim-mouse 40,210` (lab 1's
rectangle). The tracer now also pins `reskeringeto` (exe 0xDDF6BC) at 0 and `resatlatszosag` (0xDC5370) at 0.85, as
the client's held frames do. "before" = `main` (3546437) against the same captures, "now" = this change: back faces
culled, the depth range from w = 10 (was 300: the closing's wheel and glass were clipped), the wire sphere lit per
triangle and clipped at the camera plane, the ball's back edges culled, the flying glass unlit in gamma terms,
per-vertex white light without the side lights, faders and lasers over the models, no side laser tails, the row
frame's morph pre-ticked on a held frame. Mean differences (threshold 6):

| Held frame | frame | info_box / start_double | left_list / right_list | labs / labs_right | wheels | ball | Differs (now) |
|---|---|---|---|---|---|---|---|
| open 12 | 1.85 → **1.02** | 0.00 / 0.20 | 6.41 → 6.37 / 3.40 → 3.34 | 6.53 → **0.00** / 6.71 → **0.12** | 2.19 → **0.16** | 0.21 | left_list (sparks, glow sizes) |
| open 30 | 5.27 → **2.92** | 1.95 → 1.42 / 2.76 → 2.09 | 8.56 → 7.38 / 7.78 → 6.79 | 15.65 → 6.98 / 11.33 → **2.13** | 8.62 → **2.63** | 2.16 → 1.41 | lists (lasers), labs |
| open 45 | 7.52 → **4.70** | 2.69 → 2.09 / 2.11 → 1.66 | 6.64 → 5.60 / 6.18 → 5.35 | 15.04 → 21.22 / 4.33 → 4.72 | 8.07 → **3.12** | 7.01 → 4.01 | labs (the left wheel's cyan) |
| open 55 | 6.34 → **4.27** | 3.41 → **0.58** / 3.63 → **1.31** | 6.57 → 5.76 / 5.45 → 4.71 | 3.55 → 4.02 / 3.64 → 4.19 | 14.21 → 8.95 | 5.42 → 3.49 | wheels |
| closing 40 | 12.81 → 6.31 | 12.94 → 7.02 / 11.15 → 5.97 | 8.18 → 5.11 / 3.98 → 3.29 | 33.20 → 26.58 / 8.74 → **3.02** | 7.38 → **0.95** | 21.91 → 9.97 | frame, info_box, labs, ball |
| open 60 | 7.11 → 6.34 | 4.86 → **2.05** / 4.50 → **2.55** | 8.48 → **5.84** / 4.49 → 4.41 | 3.55 → 4.00 / 3.71 → 4.25 | 8.42 → 6.53 | 12.61 → 12.49 | frame, wheels, ball |
| open 60, mouse 507,50 | 7.09 → 6.34 | 4.61 → **2.04** / 4.52 → **2.72** | 8.50 → **5.88** / 4.52 → 4.44 | 3.55 → 4.00 / 3.70 → 4.25 | 8.42 → 6.54 | 12.53 → 12.41 | frame, wheels, ball |
| open 60, mouse 40,210 | 7.10 → 6.33 | 4.89 → **2.08** / 4.46 → **2.51** | 8.34 → **5.73** / 4.43 → 4.35 | 3.55 → 4.00 / 3.73 → 4.28 | 8.42 → 6.53 | 12.45 → 12.32 | frame, wheels, ball |

Bold: the largest gains. The urgent button's tooltip ("Dupla
fejlesztési pénz növeli a sebességet") is drawn at the same place and size in both (inside `info_box`/`start_double`,
2.04 / 2.72). Lab 1's rectangle shows no tooltip in either: the shown tech needs no lab, so `Research_MouseInit2`
keeps 0x13 off (research.md §5.5). The at-rest capture of the top-bar run (`--exe-png _local/screendiff/topbar/…`,
unpinned phases): frame 7.05 → 6.60, wheels 9.60 → 7.76, ball 12.39 → 12.38, labs 3.61 → 4.08, labs_right 3.66 →
4.20 (the lab counters a little darker with the per-vertex light), the rest unchanged.

Left: the side lights (research.md §5.6: the exe's cyan on the left wheel at open 45 and closing 40 is the largest
difference; every model of them tried made the at-rest capture worse), the ball's and the tech models' turn (the
frames drawn since the screen opened: `ball` ≈ 12 at rest), the lasers' random glow sizes and sparks, the closing's
middle wheel (the tracer's hold leaves the turning one shown, research.md §7; the client matches it there: wheels
0.95), the info box glass's depth against the left wheel while closing.

## 2026-10-17, the long info's planet (`starmap-planet`, re/screens/starmap.md §3.3)
The starmap's planets are the exe's meshes now: `b{k}0.xvb` at the end of its level-of-detail chain (`b{k}3`), the
texture's `uv_mul` (the remaster's PVR metadata halves v), lit per vertex from the eye (`StmapLightX/Y/Z`), drawn
over the background particles, and the long info's clouds as the remaster's procedural `CLOUD` material
(`sg3d::create_plasma` noise, bit-exact port; `sg3d_time` from the exe's `acttime`, which the capture tool now
reports). Fresh captures of quick-1.sav (`--out` a scratch folder); "before" is the `starmap-background` client on
its own captures (`_local/screendiff/bg`). The planet region was added to tax/autobuild/priority for this table
(`ig2-screendiff --region planet=0,40,300,330`).

| Screen | Region | before | now | Verdict |
|---|---|---|---|---|
| planetinfo | frame / **planet** | 17.05 / 41.52 | 6.75 / **11.83** | differs (planet −72 %) |
| planetinfo | panel / exit | 4.10 / 3.18 | 3.85 / 3.17 | match |
| tax / autobuild / priority | frame | ~17 | 6.65 / 6.91 / 6.74 | differs |
| tax / autobuild / priority | planet | 42.16 / 42.17 / 42.11 | 11.81 / 12.09 / 11.84 | differs (−72 %) |
| tax / autobuild / priority | panel / menu | 3.89–4.18 / 1.90–2.46 | 3.63–3.91 / 1.93–2.46 | match |
| starmap, production, rop, planets | every region | — | unchanged (bg captures) | |
| transfer | right_panel | 4.69 | 4.56 | match |

Steps on the cloudless capture (`run_screen.py --hide clouds`, planet region): meshes with the near plane fixed (the
mesh pass clipped at 1.0, the eye is 1.09 from the planet's centre, so the client had shown the inside of the far
half) 15.3–23.5 by light direction → `uv_mul` 15.1 → light from the eye 15.7 (the exe's shading, the colour still
off) → planets over the particles 12.2. A second planet (Tra 5, a blue-violet star, `--planet 46`) gave 12.4, so the star colour plays no part.
What is left in the planet region: the cloud rim (the exe's clouds reach a few pixels further past the limb) and the
2-pixel offset of the best match.

## 2026-10-17, the engine's lights (`engine-lights`, re/screens/research.md §2.1)
The 3D screens now light their vertices by the engine's own model (`fenyekszamolasa`, verified on 6,096 vertices of the
research screen through `run_screen.py --light-probe`; `ig2-client::lighting`): the research screen with all four of
its lights (the side lights were off), the spy and diplomacy screens instead of the inferred models (0x80 = full
light, two-sided white). Wireframes (the research wire sphere and ball, the diplomacy cages) take each end's lit
vertex colour; transparent parts are lit too. Same exe captures and states before and after ("before" = `main`
df1b03c built alongside), mean differences (threshold 6).

Research (`--anim-frame F [--anim-closing]`, captures of `research-anim-captures`):

| Held frame | frame | info_box / start_double | labs / labs_right | wheels | ball |
|---|---|---|---|---|---|
| open 12 | 1.02 → 1.00 | 0.00 / 0.20 | 0.00 / 0.17 | 0.16 → 0.07 | 0.21 |
| open 30 | 2.92 → 2.73 | 1.42 → 1.40 / 2.09 → 1.91 | 6.98 → 5.68 / 2.13 → 2.06 | 2.63 → 2.54 | 1.41 → 1.36 |
| open 45 | 4.70 → **2.18** | 2.09 / 1.66 | 21.22 → **3.81** / 4.72 → **1.72** | 3.12 → **0.65** | 4.01 → **1.85** |
| open 55 | 4.27 → **2.56** | 0.58 / 1.31 → 1.23 | 4.02 → 3.34 / 4.19 → 3.33 | 8.95 → **2.73** | 3.49 → 2.37 |
| closing 40 | 6.31 → **2.66** | 7.02 → **0.97** / 5.97 → **0.94** | 27.22 → 12.94 / 3.02 → 2.86 | 0.95 | 9.97 → **3.07** |
| open 60 | 6.34 → 5.52 | 2.05 / 2.55 → 2.48 | 4.00 → 3.25 / 4.25 → 3.32 | 6.53 → **4.15** | 12.49 → 11.66 |

The lists are unchanged (5–7: the lasers' glow and sparks). Closing 40's `labs` is now the info box glass drawn over
the left wheel where the exe draws it behind (research.md §5.6); `ball` at rest is the turn phase.

Spy (`spy:MODE`, the `spy-anim` captures) and diplomacy (the `client-lang` capture):

| Capture | frame | carousel | left_panel / right_panel | ring |
|---|---|---|---|---|
| spy roster | 5.87 → 5.69 | 3.23 → 3.73 | 1.60 → 1.50 / 1.70 → 1.62 | 16.67 → 16.17 |
| spy report | 7.13 → 6.62 | 3.87 → 3.45 | 1.60 → 1.50 / 2.21 → 2.13 | 22.86 → 22.38 |
| spy verdict | 5.83 → 5.69 | 3.12 → 3.55 | 1.60 → 1.50 / 2.75 → 2.67 | 16.52 → 16.06 |
| spy applicant | 6.62 → 6.18 | 3.34 → 3.09 | 1.60 → 1.50 / 2.64 → 2.56 | 21.30 → 20.81 |
| spy opening 12 | 2.78 → **0.93** | 0.19 → 0.05 | 7.29 → **1.69** / 5.91 → **1.86** | 3.35 → **1.44** |
| spy opening 30 | 4.08 → 3.94 | 1.69 → 1.54 | 2.52 / 1.74 → **1.16** | 12.65 → 12.89 |
| spy opening 45 | 4.78 → 4.59 | 3.65 → 3.60 | 1.52 → 1.42 / 1.53 → 1.45 | 12.33 → 12.53 |
| spy closing 20 | 3.79 → **1.14** | 0.30 → 0.17 | 7.34 → **1.44** / 6.80 → **1.39** | 7.75 → **2.92** |

| Capture | frame | info / menu | name | ring | left_heads / right_heads |
|---|---|---|---|---|---|
| diplomacy | 9.67 → 9.16 | 3.95 / 3.51 | 19.86 → 18.68 | 24.43 | 10.62 → **8.66** / 9.04 → **6.60** |

The spy screen's frame no longer takes too much of the side lights' colour while it opens (`spy.md` §9's open
point). At rest the spy carousel is a little worse in two states (+0.4): the exe's capture doesn't pin `spykeringeto`,
so the side lights stand elsewhere than the client's (Agent Bravo's armour greener in the client); the spy and
diplomacy rings are their videos and spins as before.


## 2026-10-17, the remaster's render layer (`render-layer`, re/screens/render.md)
The 3D screens now go through the GOG remaster's own GL layer: the gamma-space mesh pass (`Scene3d::gamma`:
`texture × vertex colour` on the stored values, targets laid over the frame premultiplied), the engine's parallel
light in the colony with the lights' h axis flipped for the file normals, the colony's shadow map
(`rendershadowmap`/`Kockashadow`, render.md §4) and the head videos' `VIDEO` material (scan lines with the exe's
`acttime`). "before" = `main` (be65737) built alongside, same exe captures, threshold 6; best-offset means.

Stored captures (`--exe-png`/`--exe-state` of the top-bar and language runs; research held frames: the
`research-anim-captures` set):

| Screen | Region | before | now |
|---|---|---|---|
| colony 42 | frame / **view** | 16.77 / 18.65 | **10.25 / 12.14** |
| colony 42 | planet_panel / radar / bottom_info / buttons | 13.18 / 8.28 / 12.42 / 19.79 | 13.02 / **5.46** / **8.70** / **12.88** |
| colony 42 | topbar | 0.11 | 0.16 (three pixel columns of terrain seen through a gap in the bar, brighter in the exe) |
| research at rest | frame / start_double / labs / labs_right / wheels / ball | 5.93 / 7.77 / 3.26 / 3.38 / 6.05 / 11.58 | 5.70 / 7.68 / 3.05 / 3.17 / **5.48** / 11.32 (the rest unchanged) |
| research open 45 | frame / lists / labs / labs_right / wheels / ball | 2.18 / 5.59, 5.35 / 3.81 / 1.72 / 0.65 / 1.85 | 1.64 / 5.54, 5.27 / **1.37** / 1.46 / 0.29 / 1.16 |
| research open 55 | frame / info_box / start_double / labs, labs_right / wheels / ball | 2.56 / 0.58 / 1.23 / 3.34, 3.33 / 2.73 / 2.37 | 2.01 / 0.17 / 0.85 / 3.02, 3.03 / **1.02** / 1.89 |
| research closing 40 | frame / info_box / start_double / **labs** / labs_right / ball | 2.66 / 0.97 / 0.94 / 12.94 / 2.86 / 3.07 | 1.74 / 0.55 / 0.58 / **3.03** / 2.39 / 2.16 |
| research open 60 | frame / start_double / labs, labs_right / wheels / ball | 5.52 / 2.48 / 3.25, 3.32 / 4.15 / 11.66 | 5.11 / 2.39 / 3.04, 3.12 / 2.87 / 11.35 |
| research open 12, 30 | every region | — | equal or lower (open 12 wheels 0.07 → 0.05) |
| info, messages | every region | — | unchanged (their frames stay on the linear pass, render.md §5) |

Fresh captures with the exe's `acttime` (`run_screendiff.py diplomacy | spy:roster`, `_local/screendiff/render-fresh`):

| Screen | Region | before | now |
|---|---|---|---|
| diplomacy | frame / **ring** / left_heads / right_heads | 9.01 / 23.84 / 8.69 / 6.58 | **8.26 / 21.04** / 8.66 / 6.56 |
| diplomacy | info / menu / name | 3.98 / 3.59 / 19.64 | 3.98 / 3.59 / 19.66 |
| spy roster | frame / **left_panel / right_panel** / **ring** | 5.36 / 1.49 / 1.62 / 16.23 | 5.16 / **0.25 / 0.54** / **14.83** |
| spy roster | carousel | 3.60 | 3.62 (the side lights' phase, spy.md §9) |

Starmap screens (the `starmap-background` captures with their clocks, `_local/screendiff/bg`):

| Screen | Region | before | now |
|---|---|---|---|
| starmap | frame / **view** | 5.35 / 5.49 | 2.77 / **2.84** |
| planetinfo | frame / panel / planet / exit | 10.68 / 3.85 / 23.56 / 3.18 | 9.28 / 3.19 / 22.12 / 2.77 |
| tax / autobuild / priority | frame | 10.89 / 10.95 / 10.88 | 9.55 / 9.62 / 9.52 (panels and menus lower too) |
| rop | frame / icons | 5.18 / 3.78 | 2.68 / 1.71 |
| planets | frame / menu | 5.40 / 3.27 | 3.63 / 1.62 |
| transfer | frame / left / right / scroll_bar / exit | 5.41 / 5.17 / 4.56 / 20.90 / 3.21 | 4.53 / 5.05 / 4.42 / 20.19 / 2.77 |
| production | frame / left / right / exit | 5.46 / 6.38 / 5.18 / 3.68 | 3.64 / 5.06 / 4.24 / 3.57 |
| production | name_plate | 6.52 | 7.40 (the planet behind the plate's corner: its rim matches now, its lit centre is 7 levels brighter than the exe's) |
| every screen | topbar | 0.09–0.10 | 0.06 |

Tried and left out: the info/messages frames in the gamma pass (every region worse with their guessed two-sided light,
e.g. info race_names 11.9 → 18.1), the engine's shade table for the starmap planet (planetinfo planet 22.1 → 22.5).

## 2026-10-17, the info and messages frames lit by the engine (`infomsg-lights`, re/screens/info.md §1.1, messages.md §4)
The two screens' 3D frames now take the engine's per-vertex lights (`lighting::Lights`: ambient, the white parallel
light, the two coloured side lights, info's circling with the exe's `infokeringeto`, the info heads' marks) in the
gamma pass, laid over the frame with `Blend::GammaPremul`, instead of the guessed two-sided Lambert light on the linear
pass. "before" = `main` (cd7a117) built alongside, same exe captures, threshold 6; best-offset means. Fresh captures
(`_local/screendiff/inflight`, the exe's `acttime` and `infokeringeto` 0.3744):

| Screen | Region | before | now |
|---|---|---|---|
| info | frame / heads / **race_names** / **triangle** | 10.92 / 5.78 / 11.87 / 6.73 | 9.64 / 3.90 / **5.99** / **2.35** |
| info | empire_panel / chart / **buttons** | 11.93 / 14.71 / 12.16 | 10.42 / 14.06 / **9.25** |
| info | list | 22.52 | 22.63 (the four planet discs: the exe puts the planets' textures on them, not ported; lit right they are a brighter teal. Without them, x ≥ 66: 20.50 → 20.08) |
| info | topbar | 0.05 | 0.05 |
| messages | frame / **panel** / **inbox** / topbar | 2.85 / 14.89 / 12.29 / 0.05 | 0.10 / **0.30** / **0.06** / 0.05 |

The stored info capture (`topbar/info-exe.png` + `lang/info-state.sav`, no `infokeringeto`: 0) gives the same picture
(race_names 11.86 → 5.99, triangle 6.73 → 2.35, buttons 12.15 → 9.22, chart 14.71 → 14.21, list 22.52 → 22.64).
Left: the info frame is still darker than the exe's (upper frame luma 26.7 against 35.2): the empire panel's blue
glass, the planet list and the chart's background are 2D or unported; the pillar's faces turned from the lights are
dark in the client and grey in the exe.

## 2026-10-17, the info screen's planet/fleet list (`info-list`, re/screens/info.md §6)
The list's rows (`KirakBolygoInfo`/`KirakFlottaInfo`: name, owner, population, the morale or a problem, production),
the faders `PutFaders` puts under the frame (the rows', the empire panel's, the chart's, the scroll bar; `dt.bmp`
textured, the panels' textures are transparent inside), the planet globes `bolygo_k.xvb` with the starmap's `p<n>.bmp`
(and its `uv_mul`) on `mainmeshid`'s lights, the frame's list discs hidden once open. The capture tool now records the
list's pointer, problem step, `Counter` and the four slots' spins and rotations (`InfoBF`), handed to the client as
`--info-list`/`--info-globes`. Fresh capture (`_local/screendiff/infolist`, `infokeringeto` 0.3768, step 1); "before" =
origin/main (f1a7be7) built into its own target on C:, same capture and state save, threshold 6, best-offset means:

| Region | before | now |
|---|---|---|
| **list** | 22.55 | **2.79** (luma 26.9 / 27.6) |
| **empire_panel** | 10.42 | **1.13** |
| **chart** | 14.09 | **3.28** |
| **frame** | 9.61 | **2.32** |
| buttons | 9.23 | 7.56 |
| heads / race_names / triangle / topbar | 3.79 / 5.85 / 2.35 / 0.05 | unchanged |

Every region but `buttons` matches now. Left in the list: the row lines (`MeghuzIzeVonalak`) are a little brighter
than the exe's, the globes' lighting at their rims; the detail mode, `AtteszMatrixokat`'s swap when scrolling and the
opening's translucent discs aren't ported.

## 2026-10-17, the info screen's detail mode, star log and matrix swap (`info-detail`, re/screens/info.md §6.6–§6.9)
The list's detail mode (`InfoNagyKiirBolygo`/`InfoNagyKiirFlotta`, the selected globe sliding to (−43, 28, −150) at
30 % opacity, the others fading out, the detail faders and lines), `AtteszMatrixokat`'s swap of the slots' turns when
the list scrolls, and the star log (`Info_PutStarLog`, the star-log mode's button textures and the frame posed on to
75). Captures press the hot-spots through the exe's input (`run_screendiff.py … info:IDS [--fleets N] [--starlog N]`,
info.md §6.9; `_local/screendiff/infodetail`). Found on the way: `line2d` reads its colour words as 0x00BBGGRR (the
row lines are (0, 34, 50), one pixel, gamma-space), the fleet model `flag10.xvb` is the head of a detail chain
(`flag12` drawn), the models' faces turned away are culled and their lighting normals normalised, the chart marker
is additive, and the gamma pass's glass keeps an "over" coverage so translucent models in a rendered frame let the
2D under them through (`render3d`; no change on the spy, diplomacy, research, messages and colony captures).

Chart mode, the earlier capture (`infolist`), "before" = HEAD's client built into its own target on C:

| Region | before | now |
|---|---|---|
| **list** | 2.79 | **1.67** |
| **buttons** | 7.56 | **5.02** |
| frame | 2.32 | 1.85 |
| heads / triangle / chart | 3.79 / 2.35 / 3.28 | 3.62 / 2.26 / 3.14 |
| empire_panel / race_names / topbar | 1.13 / 5.85 / 0.05 | unchanged |

New captures (every region matches):

| Capture | list | frame | buttons | chart | other regions |
|---|---|---|---|---|---|
| `info:9`: a planet's detail | 1.11 | 1.71 | 4.98 | 3.05 | ≤ 5.99 |
| `info:31×8+12 --fleets 2`: a fleet's detail after 8 swaps | 1.38 | 1.83 | 5.27 | 3.33 | ≤ 5.90 |
| `info:31+31+31 --fleets 2`: fleet rows, 3 swaps | 1.52 | 1.76 | 5.31 | 3.02 | ≤ 5.95 |
| `info:15`: the star log (empty after a load) | 1.59 | 1.67 | 2.28 | 2.64 | ≤ 5.85 |
| `info:15+35+35 --starlog 20`: 20 staged lines, scrolled 2 | 1.46 | 1.72 | 2.28 | 3.72 | ≤ 5.95 |

Left: in chart mode the exe's sixth left button (the star log's) shows a blank plate, the port its `Info_b6`
computer (unexplained, info.md §7); the chart's fade-out between `infoframe` 45 and 60.

## 2026-10-17: the info screen's planet settings menus (`info-select`)

The detail mode's select menus (info.md §7.1: `PutFaders`' box on fader pass 2, `InfoGfx2D_3`'s items, the item under
the pointer in colour 6), held open in ig2.exe by `run_screendiff.py … info:9 --select K,X,Y,I` (`run_screen.py
--info-select`) on the human's first planet of quick-1.sav; the region `select` is the menu's box:

| Capture | select | list | frame | buttons | other regions |
|---|---|---|---|---|---|
| `info:9 --select 26,150,340,2`: taxes, item 2 | 0.90 | 1.05 | 1.69 | 4.98 | ≤ 5.99 |
| `info:9 --select 27,160,300,4`: auto-build, item 4 | 1.04 | 1.26 | 1.75 | 4.97 | ≤ 5.99 |
| `info:9 --select 28,150,370,3`: priority, item 3 | 0.85 | 1.04 | 1.69 | 4.98 | ≤ 5.99 |

Every region matches; the menus' texts and boxes sit at offset 0. The triangle and the limit change no picture beyond
the knob and the texts already compared; their messages are verified in the save replay
(re/rules/traces/2026-10-17-info-orders.md).

## 2026-10-22, the research screen as a frame sequence (`sound-tracer`, re/screens/sound.md §3–§4)
The first sequence comparison: `run_screendiff.py <install> quick-1.sav research --script tools/trace/scripts/research.json`
(the opening, the urgent button's tooltip and press at t 300/330, PgDn at 400, ↓ at 470, D at 530; 19 grabs; times in
UI ticks). The exe ran one UI tick per drawn frame and every grab came back at exactly its tick (glReadPixels at the
swap). Best-offset means, threshold 6 (`_local/screendiff/seq/research-seq-seq.json`):

| t | frame | info_box | start_double | left_list | right_list | labs | labs_right | wheels | ball |
|---|---|---|---|---|---|---|---|---|---|
| 10 / 30 | 1.47 / 1.44 | 0.00 / 0.00 | 0.36 / 0.31 | 2.49 / 5.37 | 2.33 / 5.17 | 0.00 | 0.00 | 2.63 / 0.48 | 0.14 / 0.31 |
| 60 | 3.72 | 0.00 | 0.10 | 5.14 | 3.60 | 0.08 | 0.15 | 4.45 | 5.59 |
| 100 | 11.06 | 9.91 | 10.19 | 13.10 | 6.47 | 19.94 | 5.62 | 5.10 | 17.79 |
| 150 | 6.75 | 0.78 | 6.04 | 8.44 | 16.24 | 3.77 | 4.16 | 10.39 | 4.55 |
| 200–400 (at rest) | 4.4–5.1 | 1.8–2.5 | 2.1–2.6 | 4.2–6.3 | 3.1–5.2 | 3.0–3.5 | 3.1–3.4 | 3.1–3.8 | 9.2–11.6 |
| 320 (tooltip) | 5.02 | 5.14 | 6.11 | 4.55 | 3.08 | 3.05 | 3.21 | 3.52 | 9.77 |
| 420–600 (PgDn) | 4.6–9.8 | 6.5–21.3 | 5.1–15.7 | 12.3–14.0 | 4.7–6.8 | 9.0–20.2 | 9.2–20.9 | 2.3–4.4 | 3.5–16.8 |

The top bar stays at 0.05–0.58. What the sequence shows (screendiff-review): the opening's start (t 10, 30) matches;
from t 60 the poses drift (the sphere's size and turn, the corner light streaks), at t 100 the panels' fade band sits
~17–20 px lower in ours, at t 150 ours isn't open yet (one tick behind: deviations #9) so the list text and lab
counters are missing. At rest only the ball's model turn differs. The tooltip shows in the exe at t 320, in ours
later (#10); the exe draws the pointer (#11). From t 420 the two show different categories: PgDn goes the other way in
ours (#6). Sounds of the run: re/screens/sound.md §4.

## 2026-10-22, the main menu as a frame sequence (`mainscr`, re/screens/mainscr.md §13–§14)

`run_screendiff.py main --script tools/trace/scripts/mainscr.json` (the exe booted to the menu with `run_screen.py --boot`, no save). Frame mean 2.6 / 4.0 / 5.5 at t 5 / 60 / 120 (pass), 6.6–8.0 from t 180, 10.0 at t 360 (the fly-out). Rings, labels and hover match; the leftovers are deviations.md rows 17–20 (row 3 updated). Sounds: no PlaySound-family call on any ring action; the scene's sounds go through `MainPlaySound` (mainscr.md §13).

## 2026-10-22, the SpaceWar screen at rest (`spacewar-screen`, re/screens/spacewar.md §9)
`run_screendiff.py <install> S.sav spacewar:<grab>.json` (grabs from `run_spwc.py --grab-ticks`; two battles, two moments each, 800 x 600; regions topbar, view, console, info_box, radar, weapons; `_local/screendiff/spacewar/`). Best-offset means:

| grab | frame | topbar | view | console | info_box | radar | weapons |
|---|---|---|---|---|---|---|---|
| grab3 t600 | 6.07 | 5.21 | 5.19 ✔ | 10.04 | 5.51 ✔ | 10.55 | 10.92 |
| grab3 t1200 | 6.20 | 5.21 | 5.21 ✔ | 11.37 | 5.39 ✔ | 10.57 | 10.92 |
| grab4 t300 | 8.08 | 5.33 | 8.18 | 10.34 | 6.33 | 10.18 | 8.74 |
| grab4 t800 | 8.04 | 5.33 | 8.18 | 9.84 | 6.16 | 10.22 | 8.74 |


Positions match (planet limb, ships, weapon buttons, radar dots and frustum, info text; brackets within 2 px, slightly larger). Differences: light and colour (planet and weapon buttons a little brighter, the panel's moving side lights, the radar frustum fainter), the grid's look, the info model's spin phase (not dumped), the cursor (exe only), the top bar's speed buttons (battle state not drawn). Listed in `re/deviations.md` #21-#27.

## 2026-10-22, colony ground war (`grwar:17`, re/screens/colony.md §11.6)

`run_screendiff.py … quick-1.sav grwar:17`: frame 30.4 -> 13.45. Top bar 2.59 (match), texts 4.80 (match; the attack
banner), radar 9.06, bottom_info 14.95 (the exe's unit picture is missing), buttons 51.99 (the exe's terrain shadow
band, not the buttons: the retreat button matches), view 8.28; the rings line up. regression-sweep: colony 42 shows no
regressions (frame 10.24, bottom_info 8.10 better). Sound log: a move order plays Speech::Play; the buttons click only
when they act.

## 2026-10-22, the starmap's fleet and vehicle info panels (`starmap-fleet-panels`, re/screens/starmap.md §19–§20)
`ig2-client::starmap_info` draws the fleet panel (`StarmapFleetMode`: the info box's left release in fleet mode) and
the vehicle panel (`StarmapTanksInfo`: radial icon 9): cells, faders, texts, the preview model, the DesignKeret1 frame,
scrolling and the sale menus. Captures of quick-1.sav, planet 71 (`run_screendiff.py` screens `fleetinfo` / `tanksinfo`,
wait 420); mean difference per region, best offset:

| Screen | frame | topbar | list | model | data | scroll_bar | exit |
|---|---|---|---|---|---|---|---|
| fleetinfo | 4.70 | 0.06 | 5.56 | **6.53** | 4.45 | **6.91** | 2.97 |
| tanksinfo | 4.73 | 0.06 | 5.23 | **8.42** | 3.71 | 4.31 | 3.44 |

Left: the preview models' spin phase (vehicles) and the fleet preview's pitch/material (the yaw is `InfoAlapRotation`
mapped to pi - t [obs at t = 0.91 and 2.69]); the models' warmer, darker lighting; the faders' random texture
drift/shine (hud.md §1.2, not drawn); the fleet glow on the map under the scroll bar is slightly brighter. The exe runs
Hungarian texts. Sounds verified with the sound-id tracer (`tools/trace/scripts/fleetinfo.json`). Deviations #41-#48.
