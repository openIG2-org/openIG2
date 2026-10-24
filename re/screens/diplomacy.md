# The diplomacy screen (`AktKep` 6)

How the original draws the diplomacy screen and takes its input. Evidence: the ELF oracle and the exe decompiled with
`tools/ghidra/DecompileFunctions.java` (addresses ELF unless marked exe), the exe's static tables read from the
user's `ig2.exe` (`ig2-rules` `diplomacy_screen`), and captures of ig2.exe (`tools/trace/run_screen.py … --kep 6
--wait 1000`). The simulation behind it — messages, answers, deals, the contact list `AktPlayerek` — is specified and
verified in `re/rules/diplomacy.md` (§10 the human's side). `[inf]` marks inference, `[obs]` an observation on a
capture.

## 1. Functions
| Role | ELF | exe |
|---|---|---|
| `Diplomacy_Belep` (enter) | 0x529a50 | 0x4bf530 |
| `Diplomacy_Kilep` (leave: the closing animation, frees the texts) | 0x52a4d0 | 0x4bf810 |
| `Diplomacy_Main` (`MostHanyPlayerVan = MaxPlayerek`) | 0x52b160 | 0x4bfcb0 |
| `Diplomacy3DInit` (scene, lights, the first selection) | 0x5105d0 | 0x4bdb30 |
| `GFX_Diplomacy` (per frame: `DiplomGfx2D_2`, `Diplomacy3DUpdate`, then the menus' text pass) | 0x52a9d0 | 0x4c4c90 |
| `Diplomacy3DUpdate` (animation, faders, lights, background, `Update3D`) | 0x51b2f0 | 0x4be2d0 |
| `DiplomGfx2D_1` (2D pass 1: **all the input**, the menus' frames, the names under the heads) | 0x511190 | 0x4b89e0 |
| `DiplomGfx2D_2` (2D pass 2: the dialogue texts, head selection, the info panel) | 0x5198d0 | 0x4bc610 |
| `DiplomGfx2D_3` (empty; the exe registers `WaitForDrawing`) | 0x51aeb0 | — |
| `FRISSIT_Diplomacy2` (100 Hz UI tick: fades, the opening) | 0x52b1b0 | — |
| Menus: `FeltoltMenupontokPre` / `…ExchangeModePre` / `FeltoltMenupontok` / `…ExchangeMode` / `…Specials` | 0x522a50 / 0x5233c0 / 0x523ce0 / 0x527250 / 0x527800 | 0x4c41a0 / 0x4c3ee0 / 0x4c1e00 / 0x4c3ae0 / 0x4c4b90 |
| `KirakMenu` (draw + hover), `Menufolottvan` (hit test), `Igazitmenu` (place a sub-menu), `KirakFomenuAlpont`, `KirakEsInit`, `InitAlmenuk`, `DeinitMenuk` | 0x528040, 0x527ae0, 0x51cfd0, 0x529000, 0x528eb0, 0x5279f0, 0x51d3d0 | 0x4c67c0, 0x4c7ca0, 0x4c6360, 0x4c6680, 0x4c6620, 0x4c6500, — |
| `GetSzoveg` (dialogue text lookup, substitution, word wrap) | 0x521ab0 | 0x4c5890 |
| `CheckAndColorize` / `Colorize` (head pick, the leader's video) | 0x51b0e0 / 0x510500 | 0x4b8370 / 0x4b8530 |
| `ElkuldUzenet` / `ElkuldValasz` / `CancelLastMessage` (lockstep packets 0x3e / 0x3f / 0x40) | 0x51d580 / 0x51dc20 / 0x51d4a0 | — |

`Diplomacy_Belep` reads the three phrase files (`DiplomaciaFilenevek` exe 0x902FA8: `diplomac/ajanlat.eng` twice,
`diplomac/valaszok.eng`; `formats/text.md` §4) into buffers, sets the row fades `SzinallitoDesignTablazat[k] = −10k`
(exe 0xA4384C, 10 entries), builds the menus (`FeltoltMenupontokPre`, `…ExchangeModePre(1)`, `FeltoltMenupontok(0)`),
clears the selection state and opens with the animation (`dipKELLKINYITAS` = `KELLKINYITASDEFAULT` exe 0x912A39,
set in the GOG build) unless it comes back from the messages, help or options screens. Keyboard: none of its own
(the screen-select keys of every game screen).

## 2. Static layout (exe tables, `ig2-rules::DiplomacyScreen`)
- `GombPoziciok` exe 0x903418: 6 × `(x, y, z, 0)` f32, the six head slots in camera space (z = −650):
  ≈ (−198, −52), (−282, 4), (−198, 52), (200, 52), (200, −52), (284, −4): three on the left, three on the right.
  A head's screen centre is `(x + 340, y + 240)` (`CheckAndColorize`, −30 px for slots > 3).
- `DipFejX` exe 0x90335C: the name x per slot (135, 60, 135, 505, 505, 582); `ArcXek` exe 0x903478 (22, 100, 22,
  100, 470, 548; `GombokX`, unused on screen).
- `MenukKezdX/Y` exe 0x902FB8/BC (25, 371), `InfonevX/Y` exe 0x903000/04 (465, 40), `InfonevFont` 3,
  `InfoSzin1` 2, `InfoSzin2` 1 (exe 0x903374..7C), `PnevekFont` 2, `PnevekSzin2` 34 (exe 0x902F98/9C),
  `MenuMaxSorokO` 37 rows per column, `MenuOszlopKoz` 5 (exe 0x902F80/84).
- `TexturaNevek` exe 0x9033D8: 16 portrait textures `1a.bmp`, `1b.bmp` … `8b.bmp` (`textures/diplomac/`): race `r`
  uses `2(r − 1)` (`a`), or `2(r − 1) + 1` (`b`) when the slot's `+0x15` (the other side blocks calls, MP) is set.
- `KemFejNevek` exe 0x926FFC, indexed by race: the leaders' videos `anim/fejek/<name>` (`human.ani`, `ufo.ani`, …).
- Font metrics `CharXMeret`/`CharYMeret` (size 2: 6 × 7, size 3: 8 × 10; `re/screens/messages.md` §1).

## 3. The 3D frame (`Diplomacy3DInit`, `Diplomacy3DUpdate`)
Scene in `vector/diplomac/`, textures in `textures/diplomac/`, camera at the origin looking down −z, `xcenter` 320,
`ycenter = 250 − 10·dipframe/80` (240 when open), `fov2` as everywhere (600 [inf: same projection as the starmap
panels]); the viewport is the 4:3 one widened by `ModifyViewPort`.
- `dipbck8.xvb` ×1.1 at (0, `dipmozgato`, −650): child 0 = `DipWireGomb`, a wireframe grid (property 0x40) that is
  the faint background grid; child 1 = the opening ring (sub-meshes 0..3 and 4..7 hidden from `dipframe` 60 on by
  `DiplomaciaInitGyuruk`, 8 and 9 always hidden). `SetFrame(dipframe)` animates the opening (frames 0..80).
- Six `golya.xvb` ×0.04 at `GombPoziciok[k]`, random start orientation (3 libc `rand() % 314 / 20` each,
  cosmetic), wireframe (0x40 | 0x800, not 8): the head cages. Per frame `RotateObjr(+0.02, +0.02, +0.02)` while the
  slot holds a living empire (or the screen is still opening), else `(−0.005, 0, 0)`. Transparency starts at 1 when
  opening (0 otherwise) and falls 0.02 a frame to 0.5. During the opening each cage swings in from
  `(x + 820·sin(3s + k/2)·s, y + 500·cos(4s + k/2)·s)`, `s = 1 − dipframe/80`.
- Six `negyzetN.xvb` ×0.04 at the same places: the portrait square (sub-mesh 0 takes the slot's portrait texture,
  §2), shown for slots `< MaxPlayerek` (`DiplomGfx2D_2`), transparency `fejetransp` (1 → 0 by 0.03 a frame once
  open).
- `tube.xvb` ×0.148 (`DipKozep`, the round frame) and `kor.xvb` ×0.105 (`dipKorId`, the screen inside it, its own
  white ambient light) at (0, 0, −650), hidden until the ring opens. From `dipframe` 60 the frame turns about y by
  `Atfordulas·π/30` while `Atfordulas` runs 0..30 (0.2 per UI tick; a new selection restarts it at 0 with
  `screen21.wav`), comes forward `30·Atfordulas` (`30·(29 − Atfordulas)` from 16 on) and, past 14, starts the leader's
  video on `kor`'s texture (`InitAnim(AnimNeve, loop, 25 fps)`, `FrissitDiplomaciaAnim` → `PlayAnimVector` each
  frame; `formats/ani.md`). At 30 it flips a half turn (`dipAtvaneFordulva = π`) and rests; the inner ring turns
  `dipFrameValtozo/220` and `−dipFrameValtozo/90` (one step per frame).
- Lights: ambient 0x141414, parallel 0x808080 from (0, 0, −10), two coloured parallel side lights (0x7F5D00 and
  0x004E7F, `0xBBGGRR`: blue and orange) circling with `dipkeringeto` (+0.0006 per UI tick:
  `(300·sin 20k, 200·cos 20k, −350)` and a quarter turn on). Per head (`DipRarakFenyekFejekre`): the selected one a
  white light pulsing 160..224, the others a tint `(100 − h, 100 − h/2, 100 + h)`; ambient marks
  (`0xBBGGRR`): `JelolFej` 0x860E0E (blue: selected), `JelolFej2` 0x0E0E86 (red: the slot isn't open, or blinking
  while the human's message awaits its answer), `JelolFej3` 0x0E860E (green, blinking: a message of that empire
  waits for the human). All lit by the engine's model (research.md §2.1; the cages' wires per vertex, `wirekirak`).
- Background: `Copy3hatterkep(0x34, grey 255 − 128·dipframe/80)`: picture 52 (`_ESIG_U2.BMP`) stretched over the
  viewport, half bright once open. The picture is the saved starmap, see [`overlay-background.md`](overlay-background.md).
- Faders (`FadeKerettelKerekit`, fill (0, 54, 90), edge (0, 90, 130); `f = 2·dipframe/80 − 1`, 1 when open): the
  info panel `(InfonevX − 10, InfonevY + 40 − 50f, 172, 100f)` at transparency `255 − infotransp` / `255 −
  2·infotransp`; the human's own text `(160, szovegy − 9, 465, (14·n + 10)·f)` with `mymstransp`; the other side's
  text `(MenukKezdX − 23, InfonevY + 15 − 25f, 440, (14·n + 10)·f)` with `thmstransp` (n = the text's lines,
  `szovegmagassag`, shared by both).
- Wireframes (`wirekirak` 0x6083f0): each face's edges by its edge mask (`formats/xv.md`), as textured lines with
  the vertex's lit colour.

### 3.1 The opening and the closing (`Diplomacy_Belep`, `Diplomacy3DUpdate`, `Diplomacy_Kilep`)
Flags (exe): `dipframe` f32 0xAB768C, `dipKINYITVA` (open) 0xAB6504, `dipBECSUKAS` (closing) 0xAB6477, `dipELINDULT`
(the first 3D update ran), `dipKELLKINYITAS` (animate). `Diplomacy_Belep` (ELF 0x529a50): `dipKELLKINYITAS =
KELLKINYITASDEFAULT` unless the previous screen was messages (10), help (1) or options (0xd) — then 0, the screen
starts at rest and silent; `dipKINYITVA = !dipKELLKINYITAS`; animating: `dipmozgato` 29, `fejetransp` 1 and
`PlaySound(Dip_Hangok[0])` (`screen13.wav`), else 0 and 0; `Atfordulas` 30, then `Diplomacy3DInit` sets `dipframe` 0 (80 at rest) and
`Atfordulas` 0 (the frame's first turn runs in both cases), each cage's transparency `+0x48` 1 (0 at rest).
Per drawn frame, `Diplomacy3DUpdate` (ELF 0x51b2f0, exe 0x4be2d0), with `s = 1 − dipframe/80`:
- `dipKINYITVA` set once `dipframe ≥ 80` (and the game clock runs again: `IdoPauseMertAtlepesAlattVan`, set while
  the screen opens or closes).
- `dipbck8`: `SetFrame(main, dipframe)` (its own animation, frames 0..80); while opening or closing the whole mesh
  at y `(int)(29·s)`. Child 1 (the opening ring): sub-meshes 0..3 shown only for 40 < `dipframe` < 80, 4..7 shown
  until `DiplomaciaInitGyuruk` (`dipframe ≥ 60`, not closing) hides 0..7 and shows the round frame `DipKozep`; 8
  and 9 always hidden. Child 0 (the grid `DipWireGomb`) at `(0, 200s, −1000s)` relative (`dipInitPos` 0xAB767C..84
  is never written: zero [obs: the sequence matches at t 150]), turned `(3 sin 2s, 2 sin 2.1s, 0)`, frame `dipframe + dipwireframe` (at 80 the
  `dipwireframe` cycles 0..10 by 0.25 a frame), transparency `1.35 − 2·dipframe/80` up to `dipframe` 40, then 0.
- The cages (`Fejek[k]`) at `GombPoziciok[k] + (820 sin(3s + ⌊k/2⌋)·s, 500 cos(4s + ⌊k/2⌋)·s, 0)` (sin/cos in f64),
  spinning `+0.02` on each axis also while not open; transparency −0.02 a frame while above 0.5 (exe 0x4be2d0 lines
  79-90): after an opening the cages **stay about half transparent** (26 steps from 1 end at 0.48000047), at rest (no
  opening) opaque. Without property 8 the engine draws them additively, the lit colour scaled by `≈ 1 − t`; the
  client's wire alpha `2(1 − t)` carries its doubled wire light (the factor is calibrated, not derived).
- The portraits (`Fejek[k]+0x10`): `fejetransp` −0.03 a frame once open (+0.09 while closing); they swing with the
  cages only while closing (they stand at their places, invisible, during the opening).
- The round frame: from `dipframe` 60 as in §3 (the turn, `dipFrameValtozo`'s ring spins: sub-mesh 0 `Rz(n/220)`
  then `Ry(dipAtvaneFordulva)`, sub-mesh 1 `Rz(−n/90)` then `Rx(dipAtvaneFordulva)`, `kor` `Ry`; while turning both
  axes get a further `Atfordulas·π/30` — `RotateObj2` composes, so the flip angle runs on from the last rest:
  `FRISSIT_Diplomacy2` toggles `dipAtvaneFordulva` 0 ↔ π each time `Atfordulas` reaches 30).
- `ycenter = 250 − 10·dipframe/80` (240 at rest), background grey `255 − (int)(128·dipframe/80)`; the three faders
  of §3 drawn only above `dipframe` 40, `f = 2·dipframe/80 − 1`.
- Closing (`dipBECSUKAS`): `kor` transparency `s`, z `−650 + 500s`, turning `Rx +0.2` a frame.

The 2D passes are gated on `dipKINYITVA && !dipBECSUKAS` (exe `DiplomGfx2D_1` 0x4b89e0 returns early when not open):
while the screen opens or closes there are no menus, names, texts, info panel or input — only the 3D frame and the
three faders' frames. The row fades (`FRISSIT_Diplomacy2`) start once open.

`Diplomacy_Kilep` (ELF 0x52a4d0, exe 0x4bf810): towards any screen but messages, help and options, when animating:
`dipBECSUKAS`, child 1's sub-meshes 0..7 shown again, `DipKozep` hidden, the ring's rotation cleared,
`PlaySound("screens/diplomac/screen20.wav")` (= `Dip_Hangok[1]`), then it draws frames (`dipframe −0.6` a UI tick)
until a frame has been drawn at 0 (`voltnulla`). The order (`IG2_gameUpdate` exe 0x4e0440): the key sets `AktKepKell`
inside a tick; the frame after it is still open; then `Diplomacy_Kilep` draws its first frame at 80 at once (no tick
between) and steps: the captures after t + 1 ticks show the open screen at 80, after t + 2 ticks 79.4 [obs:
diplomacy-fx9.json, F2 at 400: 80 / 79.4 / 78.8 at 401 / 402 / 403, the sound at 400, the client the same]. Towards messages, help or options: no closing, no sound (the screen
is drawn once more for `SaveScreen(2)`).

## 4. `FRISSIT_Diplomacy2` (every 10 ms UI tick on screen 6)
`dipkeringeto += 0.0006`; opening: `dipframe += 0.2857` up to 80 (≈ 2.8 s), closing −0.6; from 60, `Atfordulas`
+0.2 until it reaches 30. Once open, every row fade `SzinallitoDesignTablazat[k]` +1 up to 100 (all 100 at once
without the opening), `infotransp` +1.6 to 127 while an empire is selected, `mymstransp` +16 (to 127) while the
human's own text is up else −1.6, `thmstransp` ±1.6 for the other side's text. `KiirStrSzinSzorzo` (the text
brightness, %) = the row fade for the menus' rows and the info panel's lines, `mymstransp/1.27` /
`thmstransp/1.27` for the texts.

## 5. Heads, names and the info panel (`DiplomGfx2D_1` tail, `DiplomGfx2D_2`)
For the contact-list slots `k < MaxPlayerek` (`AktPlayerek`, at most six drawn):
- A left press within 40 px of a head of a living empire selects it (`CheckAndColorize`; `PlaySoundGlobal(0)`):
  `PlayerSelected = k`, `AktPlayerek[k].+0x19 = 0`, the composing state cleared (`Keszazajanlat`, `Almenuksel`,
  `ActFomenu`, `Exchangemode`), the frame turns again (`Atfordulas` 0) and `AnimNeve = anim/fejek/` +
  `KemFejNevek[race]`. On entry the first living slot is selected (`Diplomacy3DInit`).
- Name `Birodalom_Nev[e]` at `(DipFejX[k], y + 275)` (y = `GombPoziciok[k].y + 240`), size 2, centred, colour 34,
  brightness the row fade k; a dead empire also gets `Beszolasok[6]` ("(dead)") 16 px lower, colour 6.
- Info panel of the selected empire (brightness by line, fades 0..5): its name at `(InfonevX − 3, InfonevY)` size 3
  colour 2; `GetString(6, 10, 9) + ":"` (known planets) at +20, colour 1, and the count of planets the empire owns
  that the human sees (`+0xd8 + 2·EN > 0`) at `(InfonevX + 94, +20)`, 5 cells, colour 2; `GetString(6,10,10) + ":"`
  (contract) at +40 with `GetString(6, 10, 11 | 12)` (trade agreement | none, `Viszony[EN][e].+0x24`) at
  `(InfonevX + 57, +40)`; `GetString(6, 10, 13) + ":"` (status) at +50 with `StatuszNevek[level]` (`GetString(6, 5,
  level + 1)`, `Viszony[EN][e].+0x25`). With `DEBUGX` a power line at +70.

## 6. Menus
`Menuk` exe 0xA43878, records of 0x1BA8: 0x40-byte entries (text, `+0x34` value, `+0x38` sub-menu or −1), then
`+0x1B80` count, `+0x1B84` hover colour (34), `+0x1B88` colour (2), `+0x1B8C` font, `+0x1B90/94` x/y, `+0x1B98`
hovered entry, `+0x1B9C` chosen entry, `+0x1BA0` row gap, `+0x1BA4` open. Rows are `CharYMeret[font] + gap` high,
columns `CharXMeret[font]·longest + 5` wide (37 rows a column); a menu that would end below y 449 moves up to end
there. `KirakMenu` runs twice per frame: pass 1 (`DiplomGfx2D_1`) updates the hover from the mouse (inside
`x − 4 .. x − 4 + width`, the row ± gap/2) and draws the frame `FadeKerettelKerekit(x − 4, y − 4, width + 7,
rows·(h + gap) + 5, fill (0, 54, 90) t 100, edge (0, 90, 130) t 30)`; pass 2 (`GFX_Diplomacy`) draws the texts
(colour 34 hovered, 2 otherwise, 1 when the entry's sub-menu is one of the item lists and empty).

The menus (`FeltoltMenupontok`, rebuilt every frame while open; `level` = `Viszony[EN][e].+0x25`):
- 0 (main, at (25, 371) size 3 gap 6): `GetString(6, 6, 1..6)` = negotiate → 0x29, talk about → 0x13, offer for →
  1, demand → 0x15, offer → 1, special → 0x32 (the empire's `special.eng` entries).
- 0x29 negotiate, by level (`GetString(6, 8, i)` → message sub-code): 1 war: cease-fire (0), peace (1); 2
  cease-fire: peace (1); 3 neutral: declare war (2), NAP (3), war together (7 → 0x3f); 4 NAP: declare war (2),
  break NAP (4), offer alliance (5), war together (7), trade agreement (8, left out when one exists); 5 alliance:
  declare war (2), break alliance (6), war together (7), trade (8, idem).
- 0x13 talk about, by level (`GetString(6, 9, i)` → sub): 1: peace negotiations (0), threaten (1), insult (4);
  2: (0), (1), spies (3), lie about → 0x31 (6), insult (4); 3: improve (2), spies (3), lie → 0x31 (6), insult (4),
  threaten (1); 4: improve (2), spies (3), lie (6), future alliance (9); 5: improve (2), spies (3), attack warning (6
  → 0x31), birthday (8, only on the emperor's anniversary month and once a year).
- 1 offer / offer-for items (`GetString(6, 10, 1..4)`, 15 entries): money → 2 (10 000 … 400 000 + " credits"),
  hostage → 3 (only when the selected empire holds the human's family member), planets → 4 (the human's), info about
  an empire → 0x3c, then the 12 tech areas (`Terulet_Nev`, `GetString(4, 3, 1..12)`) → 5..16 (the human's developed
  techs the empire hasn't seen).
- 0x14/0x15 the exchange (the "for:" side, size 2): money → 0x16, hostage → 0x17, the empire's visible planets →
  0x18, info → 0x3d, tech areas → 0x19..0x24 (its techs the human has seen and lacks); 0x14 = exchange for /
  cancel exchange.
- Empire lists 0x31 (talk about), 0x3c..0x41, 0x33, 0x34 (war on / peace with): living contacts other than the
  selected one, filtered by the levels named in the decompile.
- 0x28 send now / cancel, 0x2a we agree / forget it (to a counter-answer), 0x2b agree / forget it / okay but …
  (to an AI offer), 0x2f OK (acknowledge), 0x2e cancel last message (MP), 0x30 OK when the AI's terms no longer
  hold (`AjanlatFeltetelekOK`).

### 6.1 The special (story) menu 0x32 (`FeltoltMenupontokSpecials` ELF 0x527800, exe 0x4c4b90)
`SpecialLista` (exe 0xA42DE8, saved, `SpecialListaMut` 0xA41DC4 entries, 12 bytes: `+0` special id (0 = free),
`+4` empire, `+8` index into `SpecialLeiro` (`special.eng`, `formats/text.md` §4)) is written by the story:
`Diplomacy_AddSpecial(id, e)` (ELF 0x52be90: only an id `special.eng` has; first free slot, else appended),
`Diplomacy_RemoveSpecial(id, e)` (ELF 0x52bff0: clears the matching entry, trims free entries off the end),
`Diplomacy_ClearSpecials` (0x52be70, new game). Every frame (`DiplomGfx2D_1`, after the open gate) menu 0x32 gets
the selected empire's entries in list order: text = the special's title (`strncpy` 20 bytes), `+0x34` = the
`SpecialLeiro` index, no sub-menu, and the `+0x3c` "disabled" byte for id 0x217B (8571) while `Money[EN] <
2 000 000` (f64, strict). Main entry 5 links to it; with no entries it is drawn in colour 1 like the empty item
lists (`KirakMenu`), and a disabled entry is drawn in colour 1 and can't be hovered or picked.
Picking an entry (release, `ActFomenuSzam` 5) composes two lines without `GetSzoveg`: `KeszAjanlat` = the special's
text A, line 2 = text B (no substitution, through `a2az`), with menu 0x28; "send now" sends cat 4, `sub` = the index,
everything else 0, the reason `MiertValaszoltAztAmitValaszolt(EN, e, 2, 3)`. The AI hands it to the story
(`Story_Diplomacy_Special(SpecialLeiro[sub].id, b)`, `rules/diplomacy.md` §6.2) and answers no; the answer shows
the special's option `value` and its result (`options[value]`, `results[value]`; value is 0 as sent) as two lines
with menu 0x2f. A waiting cat-4 message to the human shows nothing (only humans send specials).

## 7. Input and the dialogue (`DiplomGfx2D_1`)
With an empire selected (`PlayerSelected = k`, slot `s = AktPlayerek[k]`, `e = s.empire`):
1. **Composing** (no message waiting `+0x17`, no blocked `+0x15`, nothing asked `+0x16`, the last message older than
   250 `Ido` (`+0x10`; 50 in the tutorial), no answer `+0x18` and no counter-answer `+0x2c` pending): main menu 0;
   pressing an entry opens its sub-menu (`InitAlmenuk`), releasing on a leaf of the chosen branch builds the
   proposal text (`GetSzoveg(0, KeszAjanlat, cat, sub, reason, …)`, `#1#` = the empire's race name, `#2#` = the
   item; the reason code from `MiertValaszoltAztAmitValaszolt(EN, e, 2, 0|3)`) and shows it at (172, 395) with menu
   0x28. An "offer for" (cat 1) first switches to the exchange side (`Exchangemode`, menus 0x14/0x15).
