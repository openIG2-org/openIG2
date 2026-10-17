# The spy screen (`AktKep` 7)

How the original draws and drives the espionage screen. Evidence: the ELF oracle decompiled with
`tools/ghidra/DecompileFunctions.java` (addresses ELF unless marked exe; `GetString` indices read from the asm, the
decompile drops the third argument), the exe's `Spy_Main` and chance estimate decompiled and their asm checked,
runtime captures of ig2.exe (`tools/trace/run_screen.py --kep 7 --spies MODE`) and direct calls of the exe's estimate
under Frida (`run_spy.py --estimate N`). `[inf]` marks inference, `[obs]` what only the captures show. The sim side
(the spies, missions, reports and the orders `MULTIGET_Spy` applies) is `re/rules/espionage.md`.

Port: `engine/crates/ig2-client/src/spy_screen.rs` (state, input, `spy_screen/draw.rs` the 2D pass) and
`spy_scene.rs` (the 3D pass); the orders are `ig2_net::Command::Spy`. **Status**: drawn, driven and compared (§9):
the carousel and both panels match the captures, and so do the opening's frames 12, 30, 45 and the tooltip; the
ring with the head video differs by the video's frame and the ring's spin (clock-dependent). Verified: the chance
estimate bit for bit against 600 exe calls, the verdict roll against 600 exe presses.

| Function | ELF | exe | Role |
|---|---|---|---|
| `Spy_Belep` | 0x6c5a70 | 0x58cd50 | enter: texts, state, `Spying3DInit`, the first report (§1) |
| `Spy_Kilep` | 0x6c5de0 | 0x58d250 | leave: the closing animation loop, `SaveScreen` before help/options/messages |
| `GFX_Spy` | 0x6c6080 | 0x586aa0 | per frame: the faders (§5.2), `GFX_DateMoneyScrollFade`, `Spying3DUpdate` |
| `Spying3DInit` / `Spying3DUpdate` | 0x6bad60 / 0x6c4b10 | 0x58fd10 / 0x590410 | the 3D scene (§2) |
| `SpyGfx2D_1` / `_2` / `_3` | 0x6bb6e0 / 0x6bb7c0 / 0x6c3a10 | – / 0x588bd0 / 0x58c1d0 | the 2D passes in the scene's callbacks (§3–§5) |
| `KemekFeliratai` | 0x6b95d0 | | the names on the pedestals (§3) |
| `BeallitKemekAtlatszosaga` | 0x6ba0e0 | 0x583cb0 | the carousel's turn, fades, status icons, a click on a spy (§2.3, §6.4) |
| `KozelitAktKemre` | 0x6baa60 | 0x588220 | turn the carousel to the selected spy (§6.4) |
| `FRISSIT_Spy2` | 0x6d1e20 | | the 100 Hz UI frame (§1.2) |
| `Spy_Main` | 0x6c7360 | 0x58d410 | input (§6) |
| `Spy_MouseInit` / `Spy_MouseInit2` | 0x6ca930 / 0x6c99c0 | 0x58f0f0 / 0x58ec70 | the hot-spots and their switches (§6.1) |
| `SpyKeyboardHandler` | 0x6c5770 | | ←/→ (§6.5) |
| `KemEselySzamolas1` | 0x6ca200 | 0x587b80 | the planner's escape estimate, stream 0 (§6.3) |
| `KemEselySzamolas2` | 0x6ca620 | 0x587de0 | the success chance (`espionage.md` §5.1, `ai-empire.md` §6.5) |
| `Spy_MennyiPenzbeKerul` | 0x6cb9c0 | inlined | an applicant's wage (`espionage.md` §8, 0x4b) |
| `SpyEsemenyEllenoriz` | 0x6c5d30 | inlined | show the first report (§4) |
| `SpyInitNewMission` / `SpyDeleteNewMission` | 0x6b92a0 / 0x6b9340 | | the planner on / off |

## 1. Frame structure