2. **Send now** (0x28 entry 0): `s.+0x0c = 1`, `s.+0x16 = 1`, `s.+0x10 = Ido`, then `ElkuldUzenet(EN, e, decl,
   cat, sub, value, sub2, value2, extra, reason)` → packet 0x3e (`re/rules/diplomacy.md` §10.2): negotiate = cat 0
   (decl set), talk = cat 6, offer for = cat 1 (`Predata` from the two menus), demand = cat 2, offer = cat 3,
   special = cat 4. Cancel (entry 1) drops the proposal.
3. **A message waits** (`+0x17`, not yet answered `+0x19`): its text (`GetSzoveg(1, dipUzenetSzoveg, cat, sub,
   reason, …, variant +0x60)` at (10, 40 + 14i), the other side's fader) and the answer menu by kind: 0x2a for
   negotiate/talk (0x2f for declarations and pure talk), 0x2b for deals. Agree → `+0x17 = 0`, `ElkuldValasz(EN, e,
   counter 0, yes 1, …)`; forget it → yes 0; "okay, but …" (0x2b entry 2) opens the exchange menus to name another
   item (`+0x19`), then sends a counter-proposal (`counter` set, reason 0x29a). When the terms no longer hold, only
   0x30 (OK) is offered and it answers no.
4. **An answer arrived** (`+0x18`): `GetSzoveg(2, ValaszSzoveg, …, variant +0x8c)` with menu 0x2f (OK →
   `+0x18 = 0`). A counter-answer (`+0x2c`) shows `GetSzoveg(3, …)` with 0x2a: agree or forget it → `ElkuldValasz(…,
   reply)`.
5. MP only: 0x2e "cancel last message" while `+0x16` (`CancelLastMessage`, `+0x16 = +0x0c = 0`).

`GetSzoveg(file, out, a, b, c, d, p1, p2, p3, fallback, seed)`: the header `#aaa/bbb[/ccc[/ddd]]` (−1 parts left
out) is counted in the file's buffer and variant `random(n)` picked — or `n·seed/10001` with a seed (the waiting
message's `+0x60`, the answer's `+0x8c`); with `fallback` and no match, `Accc/ddd`; nothing at all gives one of two
"radio engineers" excuses. The variant's `- ` lines are joined with spaces, `#1#`/`#2#`/`#3#` replaced, and the text
wrapped at 70 columns into at most five lines of 100 bytes. Unseeded `random()` is libc (`re/rules/diplomacy.md` §9).

### 7.1 Sounds (`re/screens/audio.md` §6.5)
`PlaySoundGlobal(0)` on every press of a menu entry or a head (inside the open gate). `Dip_Hangok[0]` on entering
with the opening only, `Dip_Hangok[1]` (`screen20.wav`) when the closing starts (none when leaving for messages,
help or options), `screen21.wav` (`Dip_Hangok[2]`) when another head is picked while the frame is at rest.

## 8. The port (`ig2-client::diplomacy`)
Drawn from the sim's state each frame; the dialogue state of the screen (selection, menus, the composed text) is the
client's own, like the original's globals. Deliberate differences, all for multiplayer (ADR 0003, ADR 0007):
- The screen's writes to the contact list (`+0x0c`, `+0x10`, `+0x16`, `+0x17`, `+0x18`, `+0x2c`) and to
  `VoltmarUdvozloUzenetEbbenAzEvben` (the birthday greeting) go through the lockstep queue as `Command::DiplomacyScreen`, applied just before the message or answer they belong to (the
  original writes them in the frame it sends the packet, one frame earlier).
- The unseeded text variants use the client's own UI generator, not the sim's libc stream.
- `+0x19`/`+0x1c`/`+0x20` (the counter-proposal being composed) live in the client, not in the save.
- The menus are press-and-hold cascades like the original's (`Mouse_Bal` held over a main entry opens its sub-menu,
  a release on a leaf picks it); the client recomputes them every frame from the state (`FeltoltMenupontok`).