### 1.1 Enter and leave
`Spy_Belep`: `TrainingKuldetesSzoveg[1..=5]` = `GetString(7, 6, k)` ("1.szintű kiképzés"…), `SpyCelTipusNevek[1..=5]`
= `GetString(7, 6, k + 5)` (planet, building, ship, tech, level); sounds `Spy_Hangok`; `AktBrushok = SpyBrushok`;
`KemNevVillogtato` 0 rising; `SpyOldKem` −1; **`AktKem` = 1**; `spyAtfordulas` = 30 (no flip running);
`SpyInitNewMission`; no list, dialog or kill question; `SpyEsemenyEllenoriz` (§4); `spyKELLKINYITAS =
KELLKINYITASDEFAULT & 1` (the opening animation: `spyframe` 0 → 50; off → 50 at once; `KELLKINYITASDEFAULT` exe 0x912A39 is a registry setting, 1 on this host
[obs: the tracer reads `spyKELLKINYITAS` exe 0xE75430 = 1]); `Spying3DInit`. The screen
loads no picture (`Screens[7]` is empty, `ScreenBetolt` ELF 0x669670): the background is black, and
`Copy3hatterkep(0x34, …)` in `Spying3DUpdate` draws nothing (kep 0x34 is the design screen's, not loaded).

`Spy_Kilep`: unless leaving for messages (10), help (1) or options (13), it plays the closing (`spyBECSUKAS` exe
0xE75383, `spyframe` 50 → 0 by 0.6 a UI frame, sound `screens/spy/scrn08ki.wav`) in a loop of `UpdateFrame` +
`FRISSITES` with `AktKep` held at 7, until frame 0 was drawn once (`voltnulla`) and one more frame; for 10/1/13 it
saves the screen (`SaveScreen(2)`) as their background. `spyKINYITVA` stays set while closing; the 2D passes
(`GFX_Spy`'s faders, `SpyGfx2D_2`, the names) draw only while it is set and `spyBECSUKAS` clear.

### 1.4 The animation (`Spying3DUpdate`, per drawn frame; exe 0x590410)
With `f = spyframe/50` (exe f32 0xE75504):
- `SetFrame(SpyMainMesh, spyframe)`: the frame `spybck2` follows its node keys (121 frames; `Kocka` ELF 0x5f6a40
  interpolates keys and translations at `spyframe·256` in 1/256 steps: `translation = record + key(f) − key(0)`,
  `matrix = parent · key(f) · record rotation`). The panels `Group59`/`Group60` swing open, `Kezelo_radar` rises,
  the sphere `Sphere01` grows from scale 0.
- Child 1 (the telescopes `Group18`; its children 4 and 5 hidden for good by `Spying3DInit`) shows and `SpyKozep`
  (the stone ring) hides while `!spyKINYITVA && spyframe < 25` or `spyBECSUKAS`; else the reverse.
  `spyKINYITVA` is set (and `IdoPauseMertAtlepesAlattVan` cleared) once `spyframe ≥ 50`.
- The sphere `Gomb` (child 0): while `spyKELLKINYITAS`, `MoveObj(Gomb, spyInitPos1 + (0, (1 − f)·200, −(1 − f)·1000))`
  (`spyInitPos1` ELF 0x171aae0 / exe 0xE75594 is never written: 0) and `RotateObj(Gomb, sin((1 − f)·2)·3, sin((1 −
  f)·2.1)·2, 0)` (the first product in f32, the second in f64). `MoveObj` replaces the child's own translation
  (ELF 0x455f80: record `+0 = v/1024`), so at rest too the sphere sits at `Gomb`'s origin, not at the file's
  (−0.12, 0.09, 0.16) model units [obs: the at-rest capture's carousel 3.30 → 3.22]. `SetFrame(Gomb, spyframe +
  spywireframe)`; transparency `1.35 − 2·spyframe/50` up to frame 25, else 1e−6 (`spySetObjTransparencyH`
  ELF 0x6b9c00: clamped to 1e−6..0.9999999). Transparent, its lines are `Racsszin × opacity` in the frame's gamma
  space; opaque, ambient-lit (§2.2) [obs: closing frame 20 (opacity 0.45): (4, 45, 59) on a line, (6, 71, 92)
  where two cross]. Only the triangles facing the camera draw their edges [obs: the sphere seen from outside while
  it flies in shows one side's grid; at rest the camera is inside].
- From frame 25 and not closing: the ring's spin and flip (§2.2); the head video's disc shows once a flip passes
  its frame 15 (`UnhideObj(spyKorId)`; `Spying3DInit` shows it at once when not animated).
- Closing: the disc fades (`spySetObjTransparencyH(spyKorId, 1 − f)`) and drops to `(0, (1 − f)·70 + 182, −1000 −
  (1 − f)·700)`.
- `SpyPalast` (the carousel) drops in from `(0, −125 − (1 − f)·150, −1000·f)` turning `−sin((1 − f)·π)` (§2.2).
- `Copy3hatterkep(0x34, …)` draws nothing (§1.1); `spyANIMBARKACS` (the 2D video path) is off.

### 1.2 `FRISSIT_Spy2` (100 Hz UI frame)
- `KemNevVillogtato` runs 0 → 100 → 0 by 1 (the selected name's blink); `spyKiirSzinSzorzo` += 1 up to 100 once
  open (the names fade in); `spykeringeto` += 0.0006 (drives the lights and the hourglass).
- Opening (`spyKELLKINYITAS`, not closing, not yet open, `spyELINDULT` = a frame was drawn): `spyframe` += 1/3 up
  to 50; closing −0.6 down to 0. `SpyPalastRotation` is first kept in [2π, 4π) by steps of 2π/5.
- The ring's flip: with `spyframe ≥ 25`, `spyAtfordulas` += 0.2 until 30; at 30 `spyAtvaneFordulva` toggles 0 ↔ π.
- Open: the pointer within 5 px of the left/right edge with 50 < y < 240 accelerates the carousel
  (`ElfordulasiSebesseg` ∓0.006 up to ±0.1), `SpyPalastRotation += speed/5`; away from the edges the speed decays
  ×0.92 (and at the edges `RaalltKem` = the selected pedestal). The rotation is kept in [2π, 4π).
- The cursor: a dead `AktKem` drops the planner; it moves up past dead slots, then down; a spy at home with no
  dialog, no report waiting and no mission just sent gets a planner (`SpyInitNewMission`); a planner of a spy no
  longer at home is dropped; a dead target empire is cleared; a planet target no longer the empire's is cleared.

### 1.3 Per frame (`GFX_Spy`)
The faders (§5.2), `GFX_DateMoneyScrollFade` (the top bar, `hud.md` §2), `Spying3DUpdate` (§2). The 2D passes run
as the scene's callbacks in this order: `SpyGfx2D_1` (after the carousel), fader pass 1, `SpyGfx2D_2`, fader pass 2,
`SpyGfx2D_3`, fader pass 3 (`InitFader1..3`, `FaderHanyadikMenet`: the lists' boxes go to pass 2).

## 2. The 3D pass

### 2.1 Camera and lights
`Spying3DUpdate`: `forgaspont` = `kamerapont` = 0 except `kamerapont.z` 600 [unused here], angles 0, `xcenter` 320,
`ycenter` 240: a world point `(x, y, z)` lands at `(600·x/−z + 320, 600·y/−z + 240)` (fov 600 as `starmap.md` §2.2;
+y down). `ModifyViewPort` widens the virtual screen for the window.
Lights: ambient 0x141414; `CreateLightParalell(0x808080)` at (0, 0, −10); `spyOldalsoFenyekBe` 0x7f5d00 and 0x004e7f
at (200, 0, −1000), moved every frame by `spyMozgatMainFenyek` to `(sin(20k)·300, cos(20k)·200, −1000)` and the
same at +1.57075 (`k = spykeringeto`); the selected pedestal gets an ambient light `(sin(90k) + 1)·127` grey
(`SpyRarakFeny`, the pedestal's own light after the inherited ones). Lit by the engine's model (research.md §2.1,
verified on the research screen; `ig2-client::lighting`): the colour words as bytes `[r, g, b]` (0x7f5d00 teal,
0x004e7f orange), transparent parts lit too; the engine's gamma-space products are mapped into the sRGB frame
(`spy_scene::gamma`). It replaced an inferred model (0x80 = full light, the white light two-sided) and took the
opening/closing captures' panels from ~7 to ~1.5 (diff-results.md `engine-lights`).

### 2.2 The models (`VectorDir = vector/spy/`, `TextureDir = textures/spy/`)
| Object | Model, `AddToScene` scale, `MoveObj` | Shown |
|---|---|---|
| `SpyMainMesh` | `spybck2.xvb`, 0.45, (0, 2, −650); `SetFrame(spyframe)` (node keys, 121 frames) | child 0 `Gomb`: a sphere drawn as wireframe (`PropertiesOn(0x40)`), `SetFrame(spyframe + spywireframe)` (at rest 50..60, `spywireframe` += 0.25 a frame, wrapping at 10); its colour (1, 8, 10) [obs] = `Racsszin` (8, 101, 132) × the ambient. Child 1 (telescopes) hidden once past frame 25. Children 2/3: the corner brackets (`4hand`) and the panels' fills `Object05`/`Object06` at `SetObjTransparency(0.8)` (the panels' (2, 20, 26) [obs] = `Racsszin` × 0.2). Child 4 `Kezelo_radar` |
| `SpyPalast` | `spybck2a.xvb`, 0.75, (0, −125 − (1 − f)·150, −1000·f), `f = spyframe/50` | the carousel (§2.3) |
| `SpyKozep` | `spybck2b.xvb`, 0.303, (0, 182, −1000) | the stone ring (`belsogyuru`, `bgyuru.bmp`); children 2, 3 hidden; shown past frame 25 |
| `spyKorId` | `kor.xvb`, 0.173, (0, 182, −1000) | the disc inside the ring; its texture is the head video (§2.5) |
| `Spy_Status_Idk[i]` | `negyzet0..b.xvb`, 0.012 | the status icon of pedestal `i` |

The ring turns `RotateObj(SpyKozep, 0, 0, spyFrameValtozo/220)` (`spyFrameValtozo` += 1 per drawn frame), then
`RotateObj2(axis y, spyAtvaneFordulva)`; the disc only the latter. While `spyAtfordulas` < 30 both turn a further
`spyAtfordulas · 0.10471976` about y and come forward: `(0, 182 − 2a, −1000 + 40a)` for `a < 16`, else with
`29 − a`; past 15 the new head video starts (`MakeAnimName`). The flip starts when `AktKem` changed and the last flip
ended (`Spying3DUpdate`), with the sound `screens/spy/screen21.wav`.

### 2.3 The carousel (`BeallitKemekAtlatszosaga`)
`spybck2a`'s twelve root children are the pedestals: child 0 the base (`ap`, `spykp.bmp`, the name plate), child 1's
child 0 the figure (`lap`, a textured quad = `KepernyonKemek[i]`). `KigyujtKemek` (ELF 0x6b9450) puts the roster's
working slots (state > 0) on pedestals 0.. in slot order, the rest empty (`−1`); `KikeresKem(AktKem)` (0x6b9550) is
the selected pedestal. `RarakKemekTexturai` (0x6b9f30): an empty pedestal's figure is hidden, a spy's figure takes
`KemTexturaNevek[race − 1]` = `<race>_a.bmp` (alpha-cut).
Per frame: `RotateObj(SpyPalast, 0, rot, 0)` (`rot = SpyPalastRotation`, while opening `− sin((1 − f)π)`), each
pedestal `RotateObj(0, −rot, 0)` (faces the camera). Pedestal `i`'s place `φ = fmod((rot/2π)·12 + i − 5, 12)`:
for `φ ≤ 3` the fade `t = φ/3`, for `φ ≥ 9` `t = 1 − (φ − 9)/3`, else `t = 1` (hidden behind); then `t ≤ 0.7 → 0`,
else `(t − 0.7)·3.3333`. `t` is `KepernyonKemek[i].+0x24` and the transparency of the pedestal, its figure and
icon. The status icon sits at the group's centre + (30, 47, 0): state 0/1 `spy_status_0.bmp`, 2 (travelling home)
`spy_status_2.bmp`, 4 (captured) `spy_status_3.bmp`, 3 (on a mission) the hourglass `010000.bmp` + `(int)(k·240) % 32`.

### 2.4 Mouse on the carousel
A left press at virtual `(x, y)` hits pedestal `i` when, with `p` = its group's centre on screen and
`f = 20 − (z + 1000)/40`, `p.x − (50 − f) < x < p.x − (50 − f) + 100 − 2f` and `p.y − (70 − f) < y <
p.y − (70 − f) + 180 − 2f`; of several the nearest (largest `z`). It selects the spy when `z > −880`, the slot is
working and no dialog is up (sound 0, the pedestal lights dropped, `SpyInitNewMission` for a spy at home).
`SpyLenyomta` (a drag turning the carousel) is never set in this build [inf].

### 2.5 The head video
`MakeAnimName` (0x6ba040): `anim/fejek/` + `KemFejNevek[race − 1]` (ELF 0x119a040: human, ufo, politikus,
radioaktiv, brutal, dyno, lamer, tremor, keresk, keresk, kaloz, scorpus, scorpus, b5, worm; no spy: `ures.ani`).
`InitAnim(name, …, 25 fps)`; `FrissitSpyingAnim` (0x6b93e0) plays it onto the disc's texture (`PlayAnimVector`)
past frame 25. The capture shows every other row at half brightness [obs: 27 vs 54] (`DrawAnimInterlace`).
`keresk.ani` is not in the install (`keresek.ani` is): races 9 and 10 have no video [inf].

## 3. Names (`KemekFeliratai`, from `SpyGfx2D_1`)
Open and not closing: for each pedestal, `p` = the centre of its base on screen; the name (`Kemek[EN][slot]`, or
"---") at `(p.x − 40, p.y − 2)`, 14 cells, size 1, flags 0x4d / 0xd (centred in the 14 cells, clipped: centred on
the pedestal), colour 2, `KiirStrSzinSzorzo = (1 − t)·100·spyKiirSzinSzorzo/100` (the selected one also
`× KemNevVillogtato/100`); pedestals with `t ≥ 0.8` get none. `SpyGfx2D_1` also queues `FadeKeret(6, 248, 172,
217)` and `(464, 248, …)` in (0, 45, 90) at 128 past frame 35, which no capture shows [obs]: the port leaves them out.

## 4. Texts and the reports
`StringReadAll` (ELF 0x531e40) fills from the Hungarian `txt17` (every language, `hud.md`): `Faj_Nev[k]` (17, 1,
k + 5), `Birodalom_Nev[e]` (17, 1, e + 33), `Rang_Nev[k]` (17, 2, k), `KemKuldetes_Nev[m]` (17, 2, m + 20),
`KemStatusz_Nev[s]` (17, 3, s), `KemBeszolas_Szoveg[o]` (17, 3, o + 4), `KemCsaladtag_Nev[k]` (17, 3, k + 9),
`KemMitortentAFellazadtBolygoval_Szoveg[k]` (17, 3, k + 14), `SpyAreYouSure_Nev[0..3]` (17, 6, 43..=45). The rest is
`GetString(7, s, i)` in the game's language, in source order (the asm's immediates): section 2 = mission reports
(3..29), 3 = more reports and dialogs (1..33), 4 = dialogs and buttons (1..34), 5 = the panels (1..28), 6 = the
training levels and parameter kinds. "Skill Points:" is an English literal in every language.

**The dialog** (`SpyBeszolasMi`) is the first report's kind (`KemBeszolasok[1]`, `info-messages.md` §3.3; `+4` = the
spy, `+8` outcome, `+9` success, `+0xc..` `x6..x14`). `SpyEsemenyEllenoriz`: with the screen up, no planner with a
mission chosen, no dialog and no mission list open, and a report waiting: `SpyBeszolasKemBackup = AktKem`,
`SpyBeszolasMi = kind`, `AktKem = +4`. Closing a report (`KemHanyBeszolas −= 1`, entries 2.. move up) returns the
cursor to the backup.

## 5. The 2D pass (`SpyGfx2D_2`, `GFX_Spy`)

### 5.1 Texts (virtual pixels; `KiirStr` size 2, flags 4, rows 0x11 apart from y 0x100 on the right)
- Left, the spy on show (`AktKem > 0`): labels (7, 5, 1..4) at x 0xc, values at 0x3b (name, race, experience,
  rank) from y 0x100 by 0x10; skills (7, 5, 11..14) at y 0x142 by 0x10, values at 0x65 right-aligned in 2 cells
  (flags 6), "+" (size 3, flags 5, colour 1) at 0x70 when the spy has skill points and state 1..3; then
  "Skill Points:" and the count at y 0x142 + 5·0x10. Colours: labels 1, values 2.
- Bottom left: `(7, 5, 15) + " " + HanyKemVan[EN] + "  " + (7, 5, 16) + " " + HanyKemLehet[EN]` at (0xc, 0x1bf), size 3.
- Right, no dialog, no planner: state 3: "Küldetés:", the mission (0x1de, 0x111), "Célbirodalom:" and the empire when
  the mission has a target, the parameters (`SpyCelTipusNevek[kind]` at 0x1d6 in 9 cells, the value at 0x212;
  `KemKuldetesek[m].+4` = parameter 1's kind, `+8` parameter 2's; 0 → "BÁRMELY"), "Hátralévő idő:" and
  `(left + 49)/50 + " " + "nap"` (no countdown: none); other states: "Státusz:" + `KemStatusz_Nev[state]`.
- The planner (`SpyNewMissionMode`): "Küldetés:", the mission or "--- VÁLASSZ ---"; with a target: "Célbirodalom:",
  the empire or "--- VÁLASSZ ---"; the parameters (with an empire or for training); with a countdown "Küldetés
  ideje:" days + "nap" (or "---"), "Esély:" `E1·E2/100 + "%"` ("---" below 100; story missions "???"); "Küldetés
  ára:" "$" + cost ("---"); the buttons "MÉGSE" (0x1fb, 0x1b1) and, with an empire or no target needed, "OK" (or
  "!!PÉNZ!!" when the treasury is short) at (0x255, 0x1b1), centred (flags 7). No mission yet and the spy at home:
  "KIRÚGÁS" (dismiss) at 0x1fb; a spy on a mission: "VISSZAHÍV" (recall).
- Dialogs: 1 (a mission report): the outcome `KemBeszolas_Szoveg[b]` (b = 4 without success → [5]) in colour 6,
  "Küldetés:" + mission, "Birodalom:" + target, then the mission's result rows (2: planets, population, ships,
  techs; 3: the planets found or the planet's population, morale, tanks; 4: up to three techs; 5: the money; 6: the
  planet and the building; 7: the ship; 8: the tech; 9: the planet and what became of it; 10: the spy's level; 11:
  the family member; 12: the leader) or the reason (`x14` 999 nothing to do, 998/0x3e6 lost the duel, 0x3e5 the
  target is beaten); 7 (a double agent's report): two lines (2, 3..6) in 6 or 7, then as 1; 2 (an applicant):
  (3, 20..22) + `Spy_MennyiPenzbeKerul`; 3..6 (a caught enemy spy, its fate): two lines with the empire; 8..0x13
  (fates, rank-ups, double agents): (4, 2..23). Buttons: 2 → (4, 24) "Nincs szabad kémközpont" when the roster is
  full, else (4, 26) "Kém felvétele" or (4, 25) "Nincs elég pénzed", (4, 27), (4, 28) at x 0x226; 3 → (4, 29..32);
  others "OK" (4, 34) at 0x255 and "ÚJRA" (4, 33) at 0x1fb when the mission can be repeated (§6.2, 0x21).

### 5.2 Faders (`FadeKerettelKerekit`)
Buttons (0, 10, 30) at 100 with a (0, 10, 50) edge at 100: the dialog's (475, 298/332/366[/400], 150, 25), OK (565,
424, 60, 25), the left button (475, 424, 60, 25); the planner's rows (470, 269, 162, 15), (470, 303, 162, 15),
(522, 320, 110, 15), (522, 337, 110, 15) when they apply. The lists' boxes (0, 10, 0) at 60, edge (0, 5, 0) at 30,
in fader pass 2: mission `(x − 5, y − 2, 0x8e, 13n + 4)`, race `(x − 6, y − 2, 0x9a, 13n + 4)`, targets
`(x − 6, y − 2, 0x72·cols + 3, 13·rows + 4)`, kill `(x − 6, y − 2, 6·chars + 10, 3·13 + 4)`.

### 5.3 The lists (`SpyGfx2D_3`)
Rows 13 px apart from `y + 3`, colour 3, the row under the pointer 6, what the treasury can't pay 0xc (0x25 under the
pointer). Mission: missions 1..26 enabled (`KemKuldetesek[m].+0`) whose `+0x18` rank the spy has (22 cells).
Race: empires 1..8 alive, known (`BirodalomIsmeri`), not `EN` (24 cells; race 5 in colour 1; none: "Nincs ismert
faj!"). Targets (18 cells, up to 32 rows a column, 0x72 apart): "BÁRMELY" first, then 1 the empire's planets the
human sees (`+0xd8 + 2·EN > 0`), 2 techs 0x51..0x90 the human knows the empire has (seen matrix `+0xc0 + 9·EN + t`),
3 hulls 1..24 of class 1, not fighters, whose tech is seen, 4 techs 1..192 seen and not developed by the human,
5 the training levels 1..5 (no "BÁRMELY", 19 cells). Kill: `SpyAreYouSure_Nev[0]` then "Igen"/"Nem" (flags 5).

## 6. Input (`Spy_Main`)

### 6.1 Hot-spots (`Spy_MouseInit` positions, `Spy_MouseInit2` switches)
| Id | Rect (x, y, w, h) | On | Action |
|---|---|---|---|
| 3 | 475, 424, 60, 25 | planner without a mission, spy at home or travelling | the kill question (list, "Igen" → `0x47`) |
| 4 | 475, 424, 60, 25 | no planner, spy on a mission | recall `0x48` |
| 0xe | 565, 424, 60, 25 | a mission, an empire (or none needed), money ≥ cost | send `0x49` (spy, mission, empire, p1, p2, days) |
| 0xf | 475, 424, 60, 25 | a mission chosen | cancel: mission and empire cleared |
| 0x10 | 470, 269, 170, 16 | planner | the mission list |
| 0x11 | 470, 303, 170, 16 | the mission needs a target | the race list |
| 0x12 / 0x13 | 522, 320 / 337, 118, 16 | parameter 1 / 2 applies | the target list (`+4` kind; 0x13: techs) |
| 0x14..0x17 | 475, 298 + 34i, 150, 25 | dialog 3 | the verdict (§6.2) |
| 0x18 | 565, 424, 60, 25 | dialog not 2 or 3 | OK: close the report |
| 0x19..0x1c | 75, 322 + 16i, 96, 10 | skill points, state 1..3 | `0x4c` skill `i + 1` |
| 0x1e..0x20 | 475, 298 / 332 / 366, 150, 25 | dialog 2 (0x1e: money and a free slot, or SP forced 3) | `0x4b` hire / reject / later, close |
| 0x21 | 475, 424, 60, 25 | a repeatable report (§6.2) | repeat the mission `0x49`, close |
| 0x22..0x25 | 12, 320 + 16i, 86, 15 | a spy on show | tooltips only (the skills' descriptions) |
Ids 1, 2 and 5..0xd are set up but always off. The common ids (2..5: pause, speeds) are the top bar's (`hud.md`).
0x22..0x25 are on while the screen is open (`spyKINYITVA`) with a spy on show. Of several hot-spots under the
pointer the lowest id wins (`Mouse_Darabol` ELF 0x5b7a40 walks the table down; the pointer clamped to 639 × 479).

**Tooltips** (`Mouse_Szoveg_Set(id, 0, x, y, w, h, GetString(7, 1, k), …)`, `k` from the asm's immediates): 1..4 →
1..4, 0xe..0x18 → 5..15, the four skill buttons 0x19..0x1c all 16, 0x1e..0x25 → 17..24 (`MouseSzoveg` ELF
0x1652bb0, entries of 0x80 at `id + 0x1e`: rect, text at +0x10, the "act on release" flag +0x74, type +0x78 (≥ 0x40 =
off), colour +0x7c). `UpdateFrame` (ELF 0x578e40) shows the hot-spot under the pointer (`AktIkonKiir`) unless the
pointer moved within the last 400 ms (`LastMouseMove`, set by `Mouse_Lekerdez` ELF 0x5b7720 on a move of more than a
pixel; `AktIdo` = SDL ticks, ms) onto another hot-spot than the one shown, or a button is held (which restarts the
wait); once shown it follows the pointer (`Mouse_Kiiras_x/y` = the pointer, x pulled back so `x + len·6 + 0x18 ≤
640`). The box is `GFX_DateMoneyScrollFade`'s (ELF 0x66d160, fader pass 3): `FadeKerettelKerekit(x + 15, y − 1,
len·6 + 10, 13)` in (0, 10, 30) at 30 with a (0, 13, 31) edge at 15; the text `KiirStr(text, x + 21, y + 2, 99,
0x1a, 0xc, 2)` (size `0x1a % 8` = 2, flags 0xc: left, clipped) over everything. Port: `SpyScreen::tooltip`.

### 6.2 Branches
- **Lists** open on the press at the pointer (mission: `x = mx − 0x8c`, `y = my − 13n/2`; race `mx − 0x98`; targets
  `mx − 0x72·cols − 3`; kill `mx − 6·chars − 12, my − 13`), kept on the 640 × 480 screen; the row under the
  pointer follows while the button is held (`Mouse_Megfogva`); the release takes it. A mission costing more than the
  treasury is refused with `GetString(7, 2, 1)` ("Nincs elég pénzed.") on the message line (`ScrollHibaUzenet` ELF
  0x66f8c0: sound 6, `HibaUzenetSzoveg`, `messages.md` §2); choosing one sets the first parameter to 1 for training,
  else 0. A race (not race 5) clears a planet target. Training levels beyond the treasury are refused silently
  (`Spy_Main` has two `ScrollHibaUzenet` calls only: this one and the repeat's).
- **The verdict** (0x14..0x17, choice `c` 1..4 on the report's empire `x7` and spy `x8`): for 2 and 3, `rndlog(0, 1,
  loyalty + 3) < 6` and not a double agent → kind 8 / 9, else `rndlog(0, 1, 2)` 1 → fate 5 (escaped), kind 10,
  else fate 6 (killed escaping), kind 0xb; 1 → kind 0xd, 4 → kind 0xe. The report's kind becomes the follow-up
  (`KemBeszolasok[1].+0`, exe 0xBAF368; the dialog `SpyBeszolasMi` exe 0xE75384) and `0x4a` sends the fate (exe
  0x58d410 at 0x58e40b: spy.cpp lines 0x313/0x318; the block is entered for 0x14..0x18, the roll only below 0x18).
  The follow-up stays until its OK. **Verified**: 600 presses of 0x14..0x17 through the exe's own input on fuzzed
  prisoners (loyalty, double-agent field, stream 0's state) reproduced the fate, the follow-up kind and stream 0's
  state (`tests/spy_verdict_trace.rs`, `run_screen.py --spies verdict --verdict N`; §9).
- **Repeat** (0x21): dialog 1, outcome > 3, the report's spy on show and at home, the target alive, a planet target
  still the target's, not a tech already stolen, the mission enabled, not 999 or 0x3e5. Cost check (else
  `GetString(7, 2, 2)` "Nincs elég pénzed, hogy elküldd a kémed" on the message line; the report closes either
  way), then `0x49` with the report's mission, empire, parameters and the mission's days
  (training: `TrainingKuldetesIdo[level]`, exe 0x926EE4).
- After the branches: the cursor kept on a working spy; the planner's figures recomputed when asked
  (`SpyKellEselySzamolas`): days `KemKuldetesek[m].+0x10` (training: `TrainingKuldetesIdo[p1]`), cost
  `(int)(KemKuldetesPenzSzorzoFajonkent[race] · cost)` (training `TrainingKuldetesAr[p1]`), `E1 =
  KemEselySzamolas1(EN, AktKem, m, t, 1000, 0)`, `E2 = KemEselySzamolas2(EN, AktKem, m, t)`; without a mission or
  a needed empire all four are −1.

### 6.3 `KemEselySzamolas1(e, k, m, t, rounds, stream)` (exe 0x587b80)
Missions without a target (`+0x1c` clear): 100, no draws. Else `rounds` duels: the target's counter-spies (state 3 on
mission 13) among slots 1..`OsszHanyKem[t]` are drawn one by one (`RndInitValasztas`, then `RndValasztas(stream, 1)`
over the remaining ones; the last offered winner is taken), each adding `counter·10 / j` (j = 1.. the draw) to the
defence; then `a = rndlog(stream, 0, infiltration·140)`, `d = rndlog(stream, 0, defence·10)`; `a /= 3` if the spy is
known to `t`; race 3 and SP difficulty 1 for a human each `a = (int)((f64)a·1.5)`; the round is won when `d ≤ a` or
no counter-spy was drawn. Result `won·100/rounds` (0 rounds divides by zero in the exe), 0 for missions without a
countdown. **Verified**: 600 direct exe calls (fuzzed rosters, races, difficulty, known flags, streams 0..20)
reproduced bit for bit, result and stream state (`tests/spy_estimate_trace.rs`).

### 6.4 The carousel
`KozelitAktKemre`: when the selected spy's pedestal isn't settled (`RaalltKem`), the turn speed is set to
`∓|x − 320|/320·0.1` toward the middle; within 316..324 it is settled. The edge scrolling and the decay are §1.2.

### 6.5 Keys (`SpyKeyboardHandler`)
VK 0x27 (→): the previous working slot; VK 0x25 (←): the next one; only without a dialog.

## 7. Orders and multiplayer
Every change to the spies goes through `Multi_Send(type, EN, …)` (also in SP) to `MULTIGET_Spy` (`espionage.md` §8):
`ig2_net::Command::Spy { order }`. Three steps the original makes on the local machine are synced state or draw on a
synced stream, so the port sends them as orders too (ADR 0007 §5; `re/rules/multiplayer.md` §4–§5):
- `Judge { choice, owner, spy }`: the verdict's stream-0 roll, the report's follow-up kind and the fate (`0x4a`).
- `Acknowledge`: the report closed (the queue is per human, `multiplayer.md` §5.2).
- `Estimate { spy, mission, target }`: the estimate's stream-0 draws (2 per round + the counter-spy picks). The
  client computes the shown figure on a copy of the state's streams; the sim makes the same draws when the order
  arrives, before the next tick, as the exe's UI does between two ticks.
In the original's MP these UI draws moved each machine's own stream 0 (a desync source); in SP the port's order of
draws is the original's. The verdict roll is verified against the exe (§6.2).

## 8. Port notes and what is missing
The client opens the screen with S (`window.rs`) animated, and S or Escape closes it after the closing animation
(`SpyScreen::leave`/`closed`); `--snapshot --screen spy --ui-frames N` renders it after N UI frames (the carousel
settles, the names fade in), animated as the exe (the sphere's `MoveObj` applies at rest too); `--spy-frame F
[--spy-closing]` holds `spyframe` at F like the tracer holds the exe's; `--mouse X,Y` rests the pointer (a tooltip);
`IG2_LANG` picks the game's language (the captures here are Hungarian). The frame's poses are cached at the integer
frames 0..=50 and blended linearly in between (`Kocka` blends the keys, not the vertices: the same at the key
frames, off by the curvature of the nested turns between them). The refusals' lines go to `World::local_lines`.
Missing: the sounds, the 2D video path (`spyANIMBARKACS`: `PlayAnimFrame(0x100, 0x11c)`), the tutorial's limits
(`TutorialSpyMelyikLehet`), `DEBUGX`, the demo mode, the top bar's own tooltips (ids 1..0x1e). `spyFrameValtozo`
counts UI frames (the exe's drawn frames). The skill buttons 0x19..0x1c are off with a dialog up in the port;
`Spy_MouseInit2` leaves them on [not checked against the exe].

## 9. Captures and comparison
`tools/trace/run_screen.py --kep 7 --spies roster|report|verdict|applicant` stages five spies of different races and
states (at home with skill points, on a mission, travelling, captured, at home) and one report, waits 1200 frames
and writes the exe's save of the moment; `run_screendiff.py … spy:<mode>` compares (regions: the top bar,
the carousel, the two panels, the ring). `--spy-frame F [--spy-closing]` holds both at a frame of the opening or
closing (the tracer writes `spyframe` before every `Spying3DUpdate`; a held closing never ran a flip, so the exe
shows no disc), `--spy-mouse X,Y` rests the pointer for a tooltip. `run_screen.py --spies verdict --verdict N
--verdict-seed S --jsonl OUT` presses the verdict buttons through the exe's input (`AktIkonKell` exe 0xB77208 = id +
0x1e) on fuzzed prisoners, hooks `Spy_Main` and `Multi_Send` (exe 0x513840) and writes `spyverdict-*.jsonl` (the
prisoner is re-staged at `Spy_Main`'s entry: the last fate's `MULTIGET_Spy` may have rewritten it since).
Results: `diff-results.md`.