Observations from the captures that shaped the port [obs]: the panels' fills are textured by `fadertexture`
(`textures/design/screen/dt.bmp`, mean grey 0.5); the background grid is lit by the ambient light only (a uniform
`racsszin` × 0x14); the cages' wire colours reach white (the port doubles the light on wires); the video is
interlaced (every other row dark, `DrawAnimInterlace`); the cages hide their back lines (the port culls back faces);
picture 52 (`_ESIG_U2.BMP`) isn't in the GOG install, so `Copy3hatterkep` draws nothing after a bare jump (a screen entered from the starmap has the `SaveScreen` picture: [`overlay-background.md`](overlay-background.md)). Results:
`re/screens/diff-results.md` (2026-10-17).

Verified in tests (`ig2-client::diplomacy`, quick-1.sav): composing "negotiate → cease-fire", sending it, the AI's
immediate answer and its acknowledgement; an AI's cease-fire offer shown with "we agree / forget it" and accepted
(relation level 2 afterwards); `GetSzoveg`'s lookup, seeded variants, substitution and wrap; `a2az`; the menu
layout (`Igazitmenu`, the bottom clamp, hit tests).

The opening, the closing, the special menu and the sounds (2026-10-23, §3.1, §6.1, §7.1): `Screen::enter`/`leave`/
`frame3d` (`Diplomacy_Belep`/`_Kilep`/`Diplomacy3DUpdate`'s per-frame steps, counted at 60 a second in the window, one
a tick under an input script), `dipbck8` posed between its keys (`scene::frame_keys`, as the design and trade
screens), F2 leaves through the closing (`DoKeyboard`: the exe has no D key; the client's own D does the same), the
messages screen in and out without the animation and silent. The special menu reads `SpecialLista` from the state;
the sim doesn't write it yet (`Diplomacy_AddSpecial`/`RemoveSpecial` from the story's events, and the AI's
`Story_Diplomacy_Special` hook, are unrouted: a save's list shows, a running campaign adds none). Verified:
`run_screendiff.py … diplomacy --anim-frame 20|50|70` (`--dip-frame` holds the exe's `dipframe`): every region
matches at 20 and 50 but `info` at 50 (the cages' wires), 70 as at rest; the sequence `scripts/diplomacy.json`
(opening, a menu press, F2): the exe's `dipframe` equals the client's at every grab of the opening (17.14, 42.86,
65.71, 80), t 60–230 match (t 150 frame 2.6), the closing runs one tick behind in the exe (F2 takes effect through
`AktKepKell` a frame later); the sounds are the exe's: `Dip_Hangok[0]` (`screen13.wav`) on entry, the click at the
press, `Dip_Hangok[1]` at F2. [obs] `kor`'s closing turn isn't mirrored: `RotateObjr` rebuilds the matrix from the
object's angles and drops the `RotateObj2` flip.

Not ported yet: the MP-only menus 0x2e, the leader video's lip sync, `DEBUGX`, the demo mode, `dipbck8`'s child 2
(`Kezelo_radar`, six small spheres; not seen on the captures); the reason codes of the AI's own messages
(`MiertValaszolt…` for the receiving side) are read from the slot as the sim stored them.
