# SpaceWar screen (`AktKep` 11): the real-time space battle

The battle itself (tick, setup, end, the orders' messages) is `ig2-sim::spacebattle`, spec `re/rules/space-combat.md` §7.

Sources: the ELF decompiles `_local/decomp/spacewar-screen-elf{,2,3}/` (unstripped -O0, the names), the exe's
`_local/decomp/spacewar-screen-exe/` (`GFX_SpaceWar`, `initgfx` 0x575050, `screenfrissites` 0x57f800, the screen's
entry points) and `objdump -d` of ig2.exe for the rest; checked against the asm at the spots marked "checked" and
against runtime captures of ig2.exe ([obs]). ELF and exe differ in layout: the `SpwC` slot stride is 0x3450 in the
ELF, 0x2D9C in the exe (unit 0x290 / 600, gun 0xb0 / 0xa8, fighter group 0xb8 / 0xa8, shot 0xd8 / 0xa8); offsets are
ELF unless marked "exe". World points are (h, x, y), height first (as on the starmap); constants f32 unless marked f64.
Tags: [inf] inferred, [obs] observed in a capture, [u] unverified, (inf) for an exe address found by call order.

Where the drafts disagree, §8.1 (from the overlay pass, which read more of the asm) corrects §1–§3.

Outline: §0 functions; §1 scene set-up; §2 camera; §3 per-frame order; §4 entering, leaving and the hot-spots; §5 the 2D
HUD (weapon column, info boxes, radar, retreat window); §6 overlays and effects; §7 input, timers and sounds; §8
corrections, exe vs ELF, checks and unknowns; §9 client status.

## 0. Functions

| Function | ELF | exe |
|---|---|---|
| `GFX_SpaceWar` (runs `screenfrissites` of all 10 slots) | 0x67c710 | 0x55e850 |
| `SpwC::screenfrissites` | 0x6a7780 | 0x57f800 |
| `SpwC::initgfx` | 0x6923a0 | 0x575050 |
| `loadurhajogfx` / `LoadUrhajoMesh` | 0x689710 / 0x6899d0 | 0x578b20 / inlined |
| `initracs` (grid lines) | 0x694f90 | 0x576380 (inf) |
| `SpaceWarhajovonalkirajzol` (drop lines) | 0x68ecb0 | 0x570960 (inf) |
| `SpaceVadaszKirajzol` (fighter sprites) | 0x694e50 | 0x56d460 (inf) |
| `frissit2` | 0x694c50 | 0x573b40 |
| `unloadgfx` | 0x691f70 | not found (0x582280 is a candidate, called from `SpaceWar_Kilep` 0x562b50) |
| `kamerabeallit` | 0x6a2cf0 | 0x576750 (inf) |
| `NezetVisszaAllitas` | 0x6a8460 | 0x56c420 (inf) |
| `NezetVisszaAllitSet` / `Set2` | 0x6aa030 / 0x6b2b00 | 0x56c120 (inf) / not found |
| `rescalescene` | 0x6b1e60 | not found |
| `sceneeltol` | 0x6944c0 | 0x57f5b0 (inf) |
| `kijelolesanim` | 0x6a8b20 | 0x576fc0 (inf) |
| `magassagkirak` | 0x6a9120 | 0x57b600 (inf) |
| `starmapszarok` | 0x6a64c0 | 0x581bc0 (inf) |
| `pulzaljspacewarcsillag` | 0x6815d0 | 0x57e420 (inf) |
| `felrobban` | 0x69b610 | 0x5737e0 |
| `urhajomeghalas` (explosion visuals) | 0x6a1690 | 0x582550 (space-combat.md) |
| `Update3D` | 0x605940 | 0x52cd10 |
| `savestarmapparticles` | 0x694210 | 0x57f550 (inf) |
| `bezoomolasztav` / `kijelolescenter` | 0x6a9ed0 / 0x6b1a40 | 0x571c00 (inf) / 0x5772c0 |
| `SpaceWarFrissit2` / `FRISSIT_SpaceWar2` | 0x6a3380 / 0x67c450 | 0x56d870 / 0x55e6e0 |
| `renderscene`, `CameraRotate` (look-at), `CameraRotate2` (yaw/tilt) | 0x604a80, 0x5fcf90, 0x5fd340 | not looked up |
| `gorbe` / `gorbe2` | 0x6b8a10 / 0x6b8ad0 | not found |

3D engine calls, ELF -> exe: AddToScene 0x4551d0 -> 0x451720; AddNullBox 0x455520 -> 0x4514b0; LoadBox 0x454770 ->
0x452520; MoveObj 0x455f80 -> 0x452600; RotateObj 0x455a60 -> 0x452960; ScaleObja 0x457140 -> 0x452e30; HideObj
0x456170 -> 0x4524c0; UnhideObj 0x4561c0 -> 0x4534b0; SetObjTransparency 0x456d10 -> 0x4533a0; SetObjShade 0x456c70 ->
0x453330; PropertiesOn/Off 0x456a50/0x456ab0 -> 0x452750/0x452710; SetRenderOther 0x456e60 -> 0x453470; SetParticle
0x456430 -> 0x4533f0; SetBoxLine 0x456490 -> 0x452ef0; SetRelative 0x4565d0 -> 0x453430; SetFrame 0x4564f0 -> 0x452f30;
CreateLightAmbient 0x4575c0 -> 0x451cb0; GetSubMeshid 0x456630 -> 0x452410; GetMaterialp 0x456960 -> 0x452240 (inf);
GetMaterial 0x456880 -> 0x4521c0 (inf); SetMaterial 0x456820 -> 0x453280; CreateXVB 0x4574b0 -> 0x4520f0; GetObjPos
0x456040 -> 0x452320; CreateMatrix 0x451ee0 -> 0x44f170 (inf); loadtexture 0x5ff310 -> 0x534720; loadanimtexture
0x5ff6f0 -> 0x534460; ModifyViewPort 0x4f6df0 -> 0x4ac4f0; random 0x4f0200 -> 0x819978 (the exe's `initgfx` calls
the CRT `rand()` and takes `% n` inline).

What the calls do (ELF bodies):
- `AddToScene(xvb, parent, size, &meshid)`: the object's bounding box is normalised so its longest side is `size`
  (times the xvb's +0x4c factor unless the xvb's flag bit 0 is set); `meshid` = (parent xvb, node index).
- `AddNullBox(parent, &meshid)`: an empty node (carries particles, lines or a render callback).
- `LoadBox(name, bool)`: file from the global `VectorDir`, textures from `TextureDir` (both strcpy'd just before);
  cached by name; `nemkellboxszamolas = 1` before it skips the bounding-box work. Missing file = message box.
- `SetObjShade(false)` = `PropertiesOff(1)` (property 1 = lit). `SetRelative(false)` = camera-fixed node (position is
  in camera space). `SetFrame(f)` stores `(int)(floor(f)*256 + frac(f)*256)`. `SetParticle(node, PARTICLESTYPE*, n)` =
  n particle arrays on the node (0x4c-byte particles). `SetBoxLine(node, VONALTYPE*, n)` = n 0x20-byte lines drawn
  with the node. `SetRenderOther(node, fn, 0)` = callback when the node is rendered.
  `CreateLightAmbient(colour, node, &lightid)`.

### 0.1 The HUD's and input's functions and the slot's fields (ELF → exe)

| Function | ELF | exe |
|---|---|---|
| SpaceWar_Belep | 0x67bad0 | 0x562680 |
| SpaceWar_Kilep | 0x67c090 | 0x562b50 |
| SpaceWar_MouseInit (MouseInit2 is empty, ELF 0x67d880) | 0x67c7d0 | 0x562d10 |
| SpaceWar_Main (10 slots x SpwC::main) | 0x67c2c0 | 0x562cb0 |
| SpwC::main (the input handler) | 0x6aed10 | 0x57b9e0 |
| SpaceWarKeyboardHandler | 0x67e190 | 0x561ef0 |
| FRISSIT_SpaceWar2 (every 100 Hz UI tick, §7.2) | 0x67c450 | 0x55e6e0 |
| SpaceWar_Autoscroll (every UI tick, §2.6) | 0x67dac0 | 0x5624c0 |
| SpwC::SpaceWarFrissit2 (per-tick HUD state) | 0x6a3380 | 0x56d870 |
| SpwC::SpaceWarMouse | 0x6a9490 | 0x56f420 |
| SpwC::SpaceWarMouseMove (picking, box) | 0x6b2c90 | 0x56fa60 |
| SpwC::BalClick (left release) | 0x6b13c0 | 0x566810 |
| SpwC::JobbClick (right release) | 0x6b0210 | 0x569190 |
| SpwC::magassagbeallitas (height mode start) | 0x6b3d20 | 0x57b510 [g] |
| SpwC::magassagkirak (height marker) | 0x6a9120 | 0x57b600 [g] |
| kijelol / kijelolagyu / kijelolinvert / kijelolinvertagyu | 0x698220 / 0x6afe20 / 0x69a630 / 0x6afec0 | 0x576ee0 / 0x576f50 / 0x577770 / 0x577820 |
| kijelolesleszed (clear selection) | 0x6ade40 | 0x5774e0 |
| csapat / csapatkijelol (groups 1-9) | 0x6b3ee0 / 0x6b3e50 | 0x572100 / 0x5721c0 |
| flottafelallit (select all, key A) | 0x68eae0 | 0x573a30 |
| infomodeonoff | 0x6afbe0 | 0x573f20 |
| panelkibe (dead) / exchangemouse (no callers at all) | 0x6aec90 / 0x6b1650 | 0x57d2b0 / none |
| initpanelek | 0x694ba0 | 0x5762e0 |
| infoszamol (selection and weapon statistics) | 0x6a4080 | 0x574040 |
| selectkisinfourhajo | 0x694340 | 0x580060 |
| kisinfokiir / nagyinfokiir | 0x6b4220 / 0x6b5e50 | 0x577c00 / 0x57c9c0 |
| SpaceWarkisinfo / SpaceWarnagyinfo (call-outs) | 0x67eea0 / 0x67eed0 | 0x563a20 / 0x563a40 |
| StarmapRovidInfoKiir (calls SpaceWarkisinfo) / StarmapGfx2D_3 (calls SpaceWarnagyinfo(0x140, 0x22)) | 0x70a920 / 0x6d4a30 | 0x5afc50 / 0x5aaba0 |
| kisinfogfxfrissit (3D ship model of the info box) | 0x6860c0 | 0x5778c0 |
| nagyinfofader (big info background box) | 0x6b26e0 | not decompiled |
| fegyverpanelfrissit (weapon column) | 0x6a7150 | 0x5727d0 |
| NezetVisszaAllitas (view animation) / NezetVisszaAllitSet | 0x6a8460 / 0x6aa030 | 0x56c420 / 0x56c120 |
| WriteRetreatWindow / ShowRetreatWindow / HandleRetreatWindow | 0x681090 / 0x680fd0 / 0x6814e0 | 0x571270 / 0x56d3f0 / 0x5679b0 |
| RadarFrissit (render callback of null box +0x2164) | 0x685290 | 0x56c890 (a label set by SetRenderOther in initgfx; not a Ghidra function) |
| lovedekekkirak / agyukirak (3D render callbacks) | 0x6823c0 / 0x6818c0 | 0x578d20 / 0x571480 |
| SpaceWar_Move (camera pan) | 0x67c5e0 | 0x563450 |
| GFX_DateMoneyScrollFade (tooltip box) | 0x66d160 | 0x554500 |
| Mouse_Szoveg_Set / _Off / _On / Mouse_Csak_Pos_Set | 0x5b7f90 / 0x5b8270 / - / - | 0x5108c0 / 0x5107c0 / 0x510810 / 0x510320 |
| gorbe (t>1: 1; t<0.5: 2t^2; else 1-2(1-t)^2) | - | 0x5839a0 |
| PlaySoundGlobal / InitSounds / DeInitSounds / PlayMusicSpWar | 0x6714c0 / 0x670ea0 / 0x671db0 / 0x61f5d0 | 0x55d0b0 / 0x55c790 / 0x55c460 / 0x540920 |
| SpWar_Hangok (42 records of 0x11c) / Global_Hangok | - | 0x91a838 / 0x919a60 |
| fegyvernevek (10 x 0x24: name[0x20] + group mask) / felallasnevek (10 x 0x14) | 0x1199960 / - | 0x9269b8 / 0x91d790 |

SpwC slot: ELF stride 0x3450, exe 0x2D9C; `AktSpaceWar` exe 0xE41788, `AktSpaceWarp` exe 0x91A830, slots from exe 0xE417C0.
SpaceWar_Main / GFX_SpaceWar run once per slot (10 slots, AktSpaceWarp pointed at each); only a slot with
`+0x24ba` (exe +0x1f2e) set acts: the slot of the human viewing the battle (MP: each client its own).

### Field map (ELF offset -> exe offset)
| Field | ELF | exe |
|---|---|---|
| hovered unit index / kind (0 none, 1 ship, 2 planet gun) | 0x2230 / 0x2234 | 0x1cd0 / 0x1cd4 |
| right held / left held | 0x2238 / 0x223a | 0x1cd8 / 0x1cda |
| events: right press / right release / left press / left release | 0x223c / 0x223d / 0x223e / 0x223f | 0x1cdc / 0x1cdd / 0x1cde / 0x1cdf |
| camera-rotate drag / height mode / box select / suppress | 0x2240 / 0x2241 / 0x2242 / 0x2243 | 0x1ce0 / 0x1ce1 / 0x1ce2 / 0x1ce3 |
| pointer inside the arena plane | 0x2244 | 0x1ce4 |
| left-press x,y / right-press x,y / mouse x,y / previous x,y | 0x2248,0x224c / 0x2250,0x2254 / 0x2258,0x2260 / 0x225c,0x2264 | 0x1ce8,0x1cec / 0x1cf0,0x1cf4 / 0x1cf8,0x1d00 / 0x1cfc,0x1d04 |
| camera yaw / pitch / distance | 0xd70 / 0xd74 / 0xd78 | 0xc80 / 0xc84 / 0xc88 |
| look-at now (x,y,z) / wanted | 0xd4c..0xd54 / 0xd5c..0xd64 | 0xc5c.. |
| per-tick inputs: zoom / yaw rate / pitch rate | 0x2300 / 0x2304 / 0x2308 | 0x1da0 / 0x1da4 / 0x1da8 |
| now ms / dt ms / dt s (dt/3000) | 0x22f0 / 0x22f8 / 0x22fc | 0x1d90 / 0x1d98 / 0x1d9c |
| view animation running / return flag / settled | 0x229e / 0x229d / 0x229c | 0x1d3e / 0x1d3d / 0x1d3c |
| gfx up / slot active / battle-end timer | 0x24b8 / 0x24ba / 0x24bc | 0x1f2c / 0x1f2e / 0x1f30 |
| saved cursor / armed weapon group / one-shot marker | 0x24c4 / 0x24c8 / 0x24cc | 0x1f38 / 0x1f3c / 0x1f40 |
| intro / leaving / fade-in / done flags | 0x2524 / 0x2544 / 0x2545 / 0x2546 | 0x1f98 / 0x1fb8 / 0x1fb9 / 0x1fba |
| big-info mode (0 off, 1 open, 2 closing) / slide factor / x centre | 0x2547 / 0x2548 / 0x254c | 0x1fbb / 0x1fbc / 0x1fc0 |
| stats of the selection: own ships 0x2550, selected ships 0x2554, sum+0x20 0x2558, sum+0x24 0x255c, damage (sum+0x18) 0x2560, max hp (sum+0x1c) 0x2564, value 0x2568, rank sum 0x256c, sum+0x1b8 0x2570 | 0x2550.. | ELF minus 0x58c (0x2554 -> 0x1fc8, 0x2564 -> 0x1fd8, 0x2568 -> 0x1fdc, 0x256c -> 0x1fe0) |
| snapshot of the live selection counters: total 0x2574, ships 0x2578, guns 0x257c, last idx 0x2580, last kind 0x2584, own 0x2588 | | 0x1fe8, 0x1fec, 0x1ff0, 0x1ff4, 0x1ff8, 0x1ffc |
| live selection counters: total 0x16f8, ships 0x16fc, guns 0x1700, last idx 0x1704, kind 0x1708, own 0x170c | | 0x15ac, 0x15b0, .., 0x15c0 |
| weapon groups: count[10] 0x30d8, max reload[10] 0x3100, enabled count[10] 0x3128, state[10] 0x3150 | | 0x2a30, 0x2a58, 0x2a80, 0x2aa8 |
| formation phase / leaving it / chosen / loaded | 0x342d / 0x342e / 0x3430 / 0x3434 | 0x2d85 / 0x2d86 / 0x2d88 / - |
| notices own / enemy ships retreating | 0x3438 / 0x3439 | 0x2d90 / 0x2d91 |
| panel records A / B / C (0x4c each, id base at +0x40) | 0x230c / 0x2358 / 0x23a4 (bases 0x234c / 0x2398 / 0x23e4) | bases 0x1dec / 0x1e38 / 0x1e84 |
| weapon-column slot i: meshes +0, +8, +0x10; x,y,z +0x18/+0x1c/+0x20; slide counter +0x28; on-target +0x2c | 0x29f8 + i*0xb0 | 0x2468 + i*0x94 |
| ship record: stride 0x290 (exe 600); selected +0x189 (exe +0x181); owner +0x14; group +0x1c4 (exe +0x1b8); retreating +0x1e0 (exe +0x1d4) | | |
| SPWRetreatWindow | | exe byte 0xE72DD1 |

### 0.2 The overlays' functions
| Function | ELF | exe |
|---|---|---|
| lovedekekkirak | 0x6823c0 | 0x578d20 (ret 0x57acaa) |
| RadarFrissit | 0x685290 | 0x56c890 (ret 0x56d3e8) |
| agyukirak | 0x6818c0 | 0x571480 (ret 0x571a4e) |
| villamfrissit | 0x6821c0 | 0x583830 |
| ElementList::Update (list head SpwC+0x3440; exe +0x2d94) | 0x4f0c00 | inlined in 0x578d20 (walk list, `call [vtbl+8]`) |
| DrawArrow | 0x6e2930 | 0x593210 |
| line3 / pixel / polygonsfader | 0x5efb60 / 0x5f0490 / 0x5f16a0 | 0x5338d0 / 0x535570 / 0x536100 |
| LineClip (-> TscVonal, line2d) | 0x536a30 | 0x4d3b00 |
| SikMetszespont | 0x5ee770 | 0x52ca10 |
| TransformPoint / RotateAndTransformPoint / TransformTav | 0x5ee370 / 0x5ee3f0 / 0x5eeb20 | 0x52cc90 / 0x52c670 / 0x52cce0 (inf) |
| kor (circle) | 0x5f3e40 | 0x533320 (inf, from agyukirak) |
| gorbe | 0x6b8a10 | 0x5839a0 (returns in x87) |
| GetObjCenter / KiirInt / KiirStr / GetString | 0x4562f0 / 0x538910 / 0x4f8570 / 0x533650 | 0x452280 / 0x4d3810 / 0x4a9e60 / 0x4c9a80 |
| tavnegyzet | 0x6b9100 | 0x583c50 |
| racspos2pos | 0x6851d0 | inlined (globals SpWarAkt X/Y/Z Range 0xf691e8/ec/f0) |
| SpaceWarLovedekKirajzol | 0x69ebc0 | 0x56e1c0 (`ret 8`; jump table 0x56e874, 7 valid entries) |
| shot-trail spawners particlerobbanas, 2, 3, 4, 5, 6 | 0x688cc0, 0x687a40, 0x687cf0, 0x687fa0, 0x688270, 0x688450 | 0x57dd40, 0x57d2f0, 0x57d510, 0x57d730, 0x57d960, 0x57daa0 |
| engine puff spawner particlerobbanas7 | 0x688630 | 0x57dbc0 |
| Hajtomu | 0x69a9c0 | 0x567140 |
| urhajomeghalas / ujparticlerobbanas / robbanasdarabok | 0x6a1690 / 0x688a50 / 0x686830 | 0x582550 / 0x5820c0 / 0x57f170 |
| seturhajopos | 0x686aa0 | 0x580540 |
| SpaceWar_ZoomFinished | 0x67fc50 | not located |

## 1. Scene set-up (`initgfx`, once per battle; guard: +0x24ba set and +0x24b8 clear; exe +0x1f2e / +0x1f2c)

### 1.0 Scene tree and lights
`spwmainxvb` is a child scene of the starmap's `mainxvb`: `Init3dStarmap` (`_local/decomp/starmap/Init3dStarmap.c`)
does `spwmainxvb = CreateXVB(); AddToScene(spwmainxvb, mainxvb, 1.0, &spwmainmeshid); PropertiesOn(spwmainmeshid,
0x10000)`. The battle is drawn inside the starmap's live 3D scene (its stars, planets and fleets stay and are shifted
by `sceneeltol`) with the starmap's lights: ambient 0x404040 on `mainmeshid` and a parallel light 0x808080 at
direction (10, 10, -10) (`Init3dStarmap` lines 58-64). The battle adds no scene-wide light. Its own lights are the
per-ship ambient lights (1.3) and the icon lights (step 21). `spwmainxvb2` (second XVB, camera-relative layer) holds
the small info model.
The battle planet, its cloud shells and the ships are nodes of this scene and take its light as the starmap planets
do: the parallel light turned back by the object's rotation, against the object-space normals, over the ambient; the
light shines along (10, 10, −10), i.e. from (−10, −10, 10) [obs: of the eight sign choices only this one fits]. The
control panel `kezel.xvb` (the starmap's console, `KezeloId`) is lit per vertex by `fenyekszamolasa`
(re/screens/research.md §2.1) with the ambient 0x404040, the parallel light and the starmap's two side lights
`smOldalsoFenyekBe` (ELF 0x6d81e0: 0x7f5d00 and 0x004e7f, bytes [r, g, b]), which `smMozgatMainFenyek` (ELF 0x6d8290)
points along `(sin a·300, cos a·200, 0)` and the same at `a + 1.57075`, `a = smkeringeto·20` (`smkeringeto` exe
0xF176DC: 0.0008 at the entry, `+ 0.0002` per UI tick): each a world vector `(h, x, y)` turned by the camera's rows (the
root's rotation chain), compared with the vertex normal with z negated, `shade[|n' − L|²·2047]`, saturating, in gamma
space [obs: the console matches at every grab of a 16-grab sequence]. The camera-fixed models (weapon column, info
model) are lit the same way but with the record rotation applied to the file normal (z negated) against the lights'
own vectors, not turned by the camera [obs]; the info model takes the parallel light without the side lights [obs]. Fog/background: none set here; the background is the starmap particle layer plus the 1000-star
field (step 19).

### 1.1 Order of `initgfx` (ELF 0x6923a0; the exe 0x575050 has the same order)
1. `+0x24b8 = 1`. `maxlevelszint = OptionsBuildingRes`. `savestarmapparticles()` (copies each FStars/DiffStars
   particle colour +0xc to +0x48; `restorestarmapparticles` is an empty `ret`).
2. `spacewarelfade = 0x3a83126f = 0.001f` (global fade; exe 0xF16EB4). `+0x2544 = 0`. Scene scale `+0x2514 =
   0x39d1b717 = 0.0004f` (=1/2500; exe +0x1f88).
3. Starmap camera capture: `+0x2528..` (exe +0x1f9c..) := {`forgaspont` x,y,z,w; `elforgatas`; tilt; `kamerapont.z`} as
   `ShowStarmap` left them (pivot (0, XStart, -YStart), yaw StarmapRotate, tilt StarmapPitch, zoom 600-Zoom); the same
   7 values go to the live pan/yaw/tilt/zoom fields +0xd4c(h,x,y,w) +0xd70 +0xd74 +0xd78 (exe +0xc5c..+0xc88). The battle
   therefore starts from the live starmap camera.
4. `VectorDir = "vector/spacewar/"`, `TextureDir = "textures/spacewar/"`.
5. Scene offset: `starmapeltolas` (x,y,z,w) = this+0x2500..0x250f (exe +0x1f74; the battle origin in starmap space, set
   by the caller); `+0x24f0..0x24f8 = 0` (exe +0x1f64); `sceneeltol(&starmapeltolas)` subtracts it from the live pan
   (0xd4c..), starmap particles tagged -1 and wormhole particles tagged -2 (+0xc..+0x14), every FStars (11 sets) and
   DiffStars (8 sets) particle position (+0,+4,+8), `Starmap_gombmesh2` (GetObjPos*1024 - offset, MoveObj), then runs
   `SpaceWarhajovonalkirajzol`, `initracs`, `RadarRangeKi`, and sets the pan target +0xd5c.. := +0xd4c... Then the
   saved camera pan +0x2528..+0x2530 -= starmapeltolas. `MoveObj(spwmainmeshid, +0x24f0..f8)`;
   `ScaleObja(spwmainmeshid, s, s, s)`, s = +0x2514. `starmapeltolas` is taken raw (y not negated): the entry look-at is
   `(0, XStart − o.x, −YStart − o.y)`, and a battle point q is the starmap point `starmapeltolas + q/2500` at every scene
   scale (`rescalescene` changes the camera's units, not the battle's place) [obs: the territories, stars, fleets and
   names line up during the zoom].
6. Four fighter-sprite models: `nemkellboxszamolas = 1; LoadBox("vad10.xvb", true)` x4 into +0x1750, +0x1798, +0x17e0,
   +0x1828 (= xvb field +0x10 of the four fighter PARTICLESTYPEs at +0x1740/0x1788/0x17d0/0x1818; exe +0x15e4/161c/1654/168c).
7. If +0xd7c != -1 (planet battle; exe +0xc8c): dirs `vector/starmap/`, `textures/starmap/`:
   - planet: `LoadBox(sprintf("b%d2.xvb", (Bolygok[planet*0x188 + 0x22] - 1) % 8), false)` (C remainder; planet idx
     +0x24b4, exe +0x1f28) -> `AddToScene(.., spwmainxvb, R*1.16, &+0x18b0)`, `RotateObj(pi/2, pi/2, 0)`, `MoveObj(P)`;
     R = (float)int +0x93c, P = (float)ints +0x930/934/938 (exe +0x858, +0x84c..854). The planet model has 528 faces [obs:
     node scale 1005.2 battle units].
   - dirs back to spacewar: `LoadBox("bolygo3.xvb")` twice -> shells +0x18a0 (size R*1.02) and +0x18a8 (R*1.04):
     `RotateObj(0, pi/2, 0)`, `MoveObj(P)`, `SetObjTransparency(0.7)` (0x3f333333); the outer shell
     `SetRenderOther(agyukirak, 0)` (ELF 0x6818c0, exe LAB_00571480; planet guns, §6.2). Exe 0x575425..0x575572: one
     `LoadBox`, two `AddToScene` (the sizes f64 products stored f32), after the planet, before `karikaload`.
     `bolygo3.xvb` is one `Sphere01` of 216 vertices at |v| = 1 under a 0.99876 child matrix (a partial shell; its
     centre 6.6 units off P by the child translation), material `ETHCLDBP.bmp` (the remaster's `CLOUD` pass, which takes
     its alpha from the procedural cloud, not the 0.7 byte). Lit, alpha-blended, depth write kept; nothing hides them;
     `frissit2` turns them (§6.6). Drawn before the shield rings, their depth hides every ring point under the outer shell
     [obs: rings 1029..1037 units from P vanish, 1065..1108 stay; shell node scales ~1041 / ~1061.5; a shell vertex 1028.4
     from the inner node at scale 1041.45, the partial shell's box re-measure].
   - `karikaload()` (0x694910 / 0x576e40): `LoadBox("karika.xvb")`, 20 nodes at +0x1fdc + 0xc*i: `AddToScene(.., 0.05)`,
     `SetObjShade(false)`, `RotateObj(pi/2, pi/2, 0)`, `MoveObj(0,0,0)`, `HideObj`, `PropertiesOff(0xc)`, +8 := 0
     (start tick; shield-hit rings, 3.4; exe nodes `+0x1a88 + 0xc*i`; §6.8.5).
8. Ships root: `CreateXVB()` -> +0x2178; `AddToScene(.., spwmainxvb, 1.0, &+0x216c)`. `loadurhajogfx(i)` for
   i < +0xd18 (exe +0xc28) (1.3).
9. NullBoxes on `spwmainxvb`: +0x20e4 (lines), +0x20d4 (effects/shots), +0x20dc, +0x2144, +0x214c, +0x2154, +0x215c
   (four fighter-sprite carriers). exe: +0x1b90, +0x1b80, +0x1b88, +0x1bf0, +0x1bf8, +0x1c00, +0x1c08.
10. Debris: `LoadBox("darab1.xvb")`, `("darab2.xvb")`, `("darab3.xvb")`; 40 nodes at +0x2594 + 0x1c*i: model
    `random(3)` (cosmetic libc stream, not reseeded yet), `AddToScene(.., 0.0015)`, `HideObj`. Node record: +8..+0x10
    spin vector, +0x18 age counter (0 = free).
11. `flottafelallit()` (0x68eae0 / 0x573a30): selects every own non-scenery unit (owner +0xd2c, +0x1e0 clear): +0x189 := 1,
    and, if +0xd7c == own empire, all fighter groups (+0x89 := 1); counters +0x16f8..+0x170c.
12. Plates: `LoadBox("lap.xvb")`; per unit i: `AddToScene(.., 0.0015, &unit+0x40)` (exe unit stride 600, +0x3c),
    `SetObjShade(false)`, `HideObj`, `SetObjTransparency(0.5)`, `RotateObj(0, pi/2, 0)` (twice, identical). Then
    `LoadBox("lap.xvb")` for +0x20ec + 8*i (i = 0,1): size 0.0015, unshaded, hidden, alpha 0.5, `PropertiesOn(4)`.
    `PropertiesOn(0x10000)` on the 7 NullBoxes of step 9.
13. `Lovedekload()` (0x694a00 / 0x569ec0), per used projectile form (1.4): `AddNullBox(spwmainxvb, form+0x38)`,
    `SetParticle(form+0x38, form+0x40, 1)`, `nemkellboxszamolas = 1; LoadBox(form.name)` -> form+0x30,
    `SetObjShade(false)`, `PropertiesOff(form+0x1c)`, `PropertiesOn(form+0x18 | 0x10000)`,
    `SetObjTransparency(form+0x20 / 255.0)`, `SetFrame(0)`; particle texture source form+0x50 = the xvb when
    form+0x24 != 0 or the xvb has more than one sub-box, else the first sub-box's xvb. Shots are billboard particles.
14. `SetParticle`: `Starmap_gombmesh2` <- the global star field `particles` (1 array); +0x20d4 <- +0x1528 (5 effect
    arrays); +0x2144/214c/2154/215c <- +0x1740/1788/17d0/1818 (1 array each). `SetRenderOther(+0x20d4,
    lovedekekkirak, 0)` (ELF 0x6823c0, exe LAB_00578d20; NOT decompiled: callees RotateAndTransformPoint, line3 x3,
    GetObjCenter, TransformPoint, TransformTav, KiirInt x2, GetString+KiirStr, villamfrissit, ElementList::Update,
    racspos2pos, tavnegyzet, DrawArrow = selection brackets, hit-point bars, names, lightning, order arrows).
15. `initpanelek()` (0x694ba0 / 0x5762e0) stores: +0x2324=55, +0x2328=80, +0x232c=176, +0x2330=107, +0x234c=25,
    +0x2370=200, +0x2374=1, +0x2378=167, +0x237c=220, +0x2398=14, +0x23bc=300, +0x23c0=30, +0x23c4=178, +0x23c8=352,
    +0x23e4=3 (panel rectangles/ids; for the 2D half).
16. `AddNullBox(mainxvb, +0x2164)`; `SetRenderOther(+0x2164, RadarFrissit, 0)` (ELF 0x685290, exe LAB_0056c890; the
    radar; not decompiled).
17. Viewport: `d = (IG2_WINDOW_WIDTH*480/IG2_WINDOW_HEIGHT - 640)/2` (int); `HatarX1 = (d*-639)/639`,
    `HatarX2 = ((d+639)*639)/639`, `HatarY1 = 0`, `HatarY2 = 479`, `HatarXm = HatarX2-HatarX1+1`, `HatarYm = 480`;
    `ModifyViewPort()`. `xcenter = 320.0` (0x43a00000), `ycenter = 250.0` (0x437a0000), `+0x254c = 320.0`.
18. Textures: `loadtexture("szar.bmp")` (+0x1890); `loadanimtexture("animated.bmp", 25)` (+0x1868); `("anim2.bmp", 25)`
    (+0x1870); `("smoke.bmp", 25)` (+0x1878); `loadtexture("star.bmp")` (+0x1880); `("texcsor.bmp")` (+0x1888). Effect
    array i (i < 5; record +0x1528 + 0x48*i) texture field +8 := animated, anim2, smoke, star, texcsor.
19. Star field: `particles = particlep` (static pool), count 1000 (`DAT_01717eac`), `DAT_01717eb0 = 100.0f`; `srand(0x66)`;
    for i < 1000: grey `g = (rand() % 255) / 2`; `(x, y, z)` = three draws of `rand() % 100 - 50` in that order; position =
    normalise(x,y,z) * 700.0 (`spwnormalizevektor`); colour = `g<<16 | g<<8 | g`; size (+0x14) = `(rand() % 100)/20.0 + 1.0`.
    ELF: `random(n)` (glibc); exe: CRT `rand()` (MSVC LCG): see 4.
20. `frissit2()`, `SpaceVadaszKirajzol()`, `initracs()`, `SpaceWarhajovonalkirajzol()`; `+0x2524 := 1` (first-frame flag; exe +0x1f98).
21. Weapon-panel icons, i = 0..9, record `+0x29f8 + 0xb0*i` (exe `+0x2468 + 0x94*i`): `LoadBox("ora.xvb")`, `("no.xvb")`,
    `("ikon<i>.xvb")` ("ikon0".."ikon9"). Icon node: `AddToScene(.., 0.02)` (0x3ca3d70a), `RotateObj(pi/2, 0, 0)`. `ora`
    (record +0x40) and `no` (+0x80): `AddToScene(.., 0.014)` (0x3c656042). All three `SetRelative(false)`, camera-space
    position x = 300.0 (0x43960000), y = i*42 - 200 (+15.0 when i == 9), z = -600.0 (0xc4160000); `MoveObj` on all three.
    Icon: `PropertiesOff(6)`, `PropertiesOn(0x20c0000)`; ora/no: `PropertiesOff(0xf)`, `PropertiesOn(0x20c0000)`;
    `SetFrame(ora, 15.0)`; `HideObj(ora)`, `HideObj(no)`; `CreateLightAmbient(0x808080, icon, &record+0x38)`. At fov 600:
    virtual x = 620, y = 250 + (i*42 - 200) (ycenter 250, screen y down; the hot-spots of §5.1 sit there). The ora/no
    `PropertiesOff(0xf)` clears property bit 8, so they are added (ONE/ONE, RGB × the opacity byte) over the buttons,
    which `PropertiesOff(6)` leaves blended (re/screens/render.md §2.3). `ora.xvb` is a vertex-animated mesh (`kirak`
    0x60e160, re/formats/xv.md): `SetFrame` blends its vertex frames (§5.1).
22. If +0x342d (formation panel; exe +0x2d85): `felallastexture = loadtexture("final2x.bmp")`; each icon's sub-mesh 0: material
    pointer saved at record+0x48, a copy at +0x50 with its texture field (+0x98 of the record) := that texture,
    `SetMaterial`; byte +0xac := 0. Otherwise record+0x48 := 0.
23. `+0xd5c..+0xd68 := +0xd4c..+0xd58`. Default yaw `+0x24d4`: planet battle -> pi (0x40490fdb); open space:
    `getcsoportcenter(own group)`; none -> 0; else v = normalise((0, c.x, -c.y)), yaw = pi - asin(v.x) when v.y >= 0 else
    asin(v.x) (f64 `asin`, pi = 3.141592653589793; stored f32).
24. State: `spwradarcounter = 0`; info-model state cleared (`kisinfomeshid`, `kisinfoobject` = 0; `elozokisinfotipus =
    kisinfotipus = -1`); `selectkisinfourhajo()`; `gfxvadaszdefendattack = side[own].+0x44`; the nine `spihany*/spi*`
    counters = 0; `visszaallitasidoszorzo = 1.0`; `initkamera = 1`; `SPWRetreatWindow = 0`; +0x2547 = 0; +0x2548 = 0;
    `infoelmegy = -1.0`; +0x2241 = 0 (height-pick off).

### 1.2 ELF vs exe in `initgfx`
Same order. exe: CRT `rand()` where the ELF has `random()`; strings built with `builtin_strncpy` (same text); the
`*639.0/639.0` (x) and `*479.0/479.0` no-ops remain only for y; pointer-size dependent offsets (PARTICLESTYPE 0x38 B instead
of 0x48; icon record 0x94 instead of 0xb0; unit 600 instead of 0x290).

### 1.3 Ship models (`loadurhajogfx(i)` 0x689710 / 0x578b20)
Only when the unit's id (+0x38, exe +0x34) is empty. `race` = unit+4, `type` = unit+8. Dirs: editor mode -> spacewar; else
`type < 8` -> `vector/spacewar/` + `textures/spacewar/`, `type >= 8` -> `vector/talalmanyok/` + `textures/talalmanyok/`.
Model name = the NUL-terminated string at `tipusnevek[race*0x240 + type*0x30]` (ELF 0x1190590, exe 0x91F488), loaded with
`LoadMaxLevelBox(name, true)` (`LoadUrhajoMesh` 0x6899d0; editor mode forces race 0x12, type 7).
`AddToScene(model, ships root +0x2178, size = tipusnevek[..+0x10] (f32), &unit+0x38)`; if unit+0x24c != 0:
`CreateLightAmbient(unit+0x24c & 0xffffff, node, &unit+0x250)`; `PropertiesOn(node, 0x10000000)`; then `seturhajopos(i)` (3.1).
Asteroids use race row 0x12. Designs 0x4d..0x50 (77..80) map to type 8..11 via the `TTeruletek` name table (`terv2tipus` 0x689a40).
The live unit's type record (exe unit `+0x214`) names the model and its scale. The file image's
`tipusnevek[race*12 + 8..11]` are "TSCtablazatabol" placeholders (both binaries); the exe's `terv2tipus` (exe 0x582010,
asm read) is their writer: for a hull tech t in 0x4d..=0x50 it copies `TTeruletek[t − 1]` (exe 0x919294 + 4t:
`ur_kurbazis.xvb`, `ur_urbazis.xvb`, …; the `RESEARCH_MODELS` entries, e.g. 0x4c / 0x4d at exe 0x9193c8 / 0x9193cc)
into `tipusnevek[race*12 + t − 0x45]` (exe 0x91F488 + 0x30*i, race = design `+0x40`) before returning t − 0x45, the scale
unchanged (0.02). `AddFleet`'s sort (exe 0x564d25, 0x564df9, 0x564e35) and a later caller (0x56585b) call it at the
battle's start, before any unit is added (found with a write watchpoint on 0x91FCC8). The ELF has no such copy. Drawn
pose: the node's matrix (exe unit `+0xe4`, 9 f32, §6.5) times the `AddToScene` scale; the overlays sit at the posed
model's box centre (`GetObjCenter`).

### 1.4 Projectile forms (`fegyveralakok`; ELF 0x1196800, 40 x 0x90; exe 0x9256F8, 40 x 0x78): fields the screen reads
+0 name (model), +0x10 f32 sprite size, +0x14 i32 pool size, +0x18 properties-on, +0x1c properties-off, +0x20 alpha 0..255,
+0x24 f32 anim period (0 = none), +0x28 f32 anim step, +0x2c kind 1..8 (jump table at ELF 0xd589c0 in
`SpaceWarLovedekKirajzol`), +0x30 xvb, +0x38 meshid, +0x40 PARTICLESTYPE and array, +0x88 shot array (0xd8 B per shot).
Static values in the ELF: forms 0..4 `lov1..lov5.xvb` size 2/2.2/2.5/2.8/3, pool 50, on 0x20, off 0xc, kind 0; 5 and 17
`ion01.xvb` size 3 pool 20 kind 4; 6 and 9 `mezon.xvb` size 0.5 anim 30/1.2 kinds 5 and 3; 7 `torpedo.xvb` size 5.8 pool 20; 8
`torpedo.xvb` size 0.1 pool 5 kind 6; 10 `antimat.xvb` kind 7; 11 `paral.xvb` alpha 200 on 0; 12 `manipul.xvb` alpha 200 on 0x60;
13..16 and 18..20 `raketa.xvb` size 1.5 pool 20 (kind 1; forms 18, 19 kind 0); 21 `lov1.xvb` size 0.5 kind 8; 30 `lov1.xvb`.
`SpaceWarLovedekKirajzol` (0x69ebc0) fills the 0x4c-byte particle of each shot: pos +0..8 = shot pos * scale; +0xc colour word
`alpha<<24 | 0xffffff`, alpha ramping from the form alpha to 255 over the last part of the life; +0x10 base size; +0x14 current
size = form size * scale (pulsing for weapon 0xb); +0x18..+0x24 direction; +0x2c frame counter stepping +0x28, wrapping at +0x24;
then it dispatches on the form kind through the jump table (not read).

### 1.5 Grid (`initracs` 0x694f90 / 0x576380) into line array +0xd00 (0x20 B per line: h, x, y, ARGB, h2, x2, y2, ARGB2)
Constants (`InitSpaceWar` 0x68c840 / 0x5684e0): cell +0x8e8/8ec/8f0 = 3.0 (x, y, h); counts +0x8f4 = 128 (x), +0x8f8 = 128 (y),
+0x8fc = 10 (h); bounds +0x900/904 = -192/+192 (x), +0x908/90c = -192/+192 (y), +0x910/914 = -15/+15 (h); extents
+0x918/91c/920 = 384/384/30; +0x928 = (10/2 + 1)*3 = 18 (grid plane h); +0x92c = (128+128)*3 + 1 = 769 lines. Line 0 = the height
pole (3.7); lines 1..768 = 3 segments per grid line. With s = +0x2514, o = +0x24f0/f4/f8:
- x loop (i = 0..127): c = ((i-64)*3.0 + 1.5)*s + o_x; the line runs along y from ymin = +0x908*s + o_y to ymax = +0x90c*s + o_y at
  (h = 18*s + o_h, x = c), split at ymin/2 and ymax/2 into three segments; the two outer ends have alpha 0.
- y loop (i = 0..127): c = ((i-64)*3.0 + 1.5)*s + o_y; along x from +0x900*s + o_x to +0x904*s + o_x at (h = 18*s + o_h, y = c); same split.
- colour: t = (64 - abs(i - 64)) / 32.0 (the 32 is (count/2)/2 in int); A = t <= 1 ? (int)(255 - 63*t) : 0xC0; word = A<<24 | 0x402000
  (editor: A = 0x40, constant 0x40402000).
- rebuilt by `rescalescene` and `sceneeltol`; drawn by `SetBoxLine(+0x20e4, +0xd00, +0x1fd8)` each frame.
The CONCAT44 overlays make each segment end ambiguous (see 5).
Drawing (the engine's `Lines`, after `Kocka` in `renderscene`): added, not blended, the colour scaled by the word's A
byte onto the scene [obs: a grid pixel (17, 35, 44) → (17, 42, 59) = + 0x402000 × 0.25; A read as a transparency fits
better than as an opacity]; one screen pixel wide, nearest first under a strict depth test with depth writes (crowded
far lines stay one line bright); the segments' outer ends add nothing [obs].

### 1.6 Drop lines (`SpaceWarhajovonalkirajzol` 0x68ecb0 / 0x570960)
`+0x1fd8 := +0x92c`. For each live unit with owner != 0 (or editor) and (owner == own empire +0xd2c, or +0x1e3 clear) one line is appended:
p0 = (h = 18*s + o_h, x = unit.x*s + o_x, y = unit.y*s + o_y), p1 = (h = unit.h*s + o_h, same x, y). Unit fields ELF +0x48 h, +0x4c x,
+0x50 y. Alpha A = 200 when unit +0x1e0 is clear, else min(255, (int)(unit.+0x10c*57 + 200)). RGB by rel[own][owner] (+0xd98: 0 same
empire, 1 enemy, 2 other; InitSpaceWar: Viszony[..+0x25] == 1 -> 1 else 2): 0 -> 0x00C000, 1 -> 0x0000E0, 2 -> 0x00C0E0 (words
0xBBGGRR, §8.1: an enemy's line is red [obs: + (48, 0, 0) = 0xe0 × 55/255]); both ends A<<24 | RGB. Array room: 0x78 lines beyond the grid.

## 2. Camera
Globals (ELF / exe): `forgaspont` (target/pivot; h,x,y,w) 0x167ec50 / 0xDE0130; `kamerapont` (eye; x,y at +0, z at +8 = `DAT_0167ec68`)
0x167ec60 / 0xDE0148; `elforgatas` (yaw, CameraRotate2 only) 0x167ec70 / 0xDE0158; `DAT_0167ec74` tilt / 0xDE015c; `DAT_0167ec78`
gamma / 0xDE0160; `zbm` 0x118b220 / 0x91965C. The battle renders with `Update3D(NULL, NULL, 1)` -> `renderscene(1)` ->
`CameraRotate(kamerapont, forgaspont, 0, ...)` (the look-at variant, ELF 0x5fcf90): eye = `kamerapont` (ABSOLUTE world position),
target = `forgaspont`, up = +h axis (`CreateMatrix2v(normalise(target - eye), (1,0,0), 0)`), view point `e = M * (p - eye)`:
`d = normalise(target − eye)`, `c = normalise(d × (1, 0, 0))`, `b = normalise(d × c)`, view rows `c`, `−b`, `−d` (ahead is
`e.z < 0`; checked in the C); screen `600·e/−e.z + (xcenter, 250)`, +y down. `TransformPoint` adds `xcenter` (`+0x1fc0`:
320, 160 in the info view, animated with the view, §2.5) to every scene point, the starmap's too.
The starmap uses `renderscene(0)` = `CameraRotate2` (yaw/tilt/gamma) instead, so `elforgatas`, tilt and gamma are not read by the
battle render. `renderscene`: `zbmc = zbm*1.05; zclip = -zbmc`; `calculateclipplanes`; clears the z-buffer over HatarX1.. unless
`ColonyMasodikUpdate3d`; then `Kocka` (tree incl. render callbacks), `Lines(linepointer, hanyline)`, `poligonok`. Fov 600 and
(xcenter, ycenter) = (+0x254c, 250) as on the other 3D screens (starmap.md 2.2).

### 2.1 State (SpwC fields, ELF; exe in brackets where seen)
- +0xd4c (h) +0xd50 (x) +0xd54 (y) +0xd58: pan pivot (live) [exe +0xc5c]; +0xd5c..+0xd64: pan target (input moves the target, the pivot
  chases it each tick) [exe +0xc6c]; +0xd70 yaw [+0xc80]; +0xd74 tilt [+0xc84]; +0xd78 zoom (distance) [+0xc88]; +0xd7c planet index or -1 [+0xc8c].
- +0x2300 zoom-delta factor, +0x2304 yaw delta, +0x2308 tilt delta (inputs; `FRISSIT_SpaceWar2` zeroes 0x2300 and 0x2304 after
  `SpaceWarFrissit2`, not 0x2308).
- +0x254c xcenter (320, or 160 in info mode) [exe +0x1fc0]; +0x2514 scene scale (0.0004 -> 1.0) [+0x1f88]; +0x24f0.. scene offset [+0x1f64].
- Default view targets: +0x24d4 yaw, +0x24d8 tilt (-0.8f = 0xbf4ccccd), +0x24dc zoom, +0x24e0..+0x24e8 pan (0,0,0) [exe +0x1f48, +0x1f4c, +0x1f50, +0x1f54..].
- Smoothed camera (per tick): `spwkamerapont` 0x1717f54 (x,y) / z at 0x1717f5c (exe 0xE72DD4), `spwforgaspont` 0x1717f64 / z 0x1717f6c (exe 0xE72DE4); targets `tkamerapont`
  0x1717f78 / z .f80, `tforgaspont` 0x1717f88 / z .f90; `initkamera`, `vadaszlinked`, `aktvadasz`, `elozovadasz`, `spwcameramode` (0x1703008).

### 2.2 Per tick: `kamerabeallit` (ELF 0x6a2cf0 / exe 0x576750 inf); runs at the end of `SpaceWarFrissit2`; does nothing while +0x2524 (first frame) is set
```
if spwcameramode != 0 and restore-anim (+0x229e): spwcameramode = 0
if (double)tilt < -1.5607963267948965: tilt = -1.5607964f (0xbfc7c82d)          // f64 compare, f32 store
P = (+0xd4c, +0xd50, +0xd54);  forgaspont = P
M = CreateMatrix(tilt, -yaw, 1.5707964f)                                       // f32; argument order (tilt, -yaw, gamma)
tforgaspont = P
tkamerapont = M * (0, 0, zoom) + P      // f32: m02*zoom + P.h, m12*zoom + P.x, m22*zoom + P.y  ~ P + zoom*(sin t, sin y * cos t, cos y * cos t)
if spwcameramode == 0: vadaszlinked = 0; aktvadasz = -1
else:  zbm = 1.0; g = first fighter group with flag byte +0 bit 0 (stride 0xb8; exe 0xa8)
       if g found: if elozovadasz != g: elozovadasz = g, vadaszlinked = 0, aktvadasz = -1;  if vadaszlinked: aktvadasz = g
                   tkamerapont = g.pos (+4,+8,+0xc);  tforgaspont = g.pos + g.dir (+0x48,+0x4c,+0x50) * 10.0f     // exe dir at +0x40..+0x48
smoothing:
  if initkamera: spw = t (no easing); initkamera = 0
  else: if spwcameramode != 0 and !vadaszlinked and tav(spwkamerapont, tkamerapont) < 1.0: vadaszlinked = 1
        if !vadaszlinked and !restore-anim: spw = t + (spw - t) * 0.85f     (six f32 components, MULSS by the f32 constant at ELF 0xd587b4)
        else spw = t
```
The cosine of 1.5707964f is -4.37e-8 (not 0): keep the f32 CreateMatrix (starmap.md 2.2 has its form).

### 2.3 Per frame, just before `Update3D` (`screenfrissites`)
```
save kamerapont/forgaspont (+ their z halves)
if +0x2524 (first frame): +0x2524 = 0; kamerabeallit(); +0x2524 = 1
kamerapont = spwkamerapont (x, y, z);  forgaspont = spwforgaspont
if !restore(+0x229e):
    if spwcameramode != 0 and aktvadasz >= 0 and vadaszlinked:          // exact follow of the live fighter group, not the per-tick copy
        eye = g.pos;  target = g.pos + g.dir * 10.0f                    // ELF fields +4..+0xc, dir +0x48..+0x50
else:                                                                   // intro / outro / mode-switch animation: straight from the fields
    M = CreateMatrix(tilt(+0xd74), -yaw(+0xd70), 1.5707964f); forgaspont = (+0xd4c..); kamerapont = M*(0,0,zoom(+0xd78)) + forgaspont
zbm = tav(kamerapont, forgaspont) / 6.0f
Update3D(0, 0, 1);  then restore the saved globals
```
exe 0x57f800 is the same (restore flag +0x1d3e, fields +0xc5c..+0xc88; `tav` = 0x583bf0 returns an x87 value, the division by 6.0 is done in extended precision then stored f32).

### 2.4 `SpaceWarFrissit2` camera part (ELF 0x6a3380 / exe 0x56d870), per tick, only while !restore (+0x229e)
- zoom: `+0xd78 -= (+0xd78 / 25.0f) * (+0x2300)`; if zoom <= 5.0: zoom = 5.0 (0x40a00000); if +0x2300 < 0 (zooming out) and zoom > 400: zoom = 400.0 (0x43c80000).
- pivot clamp (cells are 3.0): h in [+0x910 + 2*cell_h, +0x914 - 2*cell_h] = [-9, 9]; x in [+0x900 + 2*cell_x, +0x904 - 2*cell_x] = [-186, 186]; y in
  [+0x908 + 2*cell_y, +0x90c - 2*cell_y] = [-186, 186]. World units, valid after the scene scale is 1.0.
- pivot chase: d = target(+0xd5c..) - pivot; if any |d_i| >= 0.01: d /= 4; len = |d| / 4; if len > 1.5: d *= 1.5 / len (step length <= 6.0); pivot += d;
  else pivot := target (snap). f32.
- yaw `+0xd70 += (+0x2304) / 60.0f`; tilt `+0xd74 += (+0x2308) / 250.0f`; tilt < -1.5607964 -> -1.5607964 (0xbfc7c82d); tilt > -0.2 -> -0.2f (0xbe4ccccd).
- then `kamerabeallit` (2.2) at the end of the tick.
`FRISSIT_SpaceWar2` (0x67c450 / 0x55e6e0): `SpaceWarFrissit2(AktSpaceWarp)`; zero 0x2300 and 0x2304; with ControlConfig[0] and [1]: refresh four
`cValueInterpolator2` (MoveX, MoveY, Zoom, Rotate) with dt = 0.033333335f, copy zoom -> +0xd78 and rotate -> +0xd70 when not in the first-frame/restore state,
`SpaceWar_Move(speedX, speedY)` (pan keys).

### 2.5 Intro / outro / mode switches (`NezetVisszaAllitSet` 0x6aa030 then `NezetVisszaAllitas` 0x6a8460)
`NezetVisszaAllitSet()` (start): `visszaallitasidoszorzo = 1.0`; `+0x229e = 1`; origin O := current: yaw `+0x22a0 = (y/2pi - floor(y/2pi)) * 2pi` (f64 math,
stored f32), tilt `+0x22a4`, zoom `+0x22a8`, xcenter `+0x22b0 = +0x254c`, pan `+0x22b4..+0x22bc = +0xd4c..`. Target T (+0x16a4 yaw, +0x16a8 tilt, +0x16ac zoom,
+0x16b0 xcenter, +0x16b4..+0x16bc pan):
- A: +0x22ec clear (default): T = defaults (+0x24d4, +0x24d8, +0x24dc, pan +0x24e0..), xcenter 320.0 (0x43a00000); wrap: while T.yaw <= 0 add 6.2831855f, same for O.yaw;
  d = T.yaw - O.yaw; d >= 0 and d > 3.1415927f -> T.yaw -= 6.2831855f; d < -3.1415927f -> T.yaw += 6.2831855f; +0x22e8 = -1.
- B: +0x22ec set and no selection (+0x16f8 == 0): as A with T.yaw := O.yaw (yaw kept).
- C: +0x22ec set with a selection: +0x22e8 := index of the first selected unit; `kijelolescenter(&T.pan, &T.zoom)`; xcenter 160.0 (0x43200000); T.yaw := O.yaw; T.tilt default.
Both: `CreateMatrix(T.yaw, T.tilt, 1.5707964f)` into +0x16c4 and for O into +0x22c4 (not read by the code seen); start time `+0x22ac = (float)SDL_GetTicks()` (+0x22f0).
`kijelolescenter` (0x6b1a40): n = +0x3414 selected units (indices at +0x3234..): n = 0 -> pan 0, zoom 40.0; n = 1 -> pan = unit pos, zoom = type scale * 1024 * 3.0; n >= 2 ->
pan = centroid, zoom = 2.5 * max distance to the centroid; returns 0 when a selected unit is still alive.
`NezetVisszaAllitas()` each frame while +0x229e:
```
t = ((float)now - (+0x22ac)) / (float)(+0x2520) * visszaallitasidoszorzo / 50.0f        // all f32; +0x2520 = 80 -> 4000 ms at x1, 800 ms at x5
if +0x22e8 != -1 and !0x229d and 0x22ec set: re-run kijelolescenter into (+0x16b4 pan, +0x16ac zoom) every frame (follows the selection)
g = gorbe(t):  t > 1 -> 1;  (double)t < 0.5 -> 2*t*t;  else 1 - 2*(1-t)^2                  // f32; gorbe2(x) = x<1 ? x*x : 2*x-1
yaw :  !0x229d: (0x22ec clear) O.yaw + (T.yaw - O.yaw)*g ;  (0x22ec set) O.yaw + gorbe2(t/2)  [a steady spin];  0x229d: O.yaw + (T.yaw - O.yaw)*gorbe(t)
tilt = O.tilt + (T.tilt - O.tilt)*g;   xcenter = O.xc + (T.xc - O.xc)*g
L = log((double)(O.zoom / T.zoom));  zoom = (float)((double)O.zoom / exp((double)(g * (float)L)))          // geometric interpolation, f64 log/exp
p = g;  if |(float)L| > 0.1: p = 1 - (T.zoom - zoom) / (T.zoom - O.zoom)
pan(h,x,y) = O.pan + (T.pan - O.pan) * p
if +0x2545 (entering): spacewarelfade = g;  if +0x2544 (leaving): spacewarelfade = 1 - g;  if spacewarelfade == 0: 0.001
if g >= 1:
    if !0x229d { if !0x22ec: +0x229e = 0;  +0x229c = 1;  zoomdone = true }  else { +0x229e = 0; +0x229c = 1; +0x2547 = 0 }
    if +0x2544: +0x229e = 0; AktKepKell = starmaphovaleptovabb or 2; spacewarelfade = 0.001
    if +0x2545: rescalescene(2500.0); +0x2545 = 0; spacewarelfade = 1.0
    if nemlehetidotallitani: KepernyokLetiltva[0xd] = backKepernyokLetiltva[0xd] & 1; nemlehetidotallitani = 0
    if zoomdone: SpaceWar_ZoomFinished()
if +0x2547: nagyinfofader(g)
pan target (+0xd5c..) := pan;   if kijelolescenter returned 1 (selection gone): infomodeonoff()
```
`now` is `+0x22f0`, which `screenfrissites` stores after `NezetVisszaAllitas` has run (§3 steps 3 and 6), so the curve reads
the previous frame's time (one frame late, 10 ms at 100 Hz) [obs].
First frame of a battle (end of `screenfrissites`, when +0x2524 is set, after `Update3D`): `+0x2520 = 80`; tilt default `+0x24d8 = -0.8f`; zoom default
`+0x24dc = bezoomolasztav() * +0x2514`; pan default (+0x24e0..e8) = 0; `+0x22ec = 0`; `NezetVisszaAllitSet()`; `+0x229d = 0`; `+0x229c = 0` (input locked);
`+0x2524 = 0`; `+0x2545 = 1` (entering); `+0xd6c = (int)(+0x24d4 / 1.5707964 + 0.5) * 1.5707964` (quantised view yaw for the keys); `visszaallitasidoszorzo =
5.0 (0x40a00000) if spwoldkep == 0xd else 1.0`; `KepernyokLetiltva[0..19] = 1`; `nemlehetidotallitani = 1`; `spwckezdotimer = SDL_GetTicks()`.
`bezoomolasztav` (0x6a9ed0): 200.0 when there are no units; else 2.5 * max `tav((0,0,0), unit.pos)` over live units with +0x1e0 clear; 200.0 if that max is 0.
So the intro zooms from the starmap camera (scene scale 0.0004, starmap particles around) to the battle default; when g reaches 1 `rescalescene(2500)` makes the
scene scale 1.0 and the fade 1. Exit (`Starmapvisszaallit` 0x6b4020 / 0x570c10): `rescalescene(0.0004)`; `+0x2544 = 1`; `+0x2520 = 80`; defaults := the saved starmap camera
(+0x2538 yaw, +0x253c tilt, +0x2540 zoom, +0x2528..+0x2530 pan); `+0x22ec = 0`; `NezetVisszaAllitSet()`; `AktKep = 0xb`; at g >= 1 `AktKepKell = starmaphovaleptovabb or 2`.
`NezetVisszaAllitSet2` (0x6b2b00; called from `infomodeonoff` 0x6afd77): the current origin becomes the target (T := +0x22a0..), the origin is re-captured from the live camera (no default recompute, no wrap).
`rescalescene(k)` (0x6b1e60): multiplies by k: the smoothed camera x,y,z, +0x2514, `starmapszorzo`, zoom +0xd78, pan +0xd4c..+0xd54; re-applies `MoveObj`/`ScaleObja` on `spwmainmeshid`,
`ScaleObj2` on `Starmap_gombmesh2` (and its position x k*1024), planets and clouds (`PlanetId`, `CloudId`, `CloudId2` x HanyBolygo), scales every FStars/DiffStars/effect/star particle
(+0,+4,+8,+0x14,+0x10) and the lightning (`villam::scale`), rebuilds `SpaceWarhajovonalkirajzol`, `initracs`, `SpaceVadaszKirajzol`, all shots (`SpaceWarLovedekKirajzol` x 40), `RadarRangeKi`, and sets the pan target = pan.

### 2.6 Edge scrolling: `SpaceWar_Autoscroll` (ELF 0x67dac0 / exe 0x5624c0, asm read)
Every UI tick (`Fake_FRISSITES` 0x4de94d, after `FRISSIT_SpaceWar2`), only with `ControlConfig[0] == 0`, the tutorial off
(0xF6915D), `AktKep == 0xb` and no screen change (`Atlepes`/`Belep` 0xBAE24D/4C clear):
- Target per axis from the **physical** pointer (window pixels 0xDB9408/0C, window 0xFD171C/20; widescreen.md §4):
  `x < 8` (signed) → −30; `x >u W − 8` (unsigned `cmova`) → +30, so a pointer left of / above the window counts as the far
  edge; y likewise. None while any mouse button is held (`Mouse_Gomb` 0xDB4650).
- Normalised to length 30, x first, then y **with x's new value**: `x' = trunc(30x / f32(sqrt(f64(f32 x² + y²))))`,
  `y' = trunc(30y / f32(sqrt(f64(f32 x'² + y²))))` (a corner gives (∓21, ∓24)).
- Speeds `v` (globals 0xE5DFD8/DC, never reset): `if t > v: v += 5; if t < v: v −= 5` (the second test sees the new v),
  so v settles on a multiple of 5 next to the target (−21 → −25, 21 → 20).
- `v ≠ 0`: `SpaceWar_Move(f32(v.x / 5.0), f32(v.y / 5.0))` (exe 0x563450, the f64 5.0 at 0x852390): with `c, s` = f32
  cos/sin of the yaw `+0xc80` and `K = scale +0x1f88 · plane +0x844 + zoom +0xc88` (f32): for dx `x −= f64(c·dx·K / 600f) ·
  0.6`, `y += f64(s·dx·K / 600f) · 0.6`; for dy `x −= f64(−s·dy·K / 600f) · 0.6`, `y += f64(c·dy·K / 600f) · 0.6`
  (f64 0.6, results to f32); after each axis the wanted look-at `+0xc6c..` := the look-at. The exe has no step refusal
  (the ELF's `SpaceWar_Move` loops `SpaceWar_Move0` and shrinks a refused step): the bound is the per-tick look-at clamp
  of §2.4 (±(192 − 2·3) = ±186), which the scroll overshoots between two clamps.
- [obs: the cursor in the window's top-left corner, UI ticks 420 → 421 at yaw π, zoom 436.356: the pivot moves
  (−2.27178, +2.27178) per tick.] With the real cursor outside the game window the camera drifts to an arena corner
  (pivot ≈ ±189: the ±186 clamp plus the scroll's overshoot), so a capture must pin 0xDB9408/0C to its pointer.

## 3. Per-frame order (`screenfrissites` ELF 0x6a7780 / exe 0x57f800); `GFX_SpaceWar` runs it for all 10 slots, each guarded by +0x24b8
1. `kisinfogfxfrissit()` (0x6860c0): the small info model. On a `kisinfotipus` change (t): free the old model; race = t/12, type = t%12 (t == 5000: fighter `vad11.xvb`; t >= 50000: building, name from
   `TTeruletek` of the planet building list; else `tipusnevek[race*0x240 + type*0x30]` with the dirs of 1.3); `AddToScene(.., spwmainxvb2, 0.03)`, `PropertiesOn(0x2000000)`, `SetRelative(false)`,
   `RotateObj(pi/2, 0, 0)`, `MoveObj(-545, 365, -1100)`, `UnhideObj`, matrix saved to `kisinfomatrix`. Every frame: `SetObjMatrix(kisinfomatrix)`, `matrixforgat(m, m, 0.05, 0.0)` (turntable),
   `SetObjTransparency(1.0 - spacewarelfade)`. `selectkisinfourhajo` (end of `infoszamol`) picks the type; the turn restarts at 0 when the
   type changes, then +0.05 per drawn frame after the draw, so its phase depends on the frame rate. Building models: `TTeruletek[t − 1]` from
   `talalmanyok`, `t = EpuletekLeiro[Epuletek[planet][slot].type] +0` with 0x71 and 0x73 swapped. Property 0x2000000: depth `1 − 1/−z`.
2. Mouse pointer type (`MousePointerType` 0 -> 1, 2 -> 5, 3 -> 7, 4 -> 6) into `Mouse_Tipus`.
3. `NezetVisszaAllitas()` (2.5): camera animation and fade.
4. `starmapszarok()` (3.3): the starmap background keeps animating.
5. `pulzaljspacewarcsillag()` (3.5): star twinkle.
6. Time: `+0x22f0 = SDL_GetTicks()`, `+0x22f8 = now - previous` (ms), `+0x22fc = dt / 3000.0f`, `+0x22f4 = now` (after step 3: §2.5).
7. Retreat window: `if !Mouse_Bal and SPWRetreatWindow: HandleRetreatWindow()`; `if SPWRetreatWindow: ShowRetreatWindow()`.
8. `fegyverpanelfrissit()` (0x6a7150 / 0x5727d0): weapon-panel state (2D half).
9. `kijelolesanim(dt)` (3.6). `animracs()` and `KeretBeallit()` are empty (`ret`). `+0x2268 += dt`, `+0x226c += dt`; if !IdoPause: `animtick = (int)((float)animtick + (dt/3000.0f)*3000.0f)`.
10. `xcenter = +0x254c`; `SetBoxLine(+0x20e4, +0xd00, +0x1fd8)`; `magassagkirak()` (3.7); `GFX_DateMoneyScrollFade()` (top bar).
11. Camera set-up (2.3), then `Update3D(NULL, NULL, 1)`. Update3D (ELF 0x605940 / exe 0x52cd10): debug counters only when `DEBUGX`; no click handling (param_1 NULL);
    `maxszamlalo = 0`; `renderscene(1)` (z-clear of the viewport, look-at camera, `Kocka` tree walk incl. the SetRenderOther callbacks, `Lines`, `poligonok`); `MousePointer()`; fps
    (`framecount`, `tickcount`, halved when AktKep == 3); `TutorialSzovegFrissit()`; `Mouse_Debug_Kiir()`. Camera globals are restored afterwards.
12. `setsounds()` (0x69d8a0), `SpaceWarMouse()` (0x6a9490).
13. First-frame block (end of 2.5).
14. Text: `KiirStrSzinSzorzo = (int)(spacewarelfade * 100.0f)` (text alpha follows the fade; restored at the end); debug lines (+0x3438, +0x3439, +0x342d flags) at y = 30, 40, 50; in MP with
    more than 1 empire the caption "%s (%s) vs. %s (%s)" at (200, 30) for 9000 ms after spwckezdotimer (alpha `(9000 - dt) / 5` capped at 100 %); `WriteRetreatWindow()`.

Per tick: `SpaceWarFrissit2` (§7.2; its camera part §2.4; the sim tick is separate, space-combat.md 7.1). The `frissit2` pass from the sim tick: §6.6.

### 3.1 Ships (`seturhajopos(i)` 0x686aa0; called from the sim `Mozgatas`, `urhajomeghalas` for asteroids, `loadurhajogfx`; exe 0x580540 per space-combat.md)
Visibility, orientation and bank: §6.5. Heading easing: wanted = direction to the target ship / waypoint / next position; mid = normalise(normalise(wanted) + dir) -> +0x124; angle a = vektorszog(wanted, dir);
if a > 0.01 rotate dir (+0x114) about (dir x wanted) by min(a, min(0.1, a/4)) (`creatematrixfromaxis` + `rotatepointmatrix`), normalise. Then `MoveObj(node, pos(+0x48..))`.
Engine trails: `Hajtomu` (§6.3.2).

### 3.2 Fighters (`SpaceVadaszKirajzol` 0x694e50 / 0x56d460; sprites are particles in the sets at +0x1720 + 8*class)
Per group j: p = set[g.+0xb0][g.+0xb4] (4 sets by the plan's fighter class, §7.4). Alive and j != aktvadasz: `p.xyz = g.pos * s`, `p.size(+0x14) = p.size(+0x10) * s`, `p.+0x18..0x24 = g.dir (+0x48..)`, `p.+0x28 = 0`; else `p.+0x14 = 0` (hidden).
The followed group (camera mode) is not drawn.

### 3.3 Starmap background (`starmapszarok` 0x6a64c0 / 0x581bc0)
`futofeny2 += 1`; `RadarRangeKi()`; `e = log((double)(zoom / starmapszorzo)) / 3 + 0.4` (f64 log, f32 store), clamped to [0, 1] -> `starmaperosseg` (0 at full battle zoom -> starmap objects hidden:
`starmaphideflottak = 1`, `HideObj(starmapnulla)`, `HideObj(starmapRnulla)`; else shown). Then the 40-particle ring in FStars set 3 (pointer at FStars+216): radius from `TransformTav(1.0, 600 - StarmapZoom)` and
`TransformTav(0.3, ...)`; position (0, cos(i*0.15707964)*10/sqrt(-t1)*starmapszorzo, sin(...)*...); two moving highlights at index `(int)(futofeny2 mod 40)` and +-20 (size starmapszorzo*2), the rest shrinks by 0.2*starmapszorzo down to
0.5*starmapszorzo; colour word `(255 - a)<<24 | b<<16 | g<<8 | r` with (r, g, b) = `StarmapMode == 1 ? (e*32, e*128, e*255) : (e*255, e*255, e*0)`, a from the starmap info/fleet mode fade (255, or the zoom ratio formula, or 64 + frame*23).
Then `rotate_stars()`, `rotate_fleets()`, `PulzaljKisCsillag()`, `PulzaljNagyCsillag()`, `StarmapFrissitBolygoUrhajok()`, `smMozgatMainFenyek()` (the two starmap side lights circle), `starmaphalvanyit(e)`.

### 3.4 Shield rings (`karikakirak` 0x69c650 spawn, `karikafrissit` 0x6a2bb0 animate; planet battles only)
Spawn: the first of 20 rings with +8 == 0: `MoveObj(ring, hit point)`, `+8 = tick (+0xd3c)`, orientation `CreateMatrix2(normalise(b - a), 0)` -> `SetObjMatrix`.
Animate: `f = ((40.0f - 800.0f / (float)((tick - start) + 20)) / 28.0f) * 35.0f`; f <= 30: `SetFrame(f)`, show, `SetObjTransparency(f / 30.0f)`; else `+8 = 0` (free). Free rings are hidden each frame.

### 3.5 Star twinkle (`pulzaljspacewarcsillag` 0x6815d0 / 0x57e420)
Tables (first call): rndbuf1[i] = (float)(((sin((double)((float)i * 82.0f)) / 3.0 + 1.0) / 4.0) * 180.0 / 3.1415) for i < 1000; rndbuf2[i] = (int)(sin((double)((float)i * 415.0f)) * 30.0 + 120.0);
sintable[j] = (int)(sin((double)((float)j / 180.0f) * 3.141592) * 90.0) for j < 360.
Per frame: base = (int)(futofeny / 10.0f); star i: idx = ((int)((float)(i + base) * rndbuf1[i])) % 360; A = (int)(255.0f - (255.0f - (float)(sintable[idx] + rndbuf2[i])) * (1.0f - starmaperosseg));
colour word `A<<24 | 0x646464` stored at +0xc and +0x48 of the star record (stride 0x4c).

### 3.6 Selection timers (`kijelolesanim(dt)` 0x6a8b20 / 0x576fc0): data only, dt = +0x22f8 ms
Clears every unit +0x198 (attacker pointer) and every fighter group +0xa0 first. Per live unit: if selected (+0x189) != previous (+0x18a): on select clear +0x18c (grow timer) and +0x190 (fade timer) to 0
(on deselect only +0x190 = 0); +0x18a := +0x189. +0x18c += dt (cap 1000); if not selected +0x190 += dt (cap 1000). When its order type (+0x164) == 3, it is selected and +0x158 (target) != 0:
target.+0x198 = unit (if +0x160 == 1, unit target) or target.+0xa0 = unit (if +0x160 == 2, group target). Fighter groups (list +0xd88, stride 0xb0 in this list; +0x84 live, +0x89 selected, +0x8a previous, +0x8c/+0x90/+0x94/+0x10 timers capped 1000,
+0x98 target unit index) behave the same. Last: unit +0x194 += dt (cap 1000) while something marks it as attacked (+0x198 != 0) else 0 (groups: +0x94). The drawing of the brackets, bars and arrows is in `lovedekekkirak` (not decompiled).

### 3.7 Height pole (`magassagkirak` 0x6a9120 / 0x57b600)
+0x2241 clear: line 0 := zeros, `HideObj(+0x20ec)`. Set: `UnhideObj(+0x20ec)`, `RotateObj(0, pi/2, 0)`; line 0 = p0 (h = -((10/2 - 1) + 0.2) * 3.0 = -12.6, +0x2284, +0x2288) colour 0xff00, p1 (h = (10/2 + 1) * 3.0 = 18, +0x2284, +0x2288) colour 0xff00
(ground point x,y under the mouse, +0x2284/+0x2288). The mouse row +0x2260 is intersected with the projected pole (`RotatePoint` + `TransformPoint` on both ends, t = (z0 - mouseY)/(z0 - z1), `vonalmetszespont3d`) ->
h = (h0 - h1) * t2 + h1 -> +0x228c; the cell (`pos2racspos`) is clamped to [1, `SpWarAktZRange` - 2] and converted back (`racspos2pos`), so the plate snaps to a grid cell; `MoveObj(+0x20ec, h, x, y)`.

### 3.8 Explosions (visual part)
`felrobban` (0x69b610 / 0x5737e0) itself has no particles: it deletes the unit lightning effect (`delete unit.+0x270`, virtual slot 1; then +0x270/+0x278 := 0) and does the sim bookkeeping
(space-combat.md 7.5). The effects come from `urhajomeghalas` (§6.4): `ujparticlerobbanas` §6.7.1, the debris (`robbanasdarabok`, `Darabok`) §6.8.6, fighters' deaths §6.8.7.

### 3.9 Unload (`unloadgfx` 0x691f70)
If +0x24b8: `lovedekfreesound`; `sceneeltol(-starmapeltolas)` (the starmap objects move back), `starmapeltolas := 0`; remove and free the info model and its plate (`kisinfomeshid`/`kisinfolapmeshid`);
for the 10 icons `RemoveLight(+0x38)` and restore the saved material when +0x48 != 0; for every unit clear its id (+0x38) and `RemoveLight(+0x250)`; `FreeXVB(+0x1898)` (the vad10 model);
free each projectile form model once (duplicates nulled first); `SetLinePointer(NULL, 0)`; clear the 19x12 particle-set table (+0x18b8..); `restorestarmapparticles()` (empty); +0x24b8 := 0; `spacewarelfade = 0`.
The nodes added under `spwmainxvb` are not removed one by one here; `spwmainxvb` itself survives for the next battle.

## 4. Entering, leaving and the hot-spots
### 4.1 Enter / leave

#### 4.1.1 SpaceWar_Belep (ELF 0x67bad0 / exe 0x562680)
1. For i in 0..9: `fegyvernevek[i] = sprintf(GetString(0xb, 3, i + 1), 0)` and `felallasnevek[i] = sprintf(GetString(0xb, 3, i + 11), 0)`
   (indices from the exe; the ELF loses them). The static defaults in the exe image are "Lasers", "Guns" (the rest empty) and
   the formation names "formation 1".."formation 9", "Start battle" (exe 0x91d790); the group masks (name record +0x20) are zero in the exe
   file image but 0x1f, 0x1e0, 0x200, 0x400, 0x800, 0x1000, 0xe000, 0x30000, 0x40000, 0x80000 in the ELF (see section 7).
2. Screen locks: if no battle is running yet (`spacewarbanvanbarkacs` ELF global, exe 0xF176EE = 0) and `nemlehetidotallitani` (exe 0xBAD0D4) = 0:
   `backKepernyokLetiltva[0..19] = KepernyokLetiltva[0..19] & 1` (exe 0xE72E20 <- 0xF66F2C), then all 20 `KepernyokLetiltva = 1`,
   `nemlehetidotallitani = 1`. If a battle is running: all 20 = 1, `nemlehetidotallitani = 1`. (Screen switching is blocked during the battle.)
3. `TutorialWaitForBattle = 0` (exe 0xF66F48), `AktKepKell = 0` (exe 0xBAE250).
4. Pointer clamp: if `Mouse_Akt_x` (exe 0x916008) is not in 1..0x27b: `x = (int)((x - 320) * 0.98 + 320.0)` (f64), also `Mouse_x`;
   y: not in 1..0x1d9: `y = (int)((y - 240) * 0.98 + 240.0)`; then `Mouse_Move(x, y, true)`.
5. Redirect: unless `OldKep` (exe 0xBAE19C) is 2 (starmap), or 0xd (options) with a battle running: `starmaphovaleptovabb = 0xb; AktKep = 2; Atlepes = 1; return`.
   I.e. the battle is always entered from the starmap; any other previous screen bounces to the starmap, which zooms to the battle
   (the starmap then requests screen 0xb itself; not read).
6. Normal path (not editor, `spacewareditormode` exe 0xE72DD3 = 0): `AktSpaceWarp = spw + AktSpaceWar * stride`; `SpwC::initgfx` (ELF 0x6923a0 / exe 0x575050;
   guarded by +0x24b8, so re-entry from options does not rebuild; sets spacewarelfade = 0.001 (0x3a83126f), the HUD fade 0..1);
   `PlayMusicSpWar(true, false)` only if `!SelfPlaying && !spacewarbanvanbarkacs` (exe 0x540920); `srand(SDL_GetTicks())` (libc srand, not the sim streams);
   `InitSounds(SpWar_Hangok, 0x2a)` (42 records; no record has the load-at-init flag in GOG, so they load on first play); `LoadKepToMemory(0x1f)` +
   `GetMouseKepAddr(0x1f)`: kep 0x1f = `STAR_M.RAW` (the starmap control-cluster mask, 640x480 mhwanh: header 800 bytes, then 1 byte per pixel;
   it is not drawn here, only used as the hot-spot colour mask, see section 2); `spacewarbanvanbarkacs = 1`.
   If `ControlConfig[1]` (exe 0x90B521): MoveX/MoveY/Rotate `cValueInterpolator2::Setup` from ControlConfig floats (Rotate with wrap 2*pi = 6.2831855),
   `Reset` x4, `SetValue(Zoom, +0xd78)`, `SetValue(Rotate, +0xd70)`.
7. Editor path (`spacewareditormode`): `AktSpaceWar = 0; AktSpaceWarp = spw; SpwC::initeditor (ELF 0x6b85e0 / exe 0x574f50); initgfx; LoadKepToMemory(0x1f); GetMouseKepAddr(0x1f)`.
`SpaceWar_MouseInit` runs after Belep (screen kit order); `MouseInit2` is empty.

#### 4.1.2 SpaceWar_Kilep (ELF 0x67c090 / exe 0x562b50)
Two phases, because leaving first plays the zoom-out to the starmap:
- `if AktKep not in {0xd, 1, 10}: TutorialCheckEvent(0x20)`.
- Phase 1 (leaving flag `+0x2544` clear): `SpwC::Starmapvisszaallit` (ELF 0x6b4020 / exe 0x570c10): `rescalescene(0.0004)`, `+0x2544 = 1`, view animation of 0x50 frames
  to the saved starmap view (+0x2528..0x2540), `NezetVisszaAllitSet`, `AktKep = 0xb; Atlepes = 0; Belep = 0` (cancels the screen change; it stays on 11), screens locked,
  `infoelmegy = +0x2548` if outside 0..1. Then `starmaphovaleptovabb = requested AktKep` (0 if the request was the starmap 2), `nemlehetidotallitani = 1`, `SetIdoPause(true, false)`; return.
- When the zoom-out ends (`NezetVisszaAllitas`, factor >= 1 with +0x2544 set): `AktKepKell = starmaphovaleptovabb` (or 2 if 0), spacewarelfade = 0.001: the change is requested again and Kilep runs again.
- Phase 2 (+0x2544 set): `SpwC::unloadgfx` (ELF 0x691f70 / exe 0x582280). If the target is not 2/0xd: `starmaphovaleptovabb = AktKep; AktKep = 2; Atlepes = 1` (it always leaves via the starmap).
  If the target is 0xd (options): `KilepesElott2 = KilepesElott = 0; GFX_Starmap(); SaveScreen(2); OptionsHarcFolyik = 1` (exe 0xBAE24F; gate `PTR_DAT_0091a830[0x1fb8]` = the leaving flag +0x2544, which `Starmapvisszaallit` sets: so it is set only in phase 2, after the zoom-out). (The ELF also lists 10 and 1 in that test; after the redirect they can never hold, so same behaviour.)
  Battle not over (`+0x2546` clear, exe +0x1fba): `nemlehetidotallitani = 1; SetIdoPause(true, false)`; `DeInitSounds(SpWar_Hangok, 0x2a)`; return (the battle continues, e.g. after options).
  Battle over: SP -> `SpwC::SpaceWarVege` (ELF 0x6a1560 / exe 0x5706f0); MP -> `Multi_Send(0x44d, EN, 0)` (space-combat.md 7.13); `spacewarbanvanbarkacs = 0`;
  `KepernyokLetiltva[i] = backKepernyokLetiltva[i] & 1`; `nemlehetidotallitani = 0`; `DeInitSounds`.
- Towards Options a running battle sends nothing (the exe returns before `SpaceWarVege` / `Multi_Send 0x44d`), and the
  zoom-out runs five times faster (`visszaallitasidoszorzo = 5.0`, 800 ms). [obs, Esc in a running SP battle, UI ticks:
  t500 `+0x1fb8` = 1, `starmaphovaleptovabb` (0xf176e0) = 13, `nemlehetidotallitani` (0xbad0d4) = 1; t581 AktKep 13,
  HarcFolyik 1, `OptionsElottiKep` (0xbae2b4) = 11. Esc again at t1300: AktKep 2 at t1352, AktKepKell 11 at t1353, `Belep`
  (0xbae24c) held until t1454 (the starmap's Belep load and its catch-up ticks, input.md §1.1), AktKep 11 at t1455 with the
  leaving flag 0 and the intro flag `+0x1f98` = 1: the opening zoom plays again; the screen lock clears at t1535.]
- Other screens: F1..F11 pass in a battle only with `DEBUGX` (`DoKeyboard` 0x4db29c, the lock array bypassed); M is held
  off even then (`KepernyokLetiltva[10]`: the opening zoom sets all 20, its end restores only [0xd], the battle's end the
  rest); top-bar spot 7 needs `DEBUGX` and an unread message (§7.1.6). Each runs phase 1 (`TutorialCheckEvent(0x20)` unless
  the target is 0xd, 1 or 10; `Starmapvisszaallit`; `starmaphovaleptovabb = target`; `SetIdoPause(1, 0)`); the zoom's end
  asks for the target, phase 2 sends the 0x20 again, keeps the battle (`spacewarbanvanbarkacs` 0xf176ee stays 1, no
  `SpaceWarVege`) and enters the starmap with the target in 0xf176e0; there `FUN_00594a40` (from `Fake_FRISSITES`
  0x4de788, only on the starmap) sees the battle alive and asks for 0xb at once: the target is dropped and the battle
  re-entered with its opening zoom.
- Options over a kept battle: an F-key from Options (`DEBUGX` only, `OptionsHarcFolyik`) runs `Options_Kilep` (0x51a100)
  and enters the target; the battle stays until the next starmap frame; F11 resumes it at once (`SpaceWar_Belep`: no
  redirect when `OldKep == 0xd` and the battle alive). Help from Options takes `OptionsElottiKep` (0xb) as
  `HelpElottiScreen` (`Help_Belep` 0x4d7060), so Help's Back goes through the starmap to the battle. The end check is
  skipped while the leaving flag is set, so a battle can't end during the options zoom-out. The tutorial's O goes
  through the zoom-out like the battle's own O.
- MP: `SetIdoPause` (exe 0x541300) changes nothing in MP (with `+0xbae54c != 4` it clears the pause).

### 4.2 Hot-spots (SpaceWar_MouseInit, ELF 0x67c7d0 / exe 0x562d10)
Globals first: `MouseSzovegSzam = 0x2e` (exe 0xDB4680), `MouseHanyasIkonKep = 0x1f` (exe 0xDB4684). `colour` (arg 2, stored at +0x7c): 0 = the rectangle is the
hot-spot; non-zero = the pixel value of mask kep 0x1f that selects it (research.md section 4: `Mouse_Darabol` ELF 0x5b7a40; the lowest id wins).
`flag` (+0x74): `IG2_gameUpdate` (ELF 0x587400 / exe 0x4e0440) renders frames in a loop and returns to the screen (exe 0x562cb0 → `SpwC::main` per slot) only when: a button is down over a hot-spot whose flag is 0 (every frame of the hold), or whose flag is 1 and `Mouse_Valtozas` is set (the press edge, once), or a button came up (`Valtozas`) over the same flag-0 hot-spot as the last frame (the release, once). After a flag-1 press `Mouse_FelengedesMajdMegvar` keeps the loop rendering until the release and then returns `AktIkon = 0`. A hover alone never runs `SpwC::main` (the radar does not pan on hover). (Variables: `Mouse_Gomb` exe 0xdb4650, `Mouse_Valtozas` 0xdb4653, `MajdMegvar` 0xdb4669, `OldIkon` 0x912a40.)
All entries have type +0x78 = 1 (on). Strings via `GetString(0xb, 2, k)` (k from the exe immediates).

| id (AktIkon) | colour | rect x,y,w,h | flag | tooltip | start state | role |
|---|---|---|---|---|---|---|
| 1 (0x1f) | 0 | 274,388,94,67 | 0 | GetString(0xb,2,1) | on | radar: press/drag moves the look-at |
| 2 (0x20) | 0 | 4,24,151,63 | 1 | GetString(0xb,2,2) | off | unused |
| 3..0xc (0x21..0x2a) | 0 | position set per frame, 40x40 | 1 | fegyvernevek[0..9] | off; fegyverpanelfrissit switches each | weapon column buttons 0..9 (9 = Retreat) |
| 0xd (0x2b) | 0 | 0,0,0,0 | 0 | empty | off | none |
| 0xe (0x2c) | 0 | 0,0,80,32 | 1 | View ship | off | dead (panel B) |
| 0xf..0x17 | 0 | 0,0,0,0 | 1 | Normal, Agressive, Berserk, Use missiles, Defend/Attack, No target, Auto target, pr 1, pr 2 | off | dead (panel B) |
| 0x18 (0x36) | 0 | 0,0,167,220 | 0 | GetString(0xb,2,3) | off | dead (panel B body, drag) |
| 0x19 (0x37) | 0 | 50,461,113,9 | 0 | GetString(0xb,2,4) | off; infoszamol turns it ON while nothing is selected (+0x2574 == 0), OFF otherwise | Defend/Attack slider of the fighters |
| 0x1a..0x1d | 0 | 0,0,0,0 / 0,0,160,18 / 0,0,160,18 / 0,0,0,0 | 1 | Group size, Target type, Priority, Close | off | dead (panel A) |
| 0x1e (0x3c) | 0 | 0,0,176,107 | 0 | Fighter window | off | dead (panel A body) |
| 0x1f..0x23 | 0 | (4,309,74,19) (82,309,74,19) (4,331,74,19) (82,331,74,19) (4,357,99,99) | 1 | Fire range, Overview, Damage bars, Auto battle, Radar | off | dead |
| 0x24 (0x42) | 0 | 0,424,166,56 | 0 | GetString(0xb,2,5) | on | click = infomodeonoff (the small info box area) |
| 0x25, 0x26, 0x27 | 0 | 0,0,0,0 | 0, 0, 1 | Rotate 1, Rotate 2, Rotate 3 | on, zero size | never hit |
| 0x28 (0x46) | 0xf | 0,20,640,440 | 1 | GetString(0xb,2,6) | only if ControlConfig[0] (exe 0x90B520) == 0 | camera button: top-down view (inner left) |
| 0x29 (0x47) | 0x10 | 0,20,640,440 | 0 | GetString(0xb,2,7) | same | yaw left (outer left) |
| 0x2a (0x48) | 0x11 | 0,20,640,440 | 0 | GetString(0xb,2,8) | same | yaw right (outer right) |
| 0x2b (0x49) | 0x12 | 0,20,640,440 | 1 | GetString(0xb,2,9) | same | camera button: default angled view (inner right) |
| 0x2c (0x4a) | 0xb | 212,377,40,32 | 0 | GetString(0xb,2,10) | same | zoom (top button) |
| 0x2d (0x4b) | 0xc | 212,377,40,32 | 0 | GetString(0xb,2,11) | same | pitch (bottom button) |
| 0x2e (0x4c) | 0 | 0,0,640,480 | 0 | empty | on | the battlefield: selection and orders |

There is no per-hot-spot cursor: the cursor is global, `Mouse_Tipus` is set every frame from `MousePointerType` (section 3 item 1 and 4.2: 0 -> 1, 2 -> 5, 3 -> 7, 4 -> 6).
Then `Mouse_Szoveg_Off(id)` for id 2..0x23, so at the start only 1 and 0x24..0x2e are on.
Measured pixel ranges of the mask colours in STAR_M.RAW: 0xb x278..361 y366..386; 0xc x277..362 y452..473; 0xd x272..367 y387..452 (centre, unused here);
0xf x244..276 y376..461; 0x10 x218..271 y384..461; 0x11 x385..421 y384..453; 0x12 x361..395 y377..461. (The control cluster itself is a 3D model, KezeloId; the mask only gives the hit areas.)
Tooltip box and text are the shared ones: `GFX_DateMoneyScrollFade` (exe 0x554500) draws `FadeKerettelKerekit(Mouse_Kiiras_x + 15, Mouse_Kiiras_y - 1, len*6 + 10, 13, ...)` with
`FaderHanyadikMenet = 3` plus the text, only while `ControlConfig[0] == 0` and `AktIkonKiir > 0` (spy.md section 5, hud.md).

## 5. The 2D HUD and the draw list (`screenfrissites`, ELF 0x6a7780 / exe 0x57f800)
Everything is virtual 640x480. Most of the HUD is 3D meshes in the battle scene (weapon column, control cluster, ship model of the info box); the
2D parts are text, rectangles, lines and fader boxes. No picture is blitted from kep 0x1f (it is only the hit mask). The screen's own ids 2..5 are off; the shared top bar's pause and speed buttons are `AktIkon` 2..5 (§7.1.6).
Frame, only when `+0x24b8` (gfx up), in §3's order (steps 1–13). Its 2D parts: the cursor kind (`MousePointerType` exe 0xDE01B4 → `Mouse_Tipus` exe 0xDB9414: 0 → 1, 2 → 5, 3 → 7, 4 → 6); the info model's `kisinfotipus` (chosen by `selectkisinfourhajo`: max over the selected ships of `class*12 + type`, else 50000 + building id of the selected planet guns, else 5000 = `vad11.xvb`; added at 0.03, §3 step 1); the retreat window's box (`ShowRetreatWindow`, before the 3D pass) and its text (`WriteRetreatWindow`, last); the tooltip box (`GFX_DateMoneyScrollFade`, only `ControlConfig[0] == 0`); the height marker (§3.7, colour 0xff00 = green); the starmap layers inside `Update3D` that call `SpaceWarkisinfo/SpaceWarnagyinfo` (§5.2); then, after the first-frame block (§2.5):
12. Text lines (exe 0x57f800 lines 177–196: `KiirStr(.., 200, y, 0x46, 2, 6, 2)`: font size 2, len 70, flags 6, colour 2, right aligned inside 200..620), `KiirStrSzinSzorzo = (int)(spacewarelfade * 100)` for all (restored after):
    - `+0x3438` set: `sprintf(GetString(0xb,1,2), 0)` at y = 30 (own ships retreating); then y += 10 per line printed.
    - `+0x3439` set: `GetString(0xb,1,3)` (enemy ships retreating).
    - `+0x342d` (formation phase): `GetString(0xb,1,4)` with the colour multiplier `(int)(fade·100)·((slot-0 counter·100)/50)/100` (fades with the column) [obs: pixel-equal].
    - MP and `+0xf28 > 1` (more than one side) and `now - spwckezdotimer < 9000` ms: `"%s (%s) vs. %s (%s)"` = `Birodalom_Nev[side a]`, `PlayerNev[a]` (stride 0xb), same for b, passed through `a2az` (ELF 0x4f24c0; text conversion helper, exe address not found), at (200, 30), size 2, flags 4, colour 2; multiplier = `min(100, (9000 - elapsed) / 5)` percent of the fade. So it fades out over the first 9 s.
13. If `SPWRetreatWindow`: `WriteRetreatWindow` (section 3.4).

### 5.1 The weapon column (fegyverpanelfrissit ELF 0x6a7150 / exe 0x5727d0; SpaceWarFrissit2 for the counters)
Ten slots i = 0..9 (meshes: `ikon<i>.xvb` button at scale 0.02, `ora.xvb` reload clock at 0.014, `no.xvb` "off" overlay at 0.014; vector/spacewar/ and textures/spacewar/; created in initgfx at x = 300, y = 42*i - 200 (+15 for i = 9), z = -600).
Per tick (SpaceWarFrissit2, every UI tick: 50 ticks = 0.5 s; only when no intro/fade and `+0x2545` clear): target on = `!(+0x342e) && ( +0x342d || ( count[i] != 0 && +0x2588 != 0 && +0x2578 != 0 ) )`; slot 9 (Retreat): `!(+0x342e) && ( +0x342d || ( side[+0xd2c].+0x70 != 0 && StoryAdat+152 == 0 ) )` (the viewer's flag B, ELF side `+0x70`, set by `infoszamol`; not `may_retreat` `+0x68`);
forced off while leaving (`+0x2544`). Counter `c = slot+0x28`: +1 if on else -1, clamped 0..50 (50 ticks = 1.67 s). When c hits 0 while `+0x342e`: formation phase ends (`+0x342e = +0x342d = 0`), the icon materials are restored and the tooltips reset to `fegyvernevek[i]` (`Mouse_Csak_Szoveg_Set`, hot-spot id 3 + i).
Per frame: `f = (1 - gorbe(1 - c/50)) * 50`; `x = f*150/50 + 150` (150 closed -> 300 open), `y = 42*i - 200` (+15 for slot 9), `z = -200 - f*400/50` (-200 closed -> -600 open); all three meshes `MoveObj(x + 320 - xcenter*639/639, y, z)` (xcenter = 320).
Icon frame `SetFrame(+8 mesh, (maxreload[i]/10) * 1.65)` (the reload clock), mesh +8 shown with transparency 0. Not in formation mode: state[i] (0x3150): 3 -> overlay (+0x10) alpha 0.75 shown (some mounts of the group disabled); 6 -> alpha 0.5 shown (all disabled); 7 -> overlay hidden (all enabled); -1 (no weapon of the group) -> unchanged. `panelkiirsetszin(state)` stores it at +0x24a8.
c == 0: meshes hidden, hot-spot off (`Mouse_Szoveg_Off(3 + i)`); else button shown and hot-spot on, repositioned by `Mouse_Csak_Pos_Set(3 + i, (int)(x - 20 + 320), (int)(y - 20 + 250), -1, -1)` (size stays 40x40). Open column: x = 300 -> hot-spots at x = 600, y = 30 + 42*i (+15 for i = 9), i.e. a right-edge column.
Blink: while the fire cursor is armed (`MousePointerType == 4`) the armed group `+0x24c8` blinks: `spwvillogas % 50 < 26` shown, else hidden (button, and the overlay if its state showed one).
Slot 9 never shows the clock/overlay. Formation phase: all ten hot-spots on, overlay hidden, frame 0 for the chosen formation `+0x3430` (exe `+0x2d88`; the chosen button shows lit, also with nothing selected) else 32, button texture `final2x.bmp`; tooltip text = `felallasnevek[i]` every tick.
The clock `ora.xvb` is vertex-animated: `SetFrame((reload/10)·1.65)` (8.8 fixed point) blends the two vertex frames around it. The places take `320 − xcenter` back, so the column's fixed projection centre is `xcenter` (the big box's view animation moves it; the info model's stays at 320) [obs]. `Mouse_Csak_Pos_Set` ignores `xcenter`, so the hot-spots part from the drawn buttons while `xcenter` is not 320.

### 5.2 The info boxes (called from the shared starmap 2D layer)
`StarmapGfx2D_3` (ELF 0x6d4a30 / exe 0x5aaba0) runs while `AktKep == 0xb`: `SpaceWarnagyinfo(0x140, 0x22)` and `StarmapRovidInfoKiir(0x32, 0x1a8, 0)`, which calls `SpaceWarkisinfo(x, y, flags)` first and
draws the starmap short info under it only while `spacewarelfade < 1` (starmap multiplier `(1 - spacewarelfade) * 100`).
Both `StarmapGfx2D_1` (ELF 0x6dcb30) and `StarmapGfx2D_3` run while `AktKep == 0xb`: the planet and fleet names (`Show_Stars2d3d`, `Show_Flottak2d`) while `starmaphideflottak` is clear (e = `starmaperosseg` > 0), unfaded, under the console; the short info's text × `(int)((1 − fade)·100)/100` and its tanks/production buttons at `y + fade·400` while `spacewarelfade` < 1; `GFX_StarmapRadar` while fade < 1: dots and outline at `(int)((1 − fade)·255)`, the galaxy points × `fade·8 + 1`, the outline's corners the starmap's last (`lastfrom*` refresh only at fade 0, which the 0.001 clamp never reaches); `StarmapGfx2D_1`'s dark minimap window (`FadeScreen`) stays under the battle's radar [obs].
`infoszamol` (ELF 0x6a4080 / exe 0x574040) runs every 6th tick (`+0x258c` counter, §7.2) and fills the selection statistics, the weapon group arrays and the fighter slider sync (below).

**Small box `kisinfokiir(x = 50, y = 424, p = 0)`** (ELF 0x6b4220 / exe 0x577c00). Fade: `KiirStrSzinSzorzo = (int)(spacewarelfade * 100)` (restored). `KiirStrAlap(text, x, y, len = 0x13, size, flags = p + 0xc, colour)`; numeric columns use flags p + 0xe, len 4. Strings `GetString(0xb, 1, k)`.
Case A, something selected (`+0x2574 != 0`) and at least one ship (`+0x2578 != 0`, exe `0x1fec`): 
- title y+3, size 2, colour 3: one selected unit -> ship name `Tervek[owner][class]` (plan record +0x84*class, name first); if exactly one ship selected and its plan type is set: tech name `Talalmanyok[ship+0x10]` (record 0x114) at y+0xc, size 2, colour 0x21. More than one -> `sprintf(GetString(0xb,1,5), ships + guns)`.
- y+0x17 size 1 colour 0x21: owner `Birodalom_Nev[...]` (selected ship owner if kind 1; own empire for guns kind 2... exactly: kind 2 -> planet owner `+0xd7c`, kind 1 -> owner of the last selected ship, else 1).
- if selected ships != 0 and max hp (`+0x2564`) != 0: if own-selected count (`+0x2588`) == 0 the rank text is empty; else `avg = rank sum / selected` -> `GetString(0xb,1,6)` if avg < 4, `7` if < 0x10, `8` if < 0x1e, `9` if < 0x32, else `10`; negative -> "-". y+0x20: `sprintf(GetString(0xb,1,0xb), value(+0x2568), rank text)`; y+0x29: `sprintf(GetString(0xb,1,0xc), 0)` label and `DrawHpStripe(maxhp - damage, maxhp, x + 0x2c, y + 0x29, 0x42, (int)(spacewarelfade * 255))` (ELF 0x45f6a0 / exe 0x472020).
Case A also adds the selected guns (count, building damage `+4`, `EpuletekLeiro` hp, tech firepower to the value, exe 0x574040); a plan-less unit's value stays its design's (`+0xc`); the rank sum skips negative experience and the rank shows "-" when `(f32)sum / (f32)n < 0` [obs: "TE: 2860 -"].
Case B, only planet guns selected (exe `0x1fec == 0`, guns `+0x257c != 0`, kind 2): title = building tech name (single gun) or `sprintf(GetString(0xb,1,0xd), count)`; owner `+0xd7c`; y+0x20 `GetString(0xb,1,0xe)` with `+0x2568`; y+0x29 `GetString(0xb,1,0xf)` and the same HP stripe.
Case C, nothing selected (`+0x2574 == 0`): battle overview. Title y+3 `GetString(0xb,1,0x10)` (size 2, colour 3); row y+0xe: label `GetString(0xb,1,0x11)` + firepower of own (x+0x2a, colour 0x29), enemies (x+0x42, colour 0x2a), others (x+0x5a, colour 0x21, only if others have any ships/fighters/firepower) as `"%d%s"`
with unit divisor 1 (max <= 9999), 1000 ("K") if max < 0xf3e59 (exe test `> 0xf3e58` -> 1e6), else 1000000 ("M"); row y+0x17 label `GetString(0xb,1,0x12)` + ship counts (`KiirInt`, len 4, size 1); row y+0x20 label `GetString(0xb,1,0x13)` + fighter counts;
then the Defend/Attack slider `v = gfxvadaszdefendattack` (0..100): gradient bar `DDFillToScreen3dtRGB(x, y + 0x2a, 0x71, 3, (alpha << 24) | ((0x7f - v*0x7f/100) << 8) | (v*0x7f/100), alpha, 0)` (exe 0x4aba30), marker `DDFillToScreen3dt(x - 1 + v*0x71/100, y + 0x28, 3, 7, colour 2, alpha, 0)` and `(x + v*0x71/100, y + 0x29, 1, 5, colour 0, alpha, 0)` (exe 0x4ab640), label `GetString(0xb,1,0x14)` at y+0x29 (size 1, colour 0x21). alpha = `(int)(spacewarelfade * 255)`. So the slider rect is x 50..163, y 466..469, matching hot-spot 0x19 (50, 461, 113 x 9); the label is drawn after the bar and marker [obs].
The overview's firepower adds each unit's cached `+0x250` (exe; ELF `+0x284`; a plan-less unit's mounts' `+0x28`) and each fighter group's `+0x9c` (ELF `+0xac`), which only `AddUjVadasz` (exe 0x566420) writes, with 0: fighters never add firepower to the overview, though the side strengths `+0x54` count `Tervek[o][d] +0x2c × count` for them [obs: 3690 / 708 in the default battle].
Side counts (infoszamol): own = ships whose owner == `+0xd2c`; "enemy" = `side[owner]...[0xd98 + owner + me*0x13] == 1` (the relation matrix); others otherwise; fighters from the `0xb8`-stride array; firepower per ship `+0x284` (sum of mount firepower `+0x28` for plan-less ships, else the plan weapon values `Tervek[..]+0x2c` per hull slot), fighters `plan+0x2c * count`, guns `TalalmanyokrEro[tech]`.

**Big box `nagyinfokiir(0x140, 0x22)`** (ELF 0x6b5e50 / exe 0x57c9c0), only when `+0x2547 != 0` (big-info mode; opened by infomodeonoff/NezetVisszaAllitas). x0 = `(int)((1 - f) * 350 + 320)` with `f = +0x2548` (0 = off-screen right, 1 = centred at 320). Background box (`nagyinfofader`, ELF 0x6b26e0): `FadeKerettelKerekit((int)((1 - f) * 350 + 319), 0x21, 0x12e, 0x146, 0, 0xf, 0x14, 0x3c, 0, 10, 0xf, 0x1e, false, 0, 0, 0, 0)` (302 x 326 at y 33, translucent blue). `f` = `infoelmegy` if it is in 0..1 (it is -1 normally, set by Starmapvisszaallit; decremented 0.02 per tick while in 0..1, then 0), else the view-animation factor (inverted when `+0x229d`).
Text (font size 3, flags 0xc, labels len 0xb colour 1 at x0+10, values len 0x1a colour 0x21 at x0+0x62): rows only if the last selected unit is a live ship (kind 1): y = 0x22 + 10: `GetString(0xb,1,0x15)` / ship name; +0x1a: `0x16` / tech name `Talalmanyok[+0x10]`; +0x2a: `0x17` / owner; +0x3a: `0x18` / fleet name (flotta, via the ship fleet index); then rows start at y = 0x22 + 0x4a, else at 0x22 + 10:
`sprintf("%d%%, %dhp", hp*100/maxhp, ...)` with label `GetString(0xb,1,0x19)` (hull, from `+0x2564/+0x2560`; the hp shown is `(max - damage)/0xf`), then +0x10: `GetString(0xb,1,0x1a)` with `"%d%%"` of `100 - (+0x2558*100)/(+0x255c)` or "-" ; +0x10: `GetString(0xb,1,0x1b)` with `"%d"` of value `+0x2568`; +0x10: rank text (`GetString(0xb,1,6..10)` as above) with label `GetString(0xb,1,0x1c)`; then for a single ship with a plan: the weapon list, one row per plan weapon with count > 0 (max 10 rows, step 0x10): name `Talalmanyok[plan weapon id]` (len 0x1e, colour 1) at x0+10 and count `KiirInt(len 3, size 3, flags 0xe, colour 0x21)` at x0+0x104.

### 5.3 Radar
`RadarFrissit` (render callback of the null box +0x2164, so it draws inside `Update3D`; active when `+0x24b8`): §5.5.

### 5.4 Retreat window (exe 0x56d3f0 / 0x571270 / 0x5679b0; ELF 0x680fd0 / 0x681090 / 0x6814e0)
Opened by `SPWRetreatWindow = 1` (weapon button 9, section 4.3). Box: `FaderHanyadikMenet = 2; FadeKerettelKerekit(400, 0x17c, RetreatWindowXSize * 6, 0x2c, 0, 0x14, 0x1e, 0x3c, 0, 0xf, 0x19, 0x1e, false, 0, 0, 0, 0); FaderHanyadikMenet = 1` (400..400+6w, y 380..424; drawn in the 2D fader pass, under the text).
Text (`WriteRetreatWindow`, drawn last in screenfrissites): `s = GetString(0xb, 1, 1)`; `RetreatWindowXSize = strlen(s) + 2` (exe 0x926B40); hover `h`: 1 if `401 <= Mouse_x < 6*w + 400` and `0x18a <= Mouse_y <= 0x196`; 2 if x as before and `0x198 <= Mouse_y <= 0x1a4`; else 0.
- prompt `KiirStr(s, 400, 0x17f, w, 2, 5, 2)` (size 2, flags 5 = centred in w cells + 4, colour 2).
- "Yes" `GetString(3, 4, 2)` at `KiirStr(.., 3*w + 400, 0x18b, w, 3, 7, h == 1 ? 2 : 1)`; "No" `GetString(3, 4, 3)` at `(.., 3*w + 400, 0x199, w, 3, 7, h == 2 ? 2 : 1)` (size 3, flags 7, colour 2 hovered, 1 otherwise).
Click: each frame in which the left button is not down and the window is open, `HandleRetreatWindow`: if `h == 1` (the Yes row) and not `TutorialPaused` (exe 0xF66F2A): `Multi_Send(0x72, EN, 4, &AktSpaceWar, 0)` (Order::Retreat); then `SPWRetreatWindow = 0` regardless (any release closes it, also on "No" or outside).
No sound. The same 0x72 is sent by the debug key R (section 4.5).

### 5.5 `RadarFrissit` (ELF 0x685290 / exe 0x56c890): minimap
Radar rectangle on screen: **x 274..368 (94 px), y 388..455 (67 px)**, centre (321, 421) (bottom, left of centre, of the 640x480 screen). The arena (SpwC +0x900/+0x904 and +0x908/+0x90c = -192..192, width 384) maps to 94 px in x and 67 px in y for dots (94 px in both for the cone).
1. `k = gorbe(spwradarcounter/100)` (exe global 0xe60438 int, ramps 0..100 while entering, back while leaving; `gorbe(t)`: t > 1 -> 1; t >= 0.5 -> `1 - 2(1-t)^2`; else `2t^2`), `yaw = SpwC+0xd6c` (exe +0xc7c), `sn = (f32)sin((f64)yaw)`, `cs = (f32)cos`.
2. **View cone**: for the four screen corners (0,0), (640,0), (0,480), (640,480): `SikMetszespont(corner, planeH = +0x928*s + +0x24f0)` = intersection of the view ray through that pixel with the grid plane
   (ray from the inverse of `cmatrix`; the h-direction is clamped to at most -0.01) -> world (h, x, y). Radar point `u = (cs*(-x) + sn*y)*94/384 + 321`, `v = (cs*y - sn*(-x))*94/384 + 421`.
   If `SpwC+0x2544` (leaving) is clear the 4 points are stored into `lastcs` (4 x 16 B), else they are read back (frozen cone while leaving).
   Then every point (absolute coordinates, offsets included) is multiplied by `(1-k)*6 + 1` (the outline flies in from outside while the radar opens).
   Clip rect set to `HatarX1..X2 = 0x112..0x16f`, `Y1..Y2 = 0x184..0x1c6` (274..367, 388..454), `Xm 0x5e, Ym 0x43`; four `LineClip(x0,y0,x1,y1, 2.0 (cast to int = DDColor index 2), alpha 0xff)` in the order
   (0,0)->(640,0), (640,0)->(640,480), (640,480)->(0,480), (0,480)->(0,0), i.e. the screen outline projected on the grid plane. `LineClip` clips to the rect and draws 1 px lines via `TscVonal` + `line2d(.., false, true)`;
   colour = `DDColor[2] * alpha/256` per byte.
3. Clip rect reset to the full 640x480 plus the widescreen offset `d = (IG2_WINDOW_WIDTH*480/IG2_WINDOW_HEIGHT - 640)/2`.
4. **Backdrop**: `line3` with one vertex pair (274,388) - (368,455), z 0.5, colour **0x40000020**, no texture, no depth test. [obs: the exe's window is (4, 9, 12) over the starmap's black `FadeScreen` (§5.2): the 0x20 is blue and the 0x40 its opacity (25 %); the cone's `line2d` lines are full white over it: the immediate 2D line goes over the queued `line3`.]
5. **Dots**: `k2 = (1-k)*9 + 1`; every live unit with `owner == EN` (human empire global) or **not cloaked** (+0x1e3): `a = -cs*x + sn*y`, `b = cs*y - (-sn)*x` (x = +0x4c, y = +0x50),
   `X = a*94*k2/384 + 321`, `Y = b*67*k2/384 + 421`; kept when `274 < X < 368` and `388 < Y < 455`. Colour: owner == +0xd2c (own) **0x8000FF00** (green), else **0x800000FF** (red).
   `pixel(buf, n, depth=false, additive=true)`: 1 px points at `((int)X, (int)Y)` (truncation), alpha forced 0xff. The buffer is the stack array that also held the backdrop (40 B per vertex, about 119 vertices).
6. **Outro fade**: when `SpwC+0x24bc` is nonzero and `> 0x2134 (8500)` (set to 1 by `vegevizsgal`, ELF 0x6a0d50, at the end of the battle together with `+0x24c0 = SDL_GetTicks()`; the writer that advances it was not found):
   `a = ((0x5dc - (v - 0x2134)) * 0xff) / 0x5dc` (int), at least 1; two triangles over (0,0)-(640,480), colour `a<<24 | 0x010101`, z 0.5, `polygonsfader(.., 2, 0)` (normal blend, transparency 255-a, texture `fadertexture`):
   a fade to black that is almost opaque 1500 units after 8500. Drawn after the dots.

## 6. Overlays and effects
### 6.1 `lovedekekkirak` (ELF 0x6823c0)
Per call (once per rendered frame, inside the 3D pass):
1. `c = 1 - starmaperosseg` (global exe 0x927994). Colour words (A = transparency, see 0): `Ab = (int)(255 - c*127)`, `Ar = (int)(255 - c*191)`:
   - `COL_ATK  = Ab<<24 | 0x0000ff` (pure red)
   - `COL_DMG  = Ab<<24 | 0x4040ff` (light red)   `COL_OK = Ab<<24 | 0x40ff40` (light green)   `local_1c = Ab<<24|0x40ffff` (yellow, never used)
   - `COL_BR_FOE = Ar<<24 | 0x4040ff`   `COL_BR_OWN = Ar<<24 | 0xffffff`
   Effective additive intensity = `(255-A)/256`: bars/ticks ~ 0.5*c, brackets ~ 0.75*c: everything fades out when the starmap takes over (c -> 0).
2. `KiirStrSzinSzorzo = (int)(spacewarelfade*100)` (saved first, restored at the very end; text alpha follows the fade).
3. **Pass A, planet guns** (`i < +0xd80`, base +0xd88, stride 0xb0; skip when `+0x84 == 0`). pos = (gun.+0 .. +8) * s; `RotateAndTransformPoint` -> (X, Y, Z view-space); draw only when `Z < 0`.
   Base half-size `r = 20` px (constant, no TransformTav). T1 = +0x8c, F = +0x90 (fade), T3 = +0x94.
   Condition: `(T1 != 0 && F < 250) || T3 > 0`, else nothing for this gun.
   - grow `g`, fade `f`: if `T1 != 0 && F < 250`: `g = min(1, T1/250)`, `f = (250-F)/250`; else `g = min(1, T3/250)`, `f = 1`.
   - ratio `q = building.damage(+4) / EpuletekLeiro[type].+8` (building = `Epuletek[planet(+0x24b4)] + slot*0x20`, type = its byte 0): damaged fraction.
   - **HP bar** (2 quads; each vertex pair (v0,v1) is one screen-aligned rectangle in `line3`): top `yt = (double)Y - (double)(r*f)*1.2 + 0.0 - 1.0` (f64 math, cast to f32; same in the exe),
     bottom `yb = (double)Y - ((double)(r*f)*1.2 - (double)(r*f)/5) - 1.0`: the bar is `r*f/5` px high, sitting `1.2*r*f + 1` px above the centre.
     Quad 1: x from `X - r*g` to `X - r*g + (q*r*2)*g`, u 0 -> q, colour COL_DMG. Quad 2: x from there to `X + r*g`, u q -> 1, colour COL_OK. All tv = 0. (f32 math in x.)
   - **Corner brackets** only when `T1 != 0 && F < 250`: `cb = (planet owner (+0xd7c) == own empire (+0xd2c)) ? COL_BR_OWN : COL_BR_FOE`;
     `r2 = 9000/T1 - 9 + r` (T1 = 1000 -> r; large when T1 is small: the brackets close in over 1 s), arm `la = r2*(250-F)/500` (= r2/2 while selected, 0 at F = 250).
     8 one-pixel-thick quads, all u = 0.5, in this order (first vertex -> second vertex offset): (X-r2,Y-r2)->(+la,-1) tv1; (X-r2,Y-r2)->(-1,+la) tv0; (X+r2,Y+r2)->(-la,+1) tv1; (X+r2,Y+r2)->(+1,-la) tv0;
     (X+r2,Y-r2)->(-la,-1) tv1; (X+r2,Y-r2)->(+1,+la) tv0; (X-r2,Y+r2)->(+la,+1) tv1; (X-r2,Y+r2)->(-1,-la) tv0. (tv = 1.0 on the first vertex makes line3 rotate the UV, see 2.1.)
   - **Attack ticks** when `T3 > 0` (written after the brackets, so an unselected attacked gun gets bar + ticks): `r3 = 9000/T3 + 20 - 9`, `lt = r3*250/500`, colour COL_ATK, u 0.5:
     4 quads: (X+r3, Y)->(+lt,+1) tv1; (X-r3, Y)->(-lt,+1) tv1; (X, Y+r3)->(+1,+lt) tv0; (X, Y-r3)->(+1,-lt) tv0: four ticks on the axis points pointing outward.
   Then `line3(buf +0x21b0, texture +0x1890 szar.bmp, nquads, depth_test = false, additive = true)`.
4. **Pass B, units** (`i < +0xd18`, base +0xd08, stride 0x290; skip when not alive (+0 bit 0)):
   `GetObjCenter(meshid +0x38, &C, &R)` (C = the node centre in view space, R = mesh radius), draw only when `C.z < 0`. T1 = +0x18c, F = +0x190, T3 = +0x194.
   Condition as in pass A. `TransformPoint(C) -> (X,Y)`; `r = TransformTav(type(+0x230).+0x10 * 1000 * s, C.z) = r_world*fov/-z` px.
   - `g`, `f` as in pass A.
   - ratio `q = damage(+0x18)/hp(+0x1c)` (f32 of the two ints; Rust `damage`/`hp`: hurt fraction, no zero guard). `W = r*q*2`.
   - HP bar: same geometry as the gun bar with this `r`; quad 1 [x0, x0+W*g] u 0..q COL_DMG, quad 2 [x0+W*g, X+r*g] u q..1 COL_OK, x0 = X - r*g.
   - **Shield overlay**: if `shield_used(+0x20) < shield(+0x24)`: `cc = used/cap`; a 3rd quad on the same rows from `x0 + (r*cc*2)*g` to `X + r*g`, u cc..1, colour COL_DMG
     (additive on top of the green half). (`local_c4 = (int)(-r/3)` afterwards: set but never used again.)
   - Brackets (only when `T1 != 0 && F < 250`): `cb = (unit.owner(+0x14) == +0xd2c) ? COL_BR_OWN : COL_BR_FOE`; `r2 = 9000/T1 - 9 + r`; `la = r2*(250-F)/500`; the same 8 quads around (X,Y).
   - Attack ticks (`T3 > 0`): `TransformPoint(C)` again, `r` recomputed, `r3 = 9000/T3 - 9 + r`, `lt = r3*250/500`, 4 ticks as in pass A.
   - Labels (text drawn immediately, not through line3), using the last value of `local_54` (rr = r3 if the tick block ran, else r2 if the bracket block ran, else r;
     X, Y are stale from the previous unit when no block ran this frame, possible only on the first frame of a selection):
     * selected (+0x189) and `ship_count(+0x1bc) > 1`: `KiirInt(count, (int)(X + rr - 5), (int)(Y + rr - 7), 5, 2, 0xc, 3)` (bottom-right of the bracket; the `*639/639`, `*479/479` are identity);
     * selected and editor mode: `KiirInt(unit_index, same place, 3, 2, 0xc, 3)` (printed over the count);
     * selected and jammed (+0x1e1): `KiirStr(GetString(0xb,1), (int)(X - rr + 1), (int)(Y + rr + 2), 10, 2, 0xc, 6)` (bottom-left).
   Then `line3(..., texture szar.bmp, n, depth_test = true, additive = true)` (units are depth tested with `z = 1 - zbm/-Z`): billboards at the unit's eye depth (a virtual pixel = `−Z/600` eye units along the camera's rows), additive, no depth write; pass A is not depth tested.
5. `villamfrissit()` (ELF 0x6821c0 / exe 0x583830): lightning. For every shot of **form 21** (ELF `fegyveralakok` + 21*0x90; pool count `_DAT_011973e4`, shot array `_DAT_01197458`, 0xd8 B per shot)
   with shooter (+0x18) nonzero and kind (+0x28) == 0 (ship at ship): call virtual slot 3 (+0x18; exe slot 2, +8) of each of its 7 objects at `shot+0xa0+8k`; for every live unit with an arc object at
   +0x270 (exe +0x244) and every live fighter group with +0x98 (exe +0x90) the same single call. These are `villamgenesis` beam objects (jittered poly-lines, exe Init 0x616190, jitter 0x616620,
   texture `loadvillamtexture` exe 0x616580). The draw body was NOT read.
6. `ElementList::Update(SpwC+0x3440)` (HUD element list; each virtual Update, linked via +4 in the exe).
7. **Order arrows** if `spacewareditormode` or `SpwC+0x342d` (exe +0x2d85) is set: for every live unit with owner == own empire (+0xd2c): `p = unit.pos(+0x48..)*s`; for each queued waypoint k < +0x30
   (cells at +0x78 + 16k): `w = racspos2pos(cell)`; if `tavnegyzet(unit.pos, w) > 0.01` (f32 sum of squares, compared as f64 0.01): `DrawArrow(p, w*s, 0x80808080, 1.0, mode 2)`; then `p = w` (even when not drawn;
   the comparison always uses the unit position, not the previous waypoint). Colour 0x80808080 confirmed in the asm (ELF 0x684f79).
8. Restore `KiirStrSzinSzorzo`.

#### 6.1.1 `line3(v, tex, n, depth, additive)` (ELF 0x5efb60)
Input: n pairs of `D3DTLVERTEX` (0x28 B: x, y, z(view), rhw, colour, spec, tu, tv). Each pair becomes one quad with corners
`q0 = (x1, y0), q1 = (x1, y1), q2 = (x0, y0), q3 = (x0, y1)` (x0,y0 from v0; x1,y1 from v1), depth `1 - zbm/-z` (z of each vertex), colours q0,q1 = v0.colour, q2,q3 = v1.colour.
UV, normal: q0 (v1.tu, 0), q1 (v1.tu, 1), q2 (v0.tu, 0), q3 (v0.tu, 1) (u along x, v along y). If `(int)v0.tv` is nonzero (tv >= 1): q0 (v0.tu,0), q1 (v1.tu,0), q2 (v0.tu,1), q3 (v1.tu,1) (u along y, v along x).
Triangles (0,1,2), (1,2,3). Fog off, Z write off, Z test = `depth`, blend as in section 0, texture = `tex` (or the null texture).

#### 6.1.2 `DrawArrow(p0, p1, colour, width=1.0, mode=2)` (ELF 0x6e2930)
`smRotatePoint` both ends (starmap-space view transform; not read). Mode 2 and `starmaperosseg > 0`: rgb scaled by `(1 - starmaperosseg)` (int trunc per byte; A kept). If either end has z >= 0 the segment is clipped with `clip()` (z of the clipped end -= 2), both behind = nothing; then z -= 0.0001.
Screen: `d = normalise(p1s - p0s)`, `n = (-d.y, d.x)*width` (sqrt in f64). Shaft (texture `nyilszar`, 2 triangles): quad (p0s -+ n) to (p1s -+ n - 10*d); head (texture `nyilhegy`, 1 triangle): tip p1s, base `p1s - 10*d +- 2.5*n`. Both coloured `colour`.

### 6.2 `agyukirak` (ELF 0x6818c0 / exe 0x571480): planet-gun markers (callback of the outer planet shell)
For every gun with `+0x84` nonzero (same list as 2/A): `RotateAndTransformPoint(pos*s)` -> (X,Y,Z); skip unless `gun.+0x10` nonzero and `Z < 0`.
`T = gun.+0x10` (the marker timer, see 0): `r = (9000/T + 2) - 9`.
Colour `Ar<<24 | (planet owner (+0xd7c) == own (+0xd2c) ? 0xffffff : 0x0000ff)`, `Ar = (int)(255 - (1-starmaperosseg)*191)` (white own, **red** otherwise).
- armour `gun.+0xa8` (exe +0xa0) `<= 1.0`: 8 untextured quads (texture NULL), 4 corner brackets with 10 px arms, 1 px thick:
  (X+r, Y-r)->(+10,+1); (X+r,Y-r)->(-1,-10); (X+r,Y+r)->(+10,-1); (X+r,Y+r)->(-1,+10); (X-r,Y+r)->(-10,-1); (X-r,Y+r)->(+1,+10); (X-r,Y-r)->(-10,+1); (X-r,Y-r)->(+1,-10).
- armour above 1.0: `kor(&(X,Y,Z), 10.0, 12 segments, colour)`: a 12-gon circle of radius 10 px through `line3d3` (instead of the brackets).
End: `line3(buf, NULL, n, depth=false, additive=true)`. T = 1000 for the human guns: `r = 2`, a tight square with 10 px arms around every own gun, permanently.

### 6.3 Projectiles
#### 6.3.1 `SpaceWarLovedekKirajzol(form, formIndex)` (ELF 0x69ebc0 / exe 0x56e1c0): per form, per shot, every tick (from the sim tick: it is the sprite writer, not a renderer)
Form record (ELF stride 0x90 at 0x1196800): +0x10 sprite size, +0x14 pool, +0x20 alpha a0, +0x24 anim period, +0x28 anim step, +0x2c kind, +0x40 particle array (0x4c B each), +0x88 shots (0xd8 B each).
Shot (ELF): +0 weapon id, +0x10 age (ticks), +0x18 shooter ptr (0 = free), +0x20 target ptr, +0x28 shot kind (0 ship at ship, 3 = unit at gun, ...), +0x38 pos (3 f32), +0x58 dir (16 B), +0x71 attached (beams), +0x74 tick it attached, +0x78 weapon record ptr
(weapon record: +0x4c duration T, +0x54 form index, +0x5a bit 0 = beam). Free shot (shooter 0): `particle.+0x14` (size) = 0, hidden.
Particle (0x4c B): +0 pos (x s), +0xc colour word, +0x10 base size, +0x14 current size, +0x18 dir (16 B copied from shot +0x58), +0x2c frame.
Steps for a live shot:
1. pos: `shot.pos(+0x38) * s`; for form kind 8 and shot kind 0 first `pos = shooter.pos*s`, `dir = normalise(shooter.pos - target.pos)` (then overwritten by the copy of shot +0x58 in step 3, so effectively unused).
2. `particle.+0x10 = form.size * s`.
3. dir copy; `T = weapon.duration`, `T *= 10` if shot kind == 3; `age = shot.+0x10`; `a0 = fegyveralakok[weapon.form].alpha` (the form of the weapon record).
4. Colour (A = transparency) and size. Non-beam weapons (`weapon.+0x5a` bit 0 clear):
   - weapon 12: `+0x14 = 0` (no sprite; the arc is drawn by villam), `A = (int)(a0 + (255-a0)*((age*255.0)/T)/255)`, word `A<<24 | 0xffffff` (f32 math).
   - otherwise `A = a0` while `age < T/2` (int div), else `A = a0 + ((255-a0) * ((age - T/2)*0x1ff / T)) / 255` (integer math), word `A<<24|0xffffff`; `+0x14 = form.size*s`.
   Beam weapons (bit 0 set): unless weapon 8 or 11: `A = a0` while `age < T*9/10`, else `a0 + ((255-a0) * ((age - T*9/10)*0x9f6 / T))/255` (ramp over the last 10 percent).
   - not attached (+0x71 clear): `+0x14 = form.size*s`.
   - attached and weapon 11: `d = age - (tick(+0xd3c) - attachTick(+0x74))`, `x = ((age - d) < (T - d)/2) ? (age - d) : (T - d) - (age - d)`, `x /= 20.0`, `+0x14 = (80 - (80 - form.size)/(x+1)) * s` (pulses up to 80).
   - attached, other: `R = target.type(+0x230).+0x10 * 1200`, `+0x14 = (R - (R - form.size) * (0.1/(((tick - attachTick)/20.0) + 0.1))) * s` (glow shrinking from the target size to the form size).
5. Frame `+0x2c`: if `form.+0x24 == 0` -> -1.0, else `+= form.+0x28`, set to 0 when it exceeds `form.+0x24`.
6. Trail/puff spawn by **form kind** (jump table ELF 0xd589c0; exe 0x56e874; kinds 1..8; every case ends at the common tail). The spawners append particles to the effect arrays on the +0x20d4 node
   (set 3 = `star.bmp`, set 4 = `texcsor.bmp`; ring cursors +0x169c / +0x16a0, pool sizes 0x1199bac / 0x1199bb0), position = the shot pos, all libc `rand()` (cosmetic):
   | kind | spawner (ELF) | set | count / params | particle |
   |---|---|---|---|---|
   | 1 | particlerobbanas 0x688cc0 | 3 | args (5, 0.04, 0.3, 1.0): `(int)(5*0.3) = 1` puff per shot per call (a second count `(int)(5*1.0)` is derived; its loop was not read) | colour `rand(64)<<16 | 0x50c0` (R 0xc0, G 0x50, B 0..63: orange), size `(1 + rand(200)/20) * 0.04 * s`, velocity = normalise(3 x (rand(1000)-500)) * `(rand(100)/100 + 1.5) * 0.04 * s`, frame 0, speed 1.0, `+0x34 = 0.88`, `+0x38 = 0.99` |
   | 2 | particlerobbanas2 0x687a40 | 4 | n = 3, param 0.4 | colour 0x00ffffff, size `(1 + rand(100)/20) * 0.4 * s`, vel = normalise(3 x (rand(1000)-500)) * `(rand(100)/100 + 0.1) * 0.4 * s`, frame 0, speed 1.0, `+0x34 = 0.94`, `+0x38 = 0.96`, `+0x3c = 0.01 - (rand(100)-50)/10000` |
   | 3 | particlerobbanas3 0x687cf0 | 4 | n = 5, param 0.05 | as 2 but speed `(rand(1000)/100 + 0.1) * 0.05 * s`, `+0x34 = 0.99`, `+0x38 = 0.96` |
   | 4 | particlerobbanas4 0x687fa0 | 4 | n = 5, param 0.1 | as 2 but position = shot pos*s + the velocity vector, speed `(rand(1000)/600 + 0.1) * 0.1 * s`, `+0x34 = 0.99`, `+0x38 = 0.89` |
   | 5 | particlerobbanas5 0x688270 | 3 | n = 1, size 0.3 | colour `0x005cff5c` (green), size `5.0*0.3*s`, vel 0, `+0x34 = 0`, `+0x38 = 1.0`, `+0x3c = 0.05 - (rand(100)-50)/1000` |
   | 6 | particlerobbanas6 0x688450 | 3 | only when the shot is not attached: size `0.5 * d/(d + d/5) = 0.41667` (constant), alpha arg 255 | colour from the arg: `0xff000000 | ((a<<4)>>8)<<16 | ((a<<4)>>8)<<8 | ((a<<5)>>8)` = 0xff0f0f1f for 255 (near black, A 0xff: probably invisible) |
   | 7 | particlerobbanas6 | 3 | size `form.size * ((age*255/T)/36.0)`, arg `255 - A_ramp` where `A_ramp = (int)(a0 + (255-a0)*((age*255)/T)/255)` | a growing sprite whose colour argument falls as the shot ages |
   | 8 | none (lightning: villamfrissit) | | | |
   Then, for every shot regardless of kind: if form index == 7 (torpedo), `dir.xy = normalise(sin(age/3), cos(age/3))` (spinning sprite); if weapon id == 11, `dir.xy = normalise(sin(age/10), cos(age/10))` (f64 sin/cos, f32 store).
   Kind 0 forms (lov*) get only the sprite (steps 1-5).
Particle fields `+0x30` (= 1.0), `+0x34`, `+0x38`, `+0x3c` are per-tick physics factors of the effect-particle update (velocity damping, size factor, ...); that update loop is in the engine, not read.

#### 6.3.2 Engine trails `Hajtomu` (ELF 0x69a9c0 / exe 0x567140)
For each live unit with `+0x1e3` (cloaked) clear: record `H = hajtomuvek + type(+8)*0x3c + race(+4)*0x2d0` (ints), type record `tipusnevek + type*0x30 + race*0x240` with scale `S = rec.+0x10`;
node matrix `GetObjMatrix(+0x38)`, `P = GetObjPos(+0x38) * 1024`. Four thruster offsets `th_k = (H[3k+2], -H[3k+3], H[3k+1]) * S * 1000` (k = 0..3), rotated by the node matrix and added to P;
puff direction `D = rotatepointmatrix((0, -0.1, 0), matrix)`, puff size parameter `S*4`. Trail kind `H[0]`: 1 -> thrusters 0,1 (2 puffs); 2 -> 0..3 (4); 3 -> 0..3 plus their mirror (first component x -1) (8); 4 -> 0..2 (3); 5 -> 0 (1); other -> none.
Each puff = `particlerobbanas7(n=1, size S*4, pos, D)` (ELF 0x688630, set 3 = star.bmp): `pos = given + D * rand(100) * (S*4*s)` per component (3 draws of rand(100)); size `(1 + (rand(20)+80)/2) * (S*4) * s` (about 41..50 x the parameter);
colour 0x00ffffff; velocity = D; frame 0, `+0x30` = 1.0, `+0x34 = 0.99`, `+0x38 = 0.6`, rest 0.

#### 6.3.3 Fighters `SpaceVadaszKirajzol` (ELF 0x694e50): see §3.2, confirmed
Group record (ELF stride 0xb8): +0 alive, +4 pos (h,x,y), +0x48 dir (16 B), +0xb0 set class (0..3), +0xb4 particle index. Alive and `j != aktvadasz`: `p.pos = pos*s`, `p.+0x14 = p.+0x10 * s`, `p.+0x18 = dir`, `p.+0x28 = 0`; else `p.+0x14 = 0`.
Sprite model `vad10.xvb`, four PARTICLESTYPEs (+0x1740..) on nodes +0x2144 / +0x214c / +0x2154 / +0x215c.

### 6.4 Explosions `urhajomeghalas` (ELF 0x6a1690 / exe 0x582550)
Matches §1–§3 3.8 with these additions: (a) first, for every live **scenery** unit (owner 0, not editor): `spin(+0x14c) += (fract((i*1.23)/0.015)*0.015 + 0.005)` (f64 math, f32 store), then `seturhajopos(i)`;
(b) for a live unit with the **retreating** flag (+0x1e0): `+0x1d8++`; if `+0x1dc` (explodes) and speed(+0x10c) > 3 -> `felrobban(unit, 0, 0, true, true)`; if `tav(pos, retreat point +0x24f0) >= +0x918 (384)` -> `felrobban(.., false, false)`;
(c) the vector q = `InvertMatrix(cmatrix)` applied to `(0,0,S*400)` is computed for every unit on every call; (d) a dying unit (alive clear, `+0x184 > 0`): `+0x184++`, then
- not retreating: c < 10: if `rand(100) < 40` -> p = pos + (rand(100)-50)/100 + q (per component), `ujparticlerobbanas(0, S*0.05*700, p)`, sound `SpWar_Hangok[8 + rand(6)]` with its volume field set to 0x37 (record stride 0x11c), 2 x `robbanasdarabok(p, S*r/25, 1.5/r)`, r = rand(100)/100*5+1;
  c in 16..17: offset /80, size `S*0.1*700`, debris `(S*r/18, 3.0/r)`, same sound; c == 20 (and gfx loaded): sound `SpWar_Hangok[rand(3)]`; 26 <= c <= 45: 40 percent: q*2, offset /50, an unused `random(2)` then set = rand(2), size `S*0.05*700`, debris `(S*r/18, 3.0/r)`; c > 45: c = 0.
- retreating: c < 2: `ujparticlerobbanas(0, S*0.4*700, pos)` + sound; 2 <= c < 20: `ujparticlerobbanas(0, (1/(c+5)) * S*700, pos)` + sound each tick.
`ujparticlerobbanas` and the debris as in §1–§3 3.8 (effect sets: 0 animated.bmp, 1 anim2.bmp, 2 smoke.bmp, 3 star.bmp, 4 texcsor.bmp).

### 6.5 `seturhajopos` drawing pose (ELF 0x686aa0 / exe 0x580540)
- Visibility: dead unit: `HideObj` unless gfx loaded, `+0x184` (exe `+0x17c`) nonzero, `+0x184 <= 0x29` and not retreating (then `UnhideObj`; the hull stays beside its blasts [obs]). Live: not cloaked -> `SetObjTransparency(0.01)` + show; cloaked and own -> 0.6 + show; cloaked foreign -> hide. Editor: transparency 0.5 and `ScaleObja(type scale from DAT_011907e0[type*0x30])`.
- `MoveObj(node, pos(+0x48..+0x50))`. Owner 0 (asteroid): `RotateObj(node, spin.x(+0x14c), spin.y(+0x150) + 3.1415927, spin.z(+0x154))`.
- Ships (owner nonzero): `bank e = speed(+0x10c) * turn(+0x168) * 20 * 2.5`. `M1 = CreateMatrix2v(heading(+0x114..), v2 = (-m[0][2], -m[1][2], -m[2][2]) i.e. the previous matrix elements +0xf0, +0xfc, +0x108 sign-flipped, 0.0)`,
  `M2 = CreateMatrix(+0x25c + pi/2, +0x260 + pi/2, +0x264 + 0)` (pi/2 = 1.5707964f), `matrix(+0xe8..) = MultipleMatrix(M2, M1)`.
  If `|m(+0xe8)| >= 0.01 || |e| >= 0.01` and `type.turn(+0x24)` is nonzero: target `Mt = MultipleMatrix(CreateMatrix(pi/2, pi/2, 0), CreateMatrix2v(heading, (1,0,0), 0))`; if `|e| >= 0.01`:
  `axis = normalise(cross(midHeading(+0x124), (+0xf0, +0xfc, +0x108)))`, `Mt = Mt * creatematrixfromaxis(axis, -e)`. Compare column 0 of `Mt` (elements 0, 3, 6) with column 0 of the current matrix (+0xe8, +0xf4, +0x100):
  angle `a = vektorszog(Mt.col0, cur.col0)`; if `|a| > 0.01`: axis = normalise(cross(Mt.col0, cur.col0)) (x axis if null), `step = min(a, min(0.05, a/6))`, `cur = cur * creatematrixfromaxis(axis, -step)`.
  Then `SetObjMatrix(node, +0xe8)`. The bank builds up over frames because the next frame up vector is the previous matrix third column.
- Seed: `AddUjUrhajo` (ELF 0x68a730) sets `+0xe8` (exe `+0xe4`) to `CreateMatrix(spin.x, spin.y + π, spin.z)`, whose third column is the first up vector [obs: the entry dumps' matrices all have third column (−1, 0, 0)]. `normalizevektor` (ELF 0x4522d0) leaves vectors under 1e-5 as they are, so an up vector parallel to the heading makes `CreateMatrix2v`'s cross products vanish and the matrix stays rank 1 (the hull collapses to a line).
- (The heading easing is in space-combat.md 7.2 / §1–§3 3.1.)

### 6.6 Small ones
- `frissit2` (ELF 0x694c50 / exe 0x573b40; run from `SpaceWarFrissit` exe 0x56dff0, the sim tick), when gfx is loaded: `karikafrissit(tick)`; per live unit by type: 8 -> `+0x260 += 0.007`, 9 -> `+= 0.005`, 10 -> `+= 0.01` (the asteroid third rotation angle; f32 += double constant, f32 store);
  planet battle (+0xd7c >= 0): `RotateObj(shell1 +0x18a0, tick/900.0f, 0, 0)`, `RotateObj(shell2 +0x18a8, -tick/1200.0f, 1.5707964, 0)` (exe 0x573bff / 0x573c36, f32 divisions; tick = +0xd3c); always `SpaceWarSpecialpodfrissit`. `karikafrissit` (exe 0x576c40) runs only while `+0xc8c != −1`.
- `SpaceWar_ZoomFinished` (ELF 0x67fc50): if `ControlConfig[0]` and `ControlConfig[1]` (two option bytes: smooth rotate and zoom): `Rotate_Interpolator.SetValue(SpwC+0xd70)` and `Zoom_Interpolator.SetValue(SpwC+0xd78)`
  (`cValueInterpolator2::SetValue` stores `+0x24 = v` and recalculates): snaps the mouse-wheel interpolators to the camera final angle and zoom after the zoom animation. Exe address not located.

### 6.7 Effect particles, their drawing, the battle's sounds, lightning (ELF / exe)
Corrections to 3.8, 6.4 and 6.3.1: the colour word of `ujparticlerobbanas` is **0x00ffffff** (ELF 0x688b4c, exe 0x58215f;
Ghidra prints these words as denormal floats: 2.3509886e-38 = 0x00ffffff, 4.798494e-39 = 0x344040, 4.62751e-41 = 0x80ff);
kind 1 `particlerobbanas(5, .04, .3, 1.0)` makes 1 orange and **4 grey** puffs (its third loop never runs); a projectile
form's "sprite" is its **3D model**, drawn through the particle path in model mode.

#### 6.7.1 `ParticleFrissit(arr, n)` (ELF 0x5f43a0 / exe 0x52c500, exe asm read)
`SpwC::Particlefrissites` (ELF 0x6a15f0 / exe 0x56c820) calls it from every `SpaceWarFrissit` (exe 0x56dff0) while the slot
is shown, outside the pause block, on the five sets in order (set 0 `animated.bmp`, 1 `anim2.bmp`, 2 `smoke.bmp`, 3
`star.bmp`, 4 `texcsor.bmp`; exe headers `+0x142c + 0x38·i`, ELF `+0x1528 + 0x48·i`; a sixth array's count is never set).
Ring sizes `maxhanyefektparticle` (exe 0x926B20, 5 × i32), cursors exe `+0x1544`. Particle (0x4c): +0 pos, +0xc word,
+0x10 base size, +0x14 size, +0x18 velocity, +0x28 roll, +0x2c frame, +0x30 life (1 at spawn), +0x34 velocity damping,
+0x38 life factor, +0x3c life decrement, +0x40 size-pulse amplitude, +0x44 its frequency, +0x48 its random range (read as
an int). Per call `parttick++` (a static, exe 0xE3E390: +1 per set); per record, f32 unless noted:
1. `frame = (f32)((f64)frame + 0.7)` for every record, dead or not;
2. the rest is skipped when the word is 0 (an int compare in the exe);
3. `pos += vel`, then `vel *= damp`; 4. `life = life·mult − dec`; 5. `T = (u8)(int)((1 − life)·255)` into byte +0xf;
6. `(f64)life < 0.01` → `life = 0` and word 0 (dead);
7. `R = range > 0 ? rand() % range : 0`; `size = sin((f64)(parttick·freq + (f32)(R/1000)))·base·amp + base`.
An explosion (0.98, 0.01) dies after 54 updates, its frame index reaches 24 after 35; an engine puff (0.6, 0) after 10
(tests `an_explosion_fades_out_in_54_ticks`, `a_trail_puff_lives_ten_ticks`).

Spawners (libc `rand()`; `random(n)` draws nothing below 1):
- `ujparticlerobbanas(set, param, p)` (ELF 0x688a50 / exe 0x5820c0), only while shown: pos p·s, word 0x00ffffff, size
  `param·s·1.3·11·4` (1.3 in f64), life 1, frame 0, pulse fields 0, three `rand(1000)` draws for a zero velocity, damping .88,
  life .98 / .01. Set 0 is `animated.bmp`, set 1 `anim2.bmp` (25-cell animations).
- Kind 1 (exe 0x57dd40, L = 0.04·s): one puff with the word `rand(64)<<16 | 0x50c0`, size `(rand(200)/20 + 1)·L`, velocity
  normalise(3 × (rand(1000) − 500)) · `(rand(100)/100 + 1.5)·L`, .88, .99 / `.04 − (rand(100) − 50)/10000`, amplitude .2,
  frequency `rand(100)/200·3`; then four with the word 0x00344040, size `(rand(100)/20 + 1)·L`, speed `(rand(100)/100 + .1)·L`,
  .94, .98 / `.01 − (rand(100) − 50)/10000`, amplitude .2, the frequency as before.
- `particlerobbanas7` (ELF 0x688630, exe 0x57dbc0): as in 6.3.2.
- The hit flash of `Talalat` (ELF 0x69bae0; a non-beam weapon on a ship): `p_i = target_i + (rand(100) − 50)·S·10`,
  `k = random(2)`, `size = min(S·10, S·30·dmg/30·(k == 1 ? .5 : 1))`, `ujparticlerobbanas(k, size, p)`; S = the target's
  type scale.
- `VadaszRobban` (ELF 0x69c1a0): `ujparticlerobbanas(0, 0.07, group pos)` while shown; no sound, no debris.
- `Hajtomu` (exe 0x567140) reads `hajtomuvek` at exe **0x922188** (record `race·12 + type`, 0x3c B: the kind, then 4 × 3
  **f32**; the ELF has ints) and the scale at `tipusnevek + 0x10` (0x91F498); runs per battle tick after `Mozgatas`.

#### 6.7.2 Drawing (`Particle(sets, n, M, off, flags)` ELF 0x5f4ff0; the exe's not located)
Every battle set has the blend selector 100.0 (`+0x20`): ONE/ONE, depth test, no depth write, no fog.
- Textured mode (the effect sets): per record with a word and a size: `v = M·pos + off`, skipped when `v.z >= zclip`; `k =
  fov/−(v.z + zclipadd)`; a screen-aligned square of half-size `k·size` px, vertex RGB = byte·(255 − T)/256, A 255. The cell
  `clamp((int)frame, 0, N − 1)` of an N-cell sheet (`loadanimtexture`: N = 25 for animated, anim2 and smoke; 0 = one cell for
  star and texcsor): side `(int)sqrtf(N)` = 5, `u = (f % 5)/5`, `v = (f / 5)/5`.
- Model mode (`+0x18` = 1: the forms' arrays at form +0x40, `Lovedekinit` exe 0x569db0, and the fighter sets): per record
  with a size: `k·size <= 2` px → one GL point; else (a frustum test of radius size) `Kocka` of the form's model with the matrix
  `CreateMatrix2(dir, roll)·cmatrix`, every element × size, at v, transparency T/256. `CreateMatrix2(d, a)` (ELF 0x4534e0):
  `t = normalise(d × (sin a, cos a, 0))`, `u = normalise(d × t)`, elements `[−d.x, −u.x, −t.x, −d.y, −u.y, −t.y, −d.z, −u.z,
  −t.z]`. `Lovedekload` (exe 0x569ec0) takes the form's xvb, or its first sub-box when it has no animation and several boxes.

#### 6.7.3 Sounds (no positional audio)
`PlaySound(rec)` (ELF 0x671070): the volume `rec +0x110` (a write sticks) × `GlobalSoundVolume`; three voices per record.
`lovedeksound(shot)` (ELF 0x69c890 / exe 0x57adb0) runs for each live shot from `setsounds` (ELF 0x69d8a0 / exe 0x5804e0),
once a rendered frame after `Update3D`; the "new" flag (exe shot +0x67) that `Loves2` sets is cleared on its first call.
A new shot plays: weapons 0..4 Laser01..05 (29..33) at 70; 5, 6 Neutron_Gun (35) at 70; 7 Foton_Gun (26) at 70; 8 Mezongun (34)
at 70; 9 Spec5 (19) at 80, then two looped copies of 37 and 38 crossfading ±4 a frame with the attachment; 10 Paralyzator0
(36) at 80 with the same loops; 11 Antianyag0 (21) at 80 and a loop of 22 by distance and attachment; 12 a loop of 14 at 80;
13..15 Torpedo1..3 (39..41) at 80; 16, 18 Bomba1 (24) and 17, 19 Bomba2 (25) at 80; 30 Laser01 at 60. `lovedeksoundend` (ELF
0x69d980) ends the loops. Hits: `lovedeksoundtalalat` (ELF 0x69c0b0 / exe 0x57b480), from `Talalat` only (ship targets): a
non-beam weapon plays Chainblast6 (13) when `rand(2) == 0`, else Chainblast1 (8), at 60; the beam weapon 11 Antianyag2 (23).
Fighter and gun hits and fighter deaths play nothing; ship deaths as in 3.8 (55, `8 + rand(6)`; the big boom `rand(3)`).

#### 6.7.4 Lightning (`villam`; ELF vtable 0xd64960, exe 0x8487c0: Refresh exe 0x615940, Update exe 0x615c30)
`create(n, W, I, J, seg, flare, texscale 3, texspeed −0.05, frag 15, life −1, tex 1, A, B)` (exe 0x616190). The shot of
weapon 12 (form 21) holds 7 (n 12, W 1, I 1 then .2 .2 .2 .1 .1 .1, J 1.667, seg 4 then 3, a flare on the first); a unit's arc
is (n 10, W .5, I .5, J 3.333, seg 4, flare); a fighter group's (10, .2, .5, 10, 4, flare). Base points `A + (B − A)·i/n`;
the current ones are the base plus a random unit vector × `J·W`. Refresh, once a battle tick: `r = rand() % 100`; per interior
point `cur += vel`; lit = `frag <= r` (one draw for the call); colour `0x80<<24 | c·0x010101` with `c = lit ? (int)((rand() %
128 + 128)·I) : 0`; a point further than `2·amp` from its base gets a new perpendicular velocity × amp/6 and moves 0.4 of the
way back; the texture offset moves −0.05 (wrapped to ±1); the flare size is `W·4 + rand() % 100/100·W·2`. Update (the draw,
from `villamfrissit` exe 0x583830): a Hermite curve through the points, `seg` samples a span, then `kiralyvonalhuzo` (exe
0x532b80): a ONE/ONE ribbon of half-width `W·fov/−z` px textured with `villam.bmp`, v = `(i/nseg)·texscale + offset`; the
flare (`kiralyflarerajzolo` exe 0x5327a0) is a `flare.bmp` billboard at the last point, colour 0x80ffffff.

### 6.8 Kinds 5–7, the loops, lightning, rings, debris, fighters' deaths (ELF; exe where noted)
Corrections to 6.7: kind 5's life decrement is `0.2 − (rand(100) − 50)/1000` (the f64 0.2 at ELF 0xd40ab8 and exe
0x852230), so it lives 4..7 ticks; the trail puff's size is `(1 + ((f32)rand(20) + 80)/2)·L` in f32 (no int division).

#### 6.8.1 Spawners 5, 6, 7 (ELF 0x688270 / 0x688450 / 0x688630; exe 0x57d960 / 0x57daa0 / 0x57dbc0): all in set 3
| field | pr5 `(n, size, p)` | pr6 `(n, size, p, c)` | pr7 `(n, L, p, D)` |
|---|---|---|---|
| pos | `p·s` | `p·s` | `p + D·rand(100)·(s·L)` per component |
| word | 0x005cff5c | `0xff000000 \| (c<<4>>8)<<16 \| (c<<4>>8)<<8 \| (c<<5>>8)` (c 255 → 0xff0f0f1f) | 0x00ffffff |
| size | `5·s·size` | `5·s·size` | `(1 + (rand(20) + 80)/2)·s·L` |
| velocity, damping | 0, 0 | 0, 0 | D, .99 |
| life factor / decrement | 1 / `0.2 − (rand(100) − 50)/1000` (f64) | .98 / 0 | .6 / 0 |
Pulse fields 0; frame 0, life 1. Draws: pr5 one `rand(100)`, pr6 none, pr7 `rand(100)` × 3 then `rand(20)`. pr6's A byte is overwritten by the first update (6.7.1 step 5): (31, 15, 15) at c = 255, a faint red.

#### 6.8.2 `SpaceWarLovedekKirajzol`'s kinds (jump table ELF 0xd589c0, exe 0x56e874), per live shot a tick
1 `particlerobbanas(5, .04, .3, 1)`; 2 `pr2(3, .4)`; 3 `pr3(5, .05)`; 4 `pr4(5, .1)`; 5 `pr5(1, .3)`; 6 (not attached) `pr6(1,
(f32)(0.5·(f64)(d1/(d2 + d1/5))), p, 0xff)` with `d1 = tav(shooter, target)`, `d2 = tav(shot, target)` (f32: 0.42 at the muzzle to
2.5 at the target); 7 `t = age·255/T` (f32, T the weapon's life ×10 for shot kind 3), `A = (int)(a0 + (255 − a0)·t/255)`,
`pr6(1, form.size·t/36, p, 255 − A)`; 8 nothing (the lightning).

#### 6.8.3 The loops (`lovedeksound` ELF 0x69c890 / exe 0x57adb0, once a frame per live shot; `lovedeksoundend` ELF 0x69d980)
Shot fields: handles `+0x84/+0x88/+0x8c` (−1 none), volumes `+0x90/+0x94`. A loop starts as `Duplicate(rec)` + `Play(h, 4)`
at `ConvertVolume(0)`; its volume is `SetVolume(h, ConvertVolume(v))` each frame (the record's volume is not used).
`lovedeksoundend` releases the handles when the shot's shooter is cleared (`SpaceWarLovedekMozgat` ELF 0x69da70).
- 9 (one-shot 19 at 80) and 10 (36 at 80): loops 37 → h84 and 38 → h88; later frames: not attached `v94 = max(0, v94 − 4)`,
  `v90 = min(70, v90 + 4)`; attached the other way; past `(L·9)/10` of the life L (ints): both capped at `70 − ((age −
  L·9/10)·700)/L`.
- 11 (21 at 80): loop 22 → h84; not attached `v90 = max(0, (int)(d1·20/(d2 + d1/5)))` (f32); attached `A = age − (tick −
  attach tick)`, `v = (age − A) < (L − A)/2 ? 100 : ((L − age)·100)/((L − A)/2)` (ints; the exe divides by zero when `L − A < 2`).
- 12: loop 14 → h84, from the next frame at 80.

#### 6.8.4 Lightning (`villam` create ELF 0x7c5c20, `setvillampontok` 0x7c6f20, Refresh 0x7c60c0, Update 0x7c6590)
- `create(n, W, I, J, seg, flare, texscale, texspeed, frag, life, tex, A, B)`: amplitude `amp = J·W`; base points `A + (B −
  A)·i/n`, current = base + `randommerolegesvektor(B − A, 1)·amp`, velocity `randommerolegesvektor(B − A, amp/6)` per interior
  point (`randommerolegesvektor(d, k)`: `normalise(randomvektor × d)·k`, `randomvektor` = three `rand(100) − 50`, normalised);
  the end points are A and B with the word 0x80ffffff.
- `setvillampontok(A, B)`: new ends and base points; the current interior points stay.
- Refresh: `r = rand(100)` once; per interior point `cur += vel`, `c = frag <= r ? rand(128) + 128 : 0`, word `0x80<<24 |
  (u32)(c·I)·0x010101`, and when `|cur − base|² > (2·amp)²`: a new velocity (`amp/6`) and `cur += (base − cur)·0.4`; the end
  points take one more such colour; `texoff += texspeed` wrapped into (−1, 1]; flare size `W·4 + rand(100)/100·W·2`.
- Update: tangents `0.5·(p[i+1] − p[i−1])` inside, `1.5·(p1 − p0) − 0.5·t1` at the ends; `seg` cubic Hermite samples a span
  (each sample keeps the span's first point's word), then `kiralyvonalhuzo` (ELF 0x5f2650): a ribbon of half-width
  `W·fov/−z` px across the screen-space direction, ONE/ONE, depth tested; tex 1 = `textures/spacewar/villam.bmp` (2 = villam2);
  the flare (`kiralyflarerajzolo`): a `flare.bmp` square of half-size `size·fov/−z` at the last point, word 0x80ffffff.
- Who (`villamfrissit` ELF 0x6821c0 / exe 0x583830 draws them): weapon 12's shot (form 21, shot kind 0) holds 7 arcs from the
  shooter to the target, made by `Loves2`: (12, 1, 1, 1.6666666, 4, flare), three (12, 1, .2, 1.6666666, 3), three (12, 1, .1,
  1.6666666, 3), all (texscale 3, texspeed −.05, frag 15, life −1, tex 1); each tick `setvillampontok(shooter, target)` and
  Refresh. A unit the arc jumps to (the sim's `lightning`) gets (10, .5, .5, 3.3333333, 4, flare) from the target to it, a
  fighter group (10, .2, .5, 10, 4, flare); each tick `setvillampontok(arc0.cur[9], it)` and Refresh; deleted with its shot.

#### 6.8.5 Shield rings (`karikaload` ELF 0x694910, `karikakirak` 0x69c650, `karikafrissit` 0x6a2bb0)
20 nodes of `karika.xvb` at `AddToScene` 0.05, unshaded, `PropertiesOff(0xc)`. `FoldTalalat` (ELF 0x69c460), before its
building test, for weapons other than 9..12 when the gun's armour `+0xa8 > 1`: `karikakirak(shot pos, planet centre (f32)
+0x930..)`: the first free ring at the hit point, `CreateMatrix2(normalise(centre − hit), 0)`. Animation as 3.4 (all f32;
about 31 ticks), only in planet battles.
- Records (exe nodes `+0x1a88 + 0xc·i`, start tick `+8`) [obs]: scale `diag(0.05)`, position the hit point / 1024.
  `karika.xvb`'s box is ±1024 and its vertex frames are already normalised (±0.007 at frame 0 to ±0.97 at frame 30; 72
  vertices); texture `kekring_bmp.pvr` (128², RGB565, one planar ring image, UVs 0..1).
- `PropertiesOff(0xc)` (`karikaload` exe 0x576e40) clears property bits 4 and 8: no depth write, ONE/ONE
  (`renderstateallitas` exe 0x536f90). `renderscene` draws `Kocka` (opaque and translucent objects) before `Lines`, so the
  grid never cuts the rings. Animated nodes skip the frustum cull (`Kocka` 0x528c3f). `kirak` (exe 0x531460) re-lerps each
  node's own 8.8 frame (record `+0xdc`) per draw: sizes are per ring, also when the battle is paused or over and on frames
  between battle steps.
- The light is shared: `Kocka` path B (exe 0x5281f0) hands `fenyekszamolasa` (0x52f190) the ring's own byte
  `float2int(255 − 255·t)`, `t = f/30` (young = bright), but its body runs only when `DAT_00dff940` = the mesh's `+0x90` flag
  (set by `SetFrame`/`SetObjTransparency` when a value changes and by `MoveObj`/`RotateObj`/`AddToScene`, exe 0x453630) and
  writes the colours of the one `karika` mesh all 20 nodes share: the first ring drawn after a battle step lights every
  ring until the next change [obs: a probe of 0x52f190's byte argument per ring draw; after the battle's end every ring
  keeps the last step's byte, and the frame-30 rings stack into a bright torus whose hole is the texture's].
- Depth: both draw paths (`kirak` and the static 0x530b20, rasteriser 0x4ac580) write `1 + k/z`, `k` = `DAT_0091965c`
  (zbm), with the same x/y constants (`dff8e8`·`dff910` = `dff8ec`·`dff910`·`dff918` = `e033e4` = 600). Rings under the outer
  cloud shell are hidden by its depth (§1.1 step 7).

#### 6.8.6 Debris (`robbanasdarabok` ELF 0x686830, `Darabok` 0x69a7d0)
`initgfx` picks each of the 40 nodes' model `darab{random(3) + 1}.xvb` at `AddToScene` 0.0015. Spawn: `size = min(size, .002)`
(the compare in f64), `ScaleObja(size)`, at p; spin (+8, +0xc, +0x10) each `((f32)rand(200) + 50)/400·speed`, then three
`rand(2)` signs; counter (+0x18) 1. Per tick after `Hajtomu`: counter 0 hidden; else shown, `MoveObj(pos·1024 + spin)`,
`RotateObjr(spin)`, transparency 0 to counter 30 then `(f32)((c − 30)·0.02)` (f64), counter + 1, past 80 free.
- `ScaleObja(id, a, b, c)` (ELF 0x457140 / exe 0x452e30) *replaces* the record's scale matrix `+0x10` with `diag(a, b, c)`
  and re-multiplies `+0x88` (exe `+0x80`) = scale·rotation; it does not multiply what `AddToScene` (exe 0x451720) set
  (`diag(ext/max(ext)·s·+0x38)`). `BoxUjraszamol` (ELF 0x602750) leaves every vertex normalised per axis by the box's own
  extent (re/formats/xv.md), so after `ScaleObja(size)` each axis of a piece spans ±size: the flat plates (boxes (851, 210,
  1024), (645, 1024, 380), (694, 195, 1024)) are drawn as blocks, up to 5× thicker on their flat axis.
- `robbanasdarabok` (exe 0x57f170 = the ELF's, an unrolled first-free search) sets no rotation, so a reused piece starts at
  the turn its last life left. The pieces are lit as the ships and stay dark over the blasts' white cores. [obs
  `run_spwc.py --records`: the ships' root record is the identity; piece records `diag(size)·R`, sizes 0.00016–0.0013;
  the darab child records `diag(2, 2, 2)` at (−1, 1, 1), `diag(1, 2, 1)`, `diag(1.041, 1.723, 1.027)`.]

#### 6.8.7 Fighters' deaths (`vadaszmeghalas` ELF 0x6a2300 / exe 0x582fb0, `VadaszRobban` 0x69c1a0 / 0x5710f0)
Every call of `VadaszRobban` that spawns (a hit `VadaszTalalat`/`DecVadaszHp`, or a doomed retreating group's end) makes ONE
`ujparticlerobbanas(0, 0.07, group pos)`; no sound, no debris; a group leaving the arena silently makes none.

## 7. Input, timers and sounds
### 7.1 Input

#### 7.1.0 Frame structure
`IG2_gameUpdate` (ELF 0x587400 / exe 0x4e0440) runs frames until the mouse state changes, then calls `SpaceWar_Main` once per outer iteration (cadence: section 7). `Mouse_Lekerdez` + `Mouse_Darabol` give `AktIkon` (the hot-spot under the pointer; `Mouse_Megfogva` keeps the previous one while a button stays down). Flags: `Mouse_Bal/Jobb` (held), `Mouse_Lenyom(_Bal/_Jobb)` (press edge), `Mouse_Felenged(_Bal/_Jobb)` (release edge); exe 0xDB4651/52, 0xDB465F/60/61, 0xDB465C/5D/5E; `Mouse_Megfogva` exe 0xDB466A.
In `SpwC::main` (only when `+0x24ba`):
- release edges: `Felenged_Bal` -> `+0x223f = 1` (left release event), `+0x223a = 0` (left held off); `Felenged_Jobb` -> `+0x223d = 1`, `+0x2238 = 0`.
- `+0x2258/+0x2260 = Mouse_x/y`.
- `AktIkon == 0x4c` (battlefield): `Lenyom_Bal` -> `+0x223e = 1` (left press event), `+0x223a = 1`, `Mouse_Megfogva = 1`; `Lenyom_Jobb` -> `+0x223c = 1`, `+0x2238 = 1`, `Megfogva = 1`.
Then (in screenfrissites, after Update3D) `SpaceWarMouse` runs the state machine (4.2) on those flags.
Latch: `Mouse_Megfogva` is cleared on a frame with no button held (`IG2_gameUpdate` 0x4e05bc) and the hit test is skipped while it is set (0x4e054a). `SpwC::main` sets it for 0x1f and 0x4c on a press edge (0x57bb44, 0x57bb6f / 0x57bb93) and for 0x47, 0x48, 0x4b, 0x4a and the slider 0x37 on non-release frames (0x57bcea, 0x57bdb8, 0x57bf8a, 0x57bed2, 0x57bff3); 0x57c1bf also for panel B's dead drag spot 0x36. 0x46, 0x49, the weapon and info buttons never latch. `AktIkon` follows a pointer move one frame later [obs].

#### 7.1.1 Radar and the control cluster (SpwC::main ELF 0x6aed10 / exe 0x57b9e0)
| AktIkon | Action |
|---|---|
| 0x1f (radar, id 1) | every call: `dx = Mouse_x - 321`, `dy = Mouse_y - 421`, `th = +0xd6c` (f64 sin/cos to f32); wanted look-at `+0xd60 = -((cos th * dx + sin th * dy) / (94.0 / (+0x904 - +0x900)))`, `+0xd64 = (cos th * dy - sin th * dx) / (67.0 / (+0x90c - +0x908))` (exe 0xc70 / 0xc74); if `Lenyom_Bal` or `Lenyom_Jobb`: `Mouse_Megfogva = 1` (drag continues). No message, no sound. SpaceWarFrissit2 moves the look-at `0xd4c..` to the wanted point `0xd5c..` by delta/4 per tick, the step limited to length 1.5 after the /4, snapping when all three deltas < 0.01; the look-at is first clamped inside the arena minus 2 cells per axis. `SpwC::main` itself has no button test here, but it runs only while a button is held over the radar or on its release (§4.2), so the radar never pans on hover alone |
| 0x4c battlefield | press edges set the flags of 4.0; clicks are handled by SpaceWarMouse (4.2) |
| 0x46 (colour 0xf, id 0x28), only if ControlConfig[0] == 0 | `PlaySoundGlobal(0)`; unless a view animation runs (`+0x229e`): `+0x2520 = 0x28` frames, wanted pitch `+0x24d8 = -1.5607964` (0xbfc7c82d, straight down), yaw `+0x24d4 = 0`, distance `+0x24dc = scale(+0x2514) * 200`, centre `+0x24e0..e8 = 0`, `+0x22ec = 0`, `NezetVisszaAllitSet`, `+0x229d = 0`, `+0x229c = 0`. Button animation: `KezeloGombFrame = 0`, `GombFazis = 1`, `MelyikGombot = 4` |
| 0x47 (0x10, id 0x29) | unless release: sound 0 on `Lenyom`; `Mouse_Bal`: yaw `+0xd70 -= 0.1`; `Mouse_Jobb`: `+= 0.1`; `Megfogva = 1`; `MelyikGombot = 5` |
| 0x48 (0x11, id 0x2a) | same with the opposite sign (`Bal`: `+= 0.1`, `Jobb`: `-= 0.1`); `MelyikGombot = 2` |
| 0x49 (0x12, id 0x2b) | like 0x46 but wanted pitch `-0.8` (0xbf4ccccd); sound 0; `MelyikGombot = 1` |
| 0x4a (0xb, id 0x2c) | unless release: sound 0 on `Lenyom`; `+0x2300 = +1.0` if `Mouse_Bal` else `-1.0`; on a release `+0x2300 = 0`; `MelyikGombot = 0`; `Megfogva = 1`. Per tick (SpaceWarFrissit2): `dist -= dist / 25 * +0x2300`, `dist` clamped >= 5.0 and (when zooming out) <= 400.0. FRISSIT_SpaceWar2 zeroes `+0x2300` and `+0x2304` after every tick |
| 0x4b (0xc, id 0x2d) | unless release: sound 0 on `Lenyom`; pitch `+0xd74 += 0.0314159` if `Mouse_Bal` else `-= 0.0314159`; clamp to [-1.5607964, -0.2]; `MelyikGombot = 3`; `Megfogva = 1` |
| 0x42 (id 0x24, small info box) | unless release: `infomodeonoff` (4.4) |
| 0x37 (panel A base 0x19 + 0x1e = id 0x19, the Defend/Attack slider; hot-spot live only while nothing is selected) | unless release: `Megfogva = 1`; `gfxvadaszdefendattack = clamp(((Mouse_x - 0x32) * 100) / 0x71, 0, 100)` (exe 0xE72D90) |
| 0x21..0x2a (panel C base 3: ids 3..0xc, weapon buttons k = AktIkon - 0x21) | unless release; see 4.3 |
| rest of panel A (0x38..0x3b) and panel B (0x2c..0x36) | dead (hot-spots off): A: group size `+0x48` cycles 0..2, target type `+0x4c` 0..2, priority of the selected non-own units `+0x1ac` 0..4, close = `panelkibe`; B: 0x2c = infomodeonoff, 0x36 = drag the panel (`+0x2364..0x2394`). `exchangemouse` (swaps two panels hot-spot blocks of 11) has no caller. |
Button animation: `GombFazis`/`MelyikGombot`/`KezeloGombFrame` are written by the handlers above (section 5).

#### 7.1.2 SpaceWarMouse state machine (ELF 0x6a9490 / exe 0x56f420) and SpaceWarMouseMove (ELF 0x6b2c90 / exe 0x56fa60)
Per frame: (editor mode and nothing selected: select the first live unit); `+0x2244 = pointer-on-plane inside (+0x900,+0x904) x (+0x908,+0x90c)` via `SikMetszespont(mouse, +0x928 * +0x2514 + +0x24f0)`;
left press event -> press origin `+0x2248/4c = Mouse_LastPressed_Bal`, timer `+0x2268 = 0`; right press -> `+0x2250/54 = LastPressed_Jobb`, `+0x226c = 0`.
Let L = `+0x223a` left held, R = `+0x2238` right held, Lp/Rp = press events (`+0x223e`/`+0x223c`), Lr/Rr = release events (`+0x223f`/`+0x223d`), Sup = `+0x2243`, Rot = `+0x2240`, Hgt = `+0x2241`, Box = `+0x2242`.
Not Box:
- not Hgt, not Rot, not Rr:
  1. `!Sup && Lr && !R` -> BalClick (select).
  2. else `!Sup && Lp && R` or `!Sup && Rp && L` (both buttons) -> `Rot = 1` (camera rotate drag).
  3. else `!Sup && !L && R && own-selected (+0x170c != 0) && nothing hovered (+0x2230 == -1)`: if some selected live own unit has a movable ship type (`unit +0x230 -> +0x14 > 0.01`) and `|Rpress_y - Mouse_y| > 5` -> magassagbeallitas (height mode: `Hgt = 1`, remembers the right-press point `+0x2270/74/78`, the selected unit position `+0x2280`, the plane point `+0x2284/88`; the marker is drawn by magassagkirak).
  4. else `!Sup && L && (|Lpress_y - y| > 5 || |Lpress_x - x| > 5) && !(+0x229e)` -> box select starts: `Box = 1`; snapshot of the selection flags (`+0x21b8[i] = unit +0x189`, guns `+0x8b = +0x89`).
- not Hgt, not Rot, Rr -> JobbClick (orders).
- Rot: if neither button is held -> `Rot = 0`.
- Hgt: no Rr and Lp -> `Hgt = 0; Sup = 1; Rot = 1`; Rr -> JobbClick, then `Hgt = 0`.
Box: while the left button is held: the rectangle is drawn with four `Line(.., 7, 0xff)` (colour 7) clipped to `HatarX1..HatarY2` (the full 640 x 480 viewport); the selection is rebuilt every frame: `kijelolesleszed(false)`; with Shift (`KeyShift`) the snapshot of own units is re-added first; then every live own, not yet selected, not retreating (`+0x1e0` clear) unit whose projected centre (`GetObjCenter` + `TransformPoint`, z < 0) is strictly inside the rectangle is selected (speech kind 1 for units that were not selected in the snapshot: `i % 3 + 1`, condition `+0x24b8 && unit +8 < 8 && beszedlasttick + 0x1e < acttick2`); planet guns of the human planet owner (`+0xd7c == +0xd2c`) likewise (`RotateAndTransformPoint`). Counters `+0x16f8/fc/1700/1704/1708/170c` are updated as in kijelol.
Box end: a right press during the box (`+0x223c`) cancels it (selection restored from the snapshot, counters recounted, `Box = 0; Sup = 1; Rot = 1`); when the left button goes up (`+0x223f` seen in the Box branch) `Box = 0` and, if the fire cursor was armed (`MousePointerType == 4`), it is restored (`MousePointerType = +0x24c4`, armed group and marker -1).
After the machine: if neither button is held `Sup = 0`; if `Rot && !(+0x229e)`: yaw `+0xd70 -= (+0x225c - x) / 100.0` and pitch `+0xd74 -= (+0x2264 - y) / 200.0` (f32 arithmetic; prev = last frame mouse), pitch clamped to [-1.5607964, -0.2]; then `SpaceWarMouseMove`; `prev = now`; the four event flags are cleared.
SpaceWarMouseMove (not Rot/Hgt/Box; corrected in 8.7.5): `+0x2234 = 0`; `best = 400` pixels; for each live unit whose owner == me or `+0x1e3` clear: centre projected (`GetObjCenter`, `TransformPoint`, z < 0, the 639/479 round trip), radius `max(10, TransformTav(ship type scale (+0x230 -> +0x10) * 1000, z))`; `d = tav2` (a pixel distance, `sqrt` in f64); accepted if `best > d && radius > d` (strict): `best = d`, `+0x2230 = idx`, `+0x2234 = 1`; then planet guns (`+0x84 != 0`, radius 10, `RotateAndTransformPoint`) share `best`, so a gun wins only when nearer (`+0x2234 = 2`). Nothing -> `+0x2230 = -1`, `+0x2234 = 0`. `TransformTav` takes z in scene units (battle units × the scene scale), so while the scale is 0.0004 (the opening and closing zooms) the radius is 2500× the drawn size and the nearest unit within 400 px is hovered [obs: the hover cursor with no unit under the pointer].
Cursor (`MousePointerType`, only when < 4, i.e. no fire cursor armed): nothing hovered -> 0; hovered own unit (or a gun when the planet is mine) -> 2; hovered other unit or gun: 2 if nothing is selected (`+0x16f8 == 0`) or if any selected live unit is the human's enemy (`ellenseg(u)` = `rel[EN·0x13 + u.owner] == 1`: the selected unit's owner, not the hovered one's), else 3 (attack cursor). `Mouse_Tipus` per frame: 0 -> 1, 2 -> 5, 3 -> 7, 4 -> 6 (exe 0xDB9414).

#### 7.1.3 Clicks and the messages (BalClick ELF 0x6b13c0 / exe 0x566810; JobbClick ELF 0x6b0210 / exe 0x569190; weapon buttons in main)
BalClick (left release; needs `AktIkon == 0x4c`, `+0x24bc == 0` (battle not over), `!(+0x229e)`): `PlaySoundGlobal(0)`; if the fire cursor is armed: cancel it (`MousePointerType = 0`, `+0x24c4 = 0`, `+0x24c8 = +0x24cc = -1`).
- hovered kind 0: `kijelolesleszed(false)` (deselect all).
- kind 1 and the unit not retreating (`+0x1e0`): without Shift `kijelol(idx)` (deselect all, select it: `+0x16f8 = +0x16fc = 1`, `+0x1704 = idx`, `+0x1708 = kind`, `+0x170c += 1` if own) plus speech kind 1 (`idx % 3 + 1`; own unit, `+0x24b8`, `unit +8 < 8`, `beszedlasttick + 0x1e < acttick2`); with Shift: if nothing own is selected clear first, then `kijelolinvert(idx)` (toggle).
- kind 2: `kijelolagyu(gun)`; with Shift `kijelolinvertagyu(gun)` (same for guns, counters `+0x1700`).
No message is sent by a selection: selection is local UI state.

JobbClick (right release; the same three conditions and additionally not in the formation phase: `+0x342d` clear, exe 0x2d85 tested at 0x569190; BalClick does not test it, so selecting works during formation): `PlaySoundGlobal(0)`, then by `MousePointerType`:
| Pointer | Condition | Message (Order) |
|---|---|---|
| 3 (attack) | hovered kind 1 (target t) | per own selected live unit `i`: `Multi_Send(0x71, EN, 4,&i, 4,&t, 4,&AktSpaceWar, 0)` = `Attack { unit: i, target: t }`, speech kind 2; and if I own the planet (`+0xd7c == +0xd2c`) per selected gun `g` with `+0x84 != 0`: `Multi_Send(0x74, EN, 4,&g, 4,&t, 4,&slot, 0)` = `GunTarget { gun: g, unit: t }` |
| 3 | hovered kind 2 (gun g) | per own selected live unit `i`: `Multi_Send(0x73, EN, 4,&i, 4,&g, 4,&slot, 0)` = `AttackGun { unit: i, gun: g }`; speech kind 2 |
| 0 or 2 (move) | per own selected live unit (editor: only the first unless Ctrl) | not Hgt: point = `SikMetszespont(mouse, plane height)`; y and z clamped into `[+0x900 + 2*+0x8e8, +0x904 - 2*+0x8e8]` and `[+0x908 + 2*+0x8ec, +0x90c - 2*+0x8ec]`; x (height) = the unit own height (`+0x48`), clamped into `[+0x910 + 2*+0x8f0, +0x914 - 2*+0x8f0]`; sent only if the pointer is on the plane (`+0x2244`): `Multi_Send(0x70, EN, 4,&x, 4,&y, 4,&z, 4,&i, 4,&slot, 0)` = `Move { unit: i, x, y, z }` (the port already has `Battle::screen_point` for the clamp). Hgt: the stored point (`+0x228c/90/94`, computed by magassagkirak: the point under the cursor at the chosen height) clamped the same way, sent without the on-plane test. Speech kind 0 |
| 4 (fire cursor armed, group g = `+0x24c8`) | hovered kind 1 (target t) and `+0x2230 != -1` | per own selected live unit `i`: `d = tav(pos_i, pos_t)`; for each mount `m` (count at unit `+0x238`, record 0x70) with weapon `w >= 0` whose bit `1 << w` is in the group mask `fegyvernevek[g].mask`, kind (`+4`) != 2, `d <=` range (`+0x60`) and timer (`+0xc`) < 0 (ready): `Multi_Send(0x44e, EN, 4,&i, 4,&t, 4,&m, 4,&slot, 0)` = `DirectFire { unit: i, target: t, mount: m }`. If at least one was sent and `+0x24cc == -1`: `MousePointerType = +0x24c4; +0x24c8 = -1` (disarm) |
Mount offsets are the ELF record (0x70); the exe record layout may differ [u]. space-combat.md 7.12 has the handlers.

Weapon column buttons (SpwC::main; hot-spots 3..0xc, k = AktIkon - 0x21, never on a release frame):
- Right button (`!Mouse_Bal`), no formation phase, k < 9, fire cursor not already set by the saved-cursor test (`+0x24c4 != 4`): if k == `+0x24c8` (already armed) -> disarm (`MousePointerType = 0`, `+0x24c8 = +0x24cc = -1`, `+0x24c4 = 0`); else arm: `+0x24c8 = k`, `+0x24cc = -1`, `+0x24c4 = 0`, `MousePointerType = 4` (fire cursor). No message, no sound.
- Left button, no formation phase: k == 9 -> `SPWRetreatWindow = 1`. k < 9 and `state[k] >= 0` (the group exists): if `state[k] == 7` (all mounts on): `keep = ~mask`, `set = 0`; else `keep = 0xffffffff`, `set = mask`; per own selected live unit (if `keep != 0xffffffff || set != 0`): `Multi_Send(0x44f, EN, 4,&i, 4,&keep, 4,&set, 4,&slot, 0)` = `Weapons { unit: i, keep, set }` (toggle the group). No sound.
- Formation phase (`+0x342d`): left button: k == 9 -> `+0x342e = 1; SetIdoSpeed(1, false); SetIdoPause(false, false)` (start the battle; the column slides out, then `+0x342d/+0x342e` clear); k < 9 -> `+0x3430 = k` (a tick later SpaceWarFrissit2 does `sprintf("felallas%d", k + 1); loadfelallas` once per change: a formation file of that name).
Group masks (ELF table at 0x1199960 + 0x20 + 0x24*g; the exe image has zeros there, see section 7): 0 Lasers 0x1f, 1 Guns 0x1e0, 2 Paralyser 0x200, 3 Ship manipulator 0x400, 4 Anti-matter ray 0x800, 5 Destructor Ray 0x1000, 6 Torpedo 0xe000, 7 Bomb 0x30000, 8 Virus bomb 0x40000, 9 Retreat 0x80000 (weapon kind bit numbers 0..19).
infoszamol statistics per group (every 5 ticks, from the selected own ships): `count[g]` = sum of mount barrels (`mount +8`) of kinds in the mask; `enabled[g]` = the same for kinds enabled in the unit weapon mask (`unit +0x1c8` ELF); `maxreload[g]` = max over mounts of `clamp(0xa9 - timer * 0xaa / timerSet, 0, 0xaa)`; `state[g]`: -1 if count 0, 6 if enabled 0, 7 if enabled == count, else 3.
Fighter stance: when `gfxvadaszdefendattack != side[+0xd2c].+0x44` (checked on every infoszamol run, only with `+0x24b8`): `Multi_Send(0x77, EN, 4,&value, 4,&side, 4,&slot, 0)` = `FighterStance { side, value }` (side = `+0xd2c`, my empire).
Retreat: `Multi_Send(0x72, EN, 4,&slot, 0)` from HandleRetreatWindow (3.4) = `Retreat`.

#### 7.1.4 infomodeonoff (ELF 0x6afbe0 / exe 0x573f20): small info box click, also called by NezetVisszaAllitas when `kijelolescenter` returns true
Only if `infoelmegy` is outside 0..1 and the view is settled (`+0x229c`):
- big info closed (`+0x229e` clear) and ships selected (`+0x2578 != 0`): store the selected ship list (`+0x3234[]`, count `+0x3414`), `+0x2547 = 1`, `+0x2520 = 0x14` frames, wanted pitch -0.8 (`+0x24d8 = 0xbf4ccccd`), distance 200.0 (`+0x24dc = 0x43480000`), yaw `+0x24d4 = 0`, `+0x22ec = 1`, `NezetVisszaAllitSet`, `+0x229d = 0`, `+0x229c = 0`, `+0x24d0 = 1`: the camera flies to the selection and the big info box slides in.
- big info open (`+0x229e` set): `+0x2547 = 2`, `+0x2520 = 0x14`, `NezetVisszaAllitSet2` (back to the saved view), `+0x229d = 1`, `+0x229c = 0`, `+0x24d0 = 0`.
No sound, no message. `+0x229e` = a view animation is running or the info view is held (NezetVisszaAllitas clears it at the end of a normal preset move and of the info-off move, leaves it set after the info-on move): it blocks free camera control, BalClick/JobbClick and a second infomodeonoff (`+0x229c` settled flag).

#### 7.1.5 Keyboard (SpaceWarKeyboardHandler ELF 0x67e190 / exe 0x561ef0)
Signature `(int key, int ch, bool down)`: `key` = key code (arrows 0x25..0x28, digits 0x31..0x39, numpad + / - 0x6b / 0x6d, 0x91 ScrollLock, 8 Backspace, 0x21 / 0x22 PgUp / PgDn, 0x2e Delete), `ch` = the character (letters, both cases compared). Everything except the last block only on key down; the last block (rates) on down (value 1) and up (value 0).
| Input | Condition | Action |
|---|---|---|
| ch `c` / `C` | not editor | `spwcameramode += 1`, wraps to 0 at 2: toggles the fighter chase camera (screenfrissites needs `aktvadasz >= 0 && vadaszlinked`) |
| ch `g` / `G` | down | `+0xd90` (exe 0xc98): if 0 -> `(+0xd94 < 0x81) ? 1 : -1`, else negate [meaning not read; a display mode of the ship lines, guess] |
| ch `a` / `A` | down | `flottafelallit`: deselect, then select all own non-retreating units (+ all planet guns when I own the planet) |
| key 0x31..0x39 (top row only) | down | no Ctrl: `csapat(n - 1)`: cancel an armed fire cursor, deselect, select all own units with group == n - 1. With Ctrl: `csapatkijelol(n - 1)`: if the own-selected count `+0x170c != 0`, every selected unit (also non-own ones) gets group n - 1 (unit `+0x1c4` ELF / `+0x1b8` exe). No sound, no message. Groups start at 0 for every unit, so key 1 selects the never-assigned units |
| key 0x91 | down | `chm = !chm` (global `chm`, not gated by DEBUGX in either binary; meaning unknown) |
| arrows 0x25 / 0x27 | not editor; down and up | `+0x2304 = down ? -1.0 : 0` (left) / `+1.0` (right): yaw rate; SpaceWarFrissit2: `yaw += +0x2304 / 60` per tick |
| arrows 0x26 / 0x28 | same | `+0x2308 = -1.0` (up) / `+1.0` (down): pitch rate; per tick `pitch += +0x2308 / 250`, clamped to [-1.5607964, -0.2] |
| key 0x6b / 0x6d | down and up | `+0x2300 = +1.0` / `-1.0`: zoom (same as the zoom button). FRISSIT_SpaceWar2 clears `+0x2300/04` after every tick, so a held key works through the OS key repeat (below) |
| DEBUGX only (exe 0xBAE587): ch `q` | down | `SpwC::initgfx` |
| DEBUGX: ch `u` | down | toggle `side[EN].+0x54` (AI control of my side) |
| DEBUGX: ch `r`, not `TutorialPaused` | down | `Multi_Send(0x72, EN, 4,&AktSpaceWar, 0)` = `Retreat` |
| DEBUGX: key 8 | down | for every selected unit: `felrobban(.., 0, 0, true, true)` once per ship (`+0x1bc` count): blow up |
| editor mode only | down | ch `c`: reverse the order of the selected units; arrows: move selected units by one racs cell in y/z (Shift + up/down: x); PgUp/PgDn change the unit type (`+8`, 0..0xb); key 0x2e (Delete) deletes (with Ctrl: sets the unit single waypoint) |
No other key is handled here. The shared `DoKeyboard` (exe 0x4db1a0, `re/screens/input.md` §1) runs first and keeps some keys: the top row's 1..3 (vk 0x31..0x33, jump table 0x4dc024 / index 0x4dc038, target 0x4db434: sound 10, `SetIdoSpeed(n)`, unpause; never passed on, so only 4..9 pick or assign groups 3..8 here), Space and the character P (pause toggle), O (options); these and the top bar's speed buttons (`IG2_gameUpdate` 0x4e07d6, AktIkon 2..5) need `nemlehetidotallitani` (exe 0xBAD0D4) clear in SP, i.e. not during the opening zoom (§2.5). Esc reaches `ESC_Pressed` (options.md §5.1): it opens the options too (`KepernyokLetiltva[0xd]` is restored at the zoom's end); `SpaceWar_Kilep` keeps a running battle (§4.1), and the exe comes back through the starmap (`OptionsElottiKep` 0xb -> 2, `DAT_00f176e0 = 0xb`; §4.1.2). The letters come as character events (`FUN_004db040(0, ch, 1)`), so they follow the typed character, not the key position. g/G (`+0xc98`) and ScrollLock (`chm`) have no reader in the screen's functions; DEBUGX u is a local write (exe `side[EN] +0x50`), not sent.
Key events: `MainCallBackEventHandler` (exe 0x4e2db0, the only writer of the key ring 0xb773a8 / 0xb77394, write index 0xb773fc) pushes SDL2 KEYDOWN (0x300, the keys `remapKeyCodes` 0x4e4670 accepts) and TEXTINPUT (0x303) with no repeat test (the only `[+0xd]` test, 0x4e31b1, is the text's second character), so every held key repeats at the OS rate on every screen, SpaceWar included; a full ring (19 entries) drops. V: `DoKeyboard` at 0x4dbf2f stores `SDL_GetTicks` in 0xB772C0 for a v/V text event when no text entry is up (0xbae2c3 == 0), on every screen, and eats the key; its only reader, `UpdateFrame` 0x4e3e90 at 0x4e43a7, draws "Not Relevant Anymore" at (10, 470) (`FUN_004a9e60(str, 10, 0x1d6, 0x3c, 2, 4, 3)`) for 1000 ms.

#### 7.1.6 Top bar
Spots 2..5 (pause, speeds; flag 1) act on the press edge of "any button held" (`FUN_005105d0` 0x5105d0 snapshot: 0xdb4650 any held, 0xdb4653 changed; `IG2_gameUpdate` 0x4e07d6 pause, 0x4e086d speed): a right press acts like a left, a second button while one is held does nothing, releases do nothing. Spot 7 is handled in `MOUSE_SelectScreens` (exe 0x555470): a left press in SpaceWar needs `DEBUGX` (0xbae587) and an unread message (`VanUzenet` 0x509e00) → `AktKepKell` 10 (0x555788), then the leave path of §4.1.2; the right button opens the history (0xe40c5e) on the press edge while left is not held, closed on release, ungated.

### 7.2 Timers and animations
`FRISSIT_SpaceWar2` (ELF 0x67c450 / exe 0x55e6e0; `AktKep == 0xb && AktSpaceWarp != 0`; **every 100 Hz UI tick**: `Fake_FRISSITES` calls it at exe 0x4de8de inside its `while now − stamp > 10` loop (0x4de850..0x4de97f), right before `SpaceWar_Autoscroll` (0x4de94d) [obs: the column counter runs 0 → 49 in 50 UI ticks]. The interpolators still take dt 0.033333335 = 1/30): `SpwC::SpaceWarFrissit2` (below), then `+0x2300 = +0x2304 = 0`. The pitch rate `+0x2308` is not cleared (neither binary): a pitch key stays set until its key-up event. If `ControlConfig[0] && ControlConfig[1]`: `Refresh(1/30)` of the four interpolators (MoveX, MoveY, Zoom, Rotate); if `!(+0x2524) && !(+0x229e)`: zoom `+0xd78 = Zoom.GetValue()`, yaw `+0xd70 = Rotate.GetValue()`; `SpaceWar_Move(MoveX.GetSpeed(), MoveY.GetSpeed())` pans the look-at (the exe and the ELF differ here, see 7.1). The interpolator inputs (pan keys / pad) are set elsewhere [u].
`SpwC::SpaceWarFrissit2` (ELF 0x6a3380 / exe 0x56d870), only with `+0x24b8`, per tick:
- `acttick2 += 1` (speech throttle clock: `beszedlasttick + 0x1e < acttick2` = 30 ticks = 1 s between voice lines); `spwradarcounter` +1 (to 100), or -1 (to 0) when leaving (`+0x2544`); frozen during intro and fade-in; `futofeny += 1`; `spwvillogas += 1` (blink counter, period 50 ticks, shown for the first 26); `FrissitStarmapParticles`; `FaderDesignFrissito`.
- No view animation (`+0x229e` clear): zoom `-= zoom / 25 * +0x2300`, clamp `< 5.0 -> 5.0`, zooming out (`+0x2300 < 0`) `> 400 -> 400`; look-at clamp and ease (4.1); `yaw += +0x2304 / 60`; `pitch += +0x2308 / 250`; pitch clamp [-1.5607964, -0.2].
- Control cluster animation: `if !Mouse_Gomb: GombFazis = 2`; `GombFazis == 1`: `KezeloGombFrame += 1`, max 10.0; `GombFazis == 2`: `-= 1`, min 0.0; then `SetFrame(sub-mesh MelyikGombot of KezeloId, KezeloGombFrame)`: the pressed button of the 3D cluster sinks over 10 ticks and rises again over 10. MelyikGombot: 0 zoom, 1 default view, 2 yaw right, 3 pitch, 4 top-down, 5 yaw left.
- `+0x258c -= 1`; below 0: `infoszamol` and `+0x258c = 5` (every 6th tick = 5 Hz).
- Weapon column counters (3.1) when not in intro/fade-in; formation file reload when `+0x3430 != +0x3434`.
- `smkeringeto += 0.0002`; `kisinfocounter += 1`; `kamerabeallit`; `infoelmegy`: if in [0, 1]: `-= 0.02` (min 0).
- `spwcounter`: after 10 -> 0 and `SpeakingDirector::Refresh(SpaceWarSpeaker)`; `StarmapLastCommandTime` counts down.
Sim clock: `Fake_FRISSITES` (exe 0x4de720) runs the 10 ms UI loop (with `FRISSIT_SpaceWar2`), then its `> 50` ms sim loop, so the battle steps on every fifth UI tick at a phase set by the run. During an on-screen SP battle it takes its frozen branch: the strategic clock and money stand (time-and-rng.md §1), and no `FRISSIT_Starmap` runs, so `StarmapPlanetKering` (the starmap orbits) stands too.
Frame timers (screenfrissites): SDL ticks `+0x22f0`; `+0x2268/+0x226c` (+= dt ms; unused by the code read); `animtick`; the view animation uses `(now - +0x22ac) / +0x2520 frames * visszaallitasidoszorzo / 50` through `gorbe` (3D half). `spwckezdotimer` (set at the end of the intro) drives the MP versus banner (9 s). The battle-end timer `+0x24bc` drives the black fade (3.3). View durations: buttons 0x46 / 0x49: 0x28 frames; info mode: 0x14; leaving to the starmap: 0x50; intro: 0x50.

### 7.3 Sounds and music
- Music: `PlayMusicSpWar(true, false)` in Belep (exe 0x540920), skipped if `SelfPlaying` or a battle is already running.
- `SpWar_Hangok` (exe 0x91a838, 42 records, volume 100; files under `dataraw/sounds/spacewar/...`): loaded by `InitSounds` in Belep, freed by `DeInitSounds` in Kilep. The HUD never plays from it; the records are played by the 3D/sim half (weapon sounds in `lovedeksound`, explosions in `urhajomeghalas`): 0..2 Destructions/BigBlast1-3, 3..7 Blast1-5, 8..13 Chainblast1-6, 14 Weapons/Paralyzator1, 15..20 Specialis/Spec1-6, 21..23 Antianyag0/1(loop 4)/2, 24..25 Bomba1/2, 26 Foton_Gun, 27 Halalsugar-kiloves, 28 Halalsugar-loop, 29..33 Laser01..05, 34 Mezongun, 35 Neutron_Gun, 36 Paralyzator0, 37 and 38 Paralyzator2 (the same file twice), 39..41 Torpedo1..3.
- HUD clicks [obs: none for the start button, the box select or the zoom key]: `PlaySoundGlobal(0)` = `Global_Hangok[0]` ingame/Click1.wav (volume 90). Played by BalClick, JobbClick (once per right release that passes the conditions), camera buttons 0x46 and 0x49 (on every call), 0x47 / 0x48 / 0x4a / 0x4b on `Mouse_Lenyom` only. Not played for: radar, small info box, slider, weapon buttons, retreat window, formation buttons. No other `PlaySound*` index occurs in these functions.
- Voice lines: `SpeakingDirector::Play(&SpaceWarSpeaker, i % 3 + 1, kind)`: kind 0 on a move order, 1 on a select (click or box), 2 on an attack order; kinds 5..9 are combat events (SpwC::dechp, not HUD). Conditions: unit owned by me, `+0x24b8`, `unit +8 < 8` (capital ship class), 30 ticks since `beszedlasttick`. `SpeakingDirector::Refresh` every 11th tick. Voice files not read (audio.md).
- The starmap zoom-out sound (`StarmapKiZoomol` plays Global 5) is not in these functions.

### 7.4 Fighters, gun markers, boxes, voices, formation, retreat, cursors, banner (ELF; exe where noted)
- **Fighter sprites** (correcting 3.2): the four sets are the plan's fighter class (`Tervek[owner][design] +0x44`, `AddUjVadasz`
  ELF 0x6918f0 / exe 0x566420: `clamp(class − 1, 0, 3)`), not a relation; all four nodes load `vad10.xvb`; one sprite per
  GROUP (`AddFleet`: `groups = max(1, (int)(count·min(1, 60/total)))`); particle size 1.0, word 0x2c2c2c, model mode.
- **Chase camera**: key C toggles `spwcameramode` 0/1 (`SpaceWarKeyboardHandler`); `kamerabeallit` follows the first live group
  (`zbm = 1`), links (`aktvadasz = g`, then hidden) once the eased camera is within 1.0; ends on the key, a view animation, or
  moves to the next live group when the followed one dies.
- **Gun markers** (`agyukirak`, confirmed, all f32): `T` (gun +0x10) is 1000 for the human's guns, else 1 and `+= dt` (cap 1000)
  in `kijelolesanim`: an enemy gun's brackets close from r ≈ 9000 to 2 in a second.
- **Small box** (`kisinfokiir`, confirmed against exe 0x577c00; cases B and C as in §5.2): it reads the selection only, no
  hover; the overview's unit suffix: divisor 1000 ("K") above 9999, 1e6 ("M") above 999000.
- **Big box** (`nagyinfokiir`, exe 0x57c9c0 the same): needs big-info mode AND something selected; the ship rows need exactly one
  live ship; hull `"%d%%, %dhp"` = `dmg·100/max` (damage per cent: an original quirk [u]) and `(max − dmg)/15`; it keeps the
  caller's fade multiplier.
- **Starmap fade** (`starmapszarok`, confirmed): `starmaphalvanyit(e)` (ELF 0x6a6b30) multiplies the starmap particle words'
  RGB by e (drawn colour `+0xc` = stored `+0x48` with RGB × e; FStars 0x173eea0, stride 0x48, sets 1, 2, 4..10 and a second
  block of 8 sets at 0x17a0e80; sets 0 = far stars and 3 = ring untouched; e = `starmaperosseg` = 0 at battle zoom, so the
  galaxy clouds vanish), sets the starmap objects' alpha `(int)(K − (K − a0)·e)`, and the planets' meshes transparency `1 − e` (hidden at 0; depth kept) [obs: a planet at opacity byte 166 at e 0.65].
- **Voices** (`SpaceWarSpeaker` exe 0xe72e84, filled by `InitSpaceWarSpeech` exe 0x55e8b0 / ELF 0x6765f0 with 606
  `SpeakingDirector::Add` calls, read like `ig2_rules::speech`'s tables): the colony's `Sounds/Colony/Speech/<Kind>/` files,
  kinds 0 Move, 1 Selection, 2 Attack, 3 Retreat, 5 Hit, 6 Dying, 7 Dead, 8 enemy_hit, 9 enemy_dead, 10 enemy_retreat; line
  `id % 3 + 1` (unit id exe +0x19c), the race offset from the human's `EN`. Refresh every 11th UI tick (`spwcounter > 10`);
  every site also needs `beszedlasttick + 30 < acttick2` and then sets it. Sites: 0/1/2 the HUD orders; 3 (mine) / 10 (others)
  in `retreat` (ELF 0x6acb40) with the fixed line 1; 6 in `dechp` (a near-dead capital ship, after the sim's `rndlog(6,0,2)`
  draw); 7 (mine) / 9 when a capital ship dies; 5 and 8 never (their block divides `rand()` by 0 or 1, exe 0x572470).
- **Formation** (`loadfelallas` ELF 0x6a5a60 / exe 0x578850, `FelallasLoad` exe 0x566eb0, `felallit` exe 0x572ef0): only in SP
  with 6 or more of my ships (`+0x342d`); the column's buttons 0..8 set `+0x3430 = k` (`final2x.bmp` frame 0 chosen, else 32;
  tooltips `felallasnevek` = `GetString(0xb,3,i+11)`), button 9 starts (`SetIdoSpeed(1)`, unpause). `SpaceWarFrissit2` (no zoom
  or fade) loads `felallas{k+1}` when `+0x3430 != +0x3434` and re-places my side (`felallit`: the sim's set-up placement, rotated
  to face the others). **Without the phase nothing in the screen unpauses**: the player's top-bar pause button does (`IG2_gameUpdate`
  exe 0x4e0440, blocked during the zoom); in MP the battle is never paused.
- **Retreat** (8.7.5 resolved): `HandleRetreatWindow`'s only caller is `screenfrissites` (exe 0x57f8db), when the left button is
  up and the window open; the window opens while the left button is held on button 9, so it is a press-drag-release popup:
  releasing on the button closes it.
- **Cursors** (`UpdateFrame` exe 0x4e3e90): `MOUSE.BMP` (`dataraw/screen/mouse_bmp.pvr`), 20 × 20 cells, column = type, row =
  `(MouseFazis/5) % 40` (one row per 5 UI ticks); hot-spot offset table exe 0x90b608 for x and y (types 0, 1: 0; others 10).
  `screenfrissites` maps `MousePointerType` 0 → 1, 2 → 5, 3 → 7, 4 → 6 (1 keeps the hot-spot's type, 1); tooltips are the
  shared ones (spy.md §5; the strings of 4.2, exe immediates confirmed).
- **MP banner** (exe 0x57f800): while MP (`DAT_00bae2c0`), more than one empire present (`+0xe30`), and
  `now (+0x1d90) − spwckezdotimer (0xE72E18) < 9000` (set on the screen's first frame, with the opening zoom's set-up of the
  intro flag `+0x2524`): `"%s (%s) vs. %s (%s)"` with `Birodalom_Nev` and `PlayerNev` (0xBAE588, 11 bytes) of the present
  list's first two empires (`+0xe09`, `+0xe0a`; not only humans), through `a2az`, at (200, 30), `KiirStr(.., 200, 30, 0x46, 2,
  4, 2)`; `KiirStrSzinSzorzo = (int)(spacewarelfade·100) · min(100, (9000 − ms)/5) / 100` (ints), so it fades over its last
  0.5 s (not captured: MP). The formation caption `GetString(0xb,1,4)` sits at the same place, alpha × the column's counter/50.

## 8. Corrections, exe vs ELF, checks and unknowns
### 8.1 Corrections to the 3D draft and resolved questions
- 3d spec 3.6 "fighter groups (list +0xd88, stride 0xb0, +0x84 live)" are the **planet guns** (`Gun`, Rust `guns`): `+0x84` is the building
  slot (index into `Epuletek[planet +0x24b4]`, 0x20 B per building: +0 type, +4 damage), count SpwC+0xd80 (exe +0xc90), base +0xd88 (exe +0xc94).
  Gun fields used by drawing: +0..+0xc pos (h,x,y), +0x10 marker timer (Rust `hits`: set 1000 for the human's planet, 1 otherwise, +1 per shot
  in the sim; `kijelolesanim` also caps it at 1000), +0x84 slot, +0x8c grow / +0x90 fade / +0x94 attacked timers (ms, UI), +0xa8 armour f32 (exe +0xa0).
- 3d spec 3.8 "scenery with +0x1e0 set": +0x1e0 is the **retreating** flag (Rust `retreating`, exe +0x1d4), not scenery. A dead unit with it set plays the
  warp-out puffs (urhajomeghalas below), not the ship explosion.
- Colour words (line3 vertices, `pixel`, `DDColor`, `Lines`) are **0xAABBGGRR, red in the low byte** (starmap.md: `DDColor` = 43 x 0x00BBGGRR; hud.md
  FadeScreen 0xTTBBGGRR). So the grid word 0x402000 = dark blue (B 0x40, G 0x20), and the "0x4040ff" below is light red.
- In `line3` (additive path) and `polygonsfader` the A byte is a **transparency** (255 = invisible): additive path does `rgb = rgb*(255-A)/256 (integer), A = 255`
  with blend (ONE, ONE); the normal path (`param_5` false) sets `A = 255 - A` with blend (SRC_ALPHA, INV_SRC_ALPHA). `pixel` ORs 0xff000000.
- `lovedekekkirak`, `RadarFrissit`, `agyukirak` are **render callbacks** (`SetRenderOther`), run during `renderscene` at their node's turn in the tree walk:
  lovedekekkirak on the effects NullBox +0x20d4 (the node that carries the 5 effect particle arrays), RadarFrissit on NullBox +0x2164 (child of mainxvb),
  agyukirak on the outer planet shell +0x18a8. Each starts with `if (+0x24b8 & 1)` (exe +0x1f2c).

### 8.2 Differences, checks, unknowns

#### 8.2.1 Exe vs ELF differences seen
- Side records (`+0xd2c`-indexed arrays): ELF stride 0x78, exe 0x6c (fighter stance at ELF `+0x44`, exe `+0x40`, space-combat.md 7.12). Layouts: slot stride 0x2D9C vs 0x3450; unit record 600 vs 0x290 (selected flag +0x181 vs +0x189, group +0x1b8 vs +0x1c4, retreating +0x1d4 vs +0x1e0, ship count +0x1b0 vs +0x1bc, owner +0x14 both); weapon slot 0x94 vs 0xb0; the tables (Epuletek, Tervek, Talalmanyok, Hajok) at other addresses; field offsets in the map of section 0.
- `GetString` index arguments exist only in the exe decompile (all indices in this spec come from it); the ELF decompile loses them.
- Belep: the exe copies whole bytes of `KepernyokLetiltva` to the backup, the ELF stores `& 1`; the exe inlines the three interpolator `Setup` calls (and a `pow` call for the Zoom value that the ELF does not show).
- Kilep: the exe tests `AktKep == 0xd` for the GFX_Starmap + SaveScreen branch directly; the ELF lists 0xd, 10, 1 but after its own redirect only 0xd can hold: same behaviour.
- `SpaceWar_Move` (§2.6): the exe (0x563450) moves the look-at and copies it into the wanted look-at at once; the ELF (0x67c5e0) calls `SpaceWar_Move0` (ELF 0x67d890) in two loops that shrink a refused step by `sign/5`. The exe has no such loop: its arena bound is the look-at clamp of §2.4.
- KeyboardHandler, main, BalClick, JobbClick, WriteRetreatWindow, screenfrissites text lines: same logic in both (checked line by line against the exe decompiles listed in the table).
- ELF `fegyvernevek` carries the group masks and names in its data; the exe file image has zeros (names filled by Belep, masks by something else).
- `fegyverszamol` (ELF 0x68c5c0, from `InitSpaceWar`) fills the weapon damages from the techs; the exe file's table holds placeholders.

#### 8.2.2 Check in the asm
1. Exe 0x57b9e0: the mapping of `DAT_00db4651/52/5C/5F/60/61/5D/5E/6A` to `Mouse_Bal/Jobb/Felenged/Lenyom/Lenyom_Bal/Lenyom_Jobb/Felenged_Bal/Felenged_Jobb/Megfogva` (inferred from the code shape; `IG2_gameUpdate` exe 0x4e0440 and `Mouse_Lekerdez` are the producers; the flag-1 cadence is settled, §4.2).
2. Exe 0x56c890 (RadarFrissit; not a Ghidra function, needs a manual function at that label): the frustum Y scale (94 / x-extent in the ELF vs 67 / y-extent for the dots), the dot colours (0xFF00 / 0xFF), `LineClip` width 2.0 and colour 0xff, the factors `(1 - g) * 6 + 1` and `* 9 + 1`; f32 vs f64 (sin/cos return f64, cast to f32).
3. Exe 0x56f420 / 0x56fa60: the `639.0 / 639.0` and `479.0 / 479.0` factors (identity), the pick radius `TransformTav(scale * 1000, z)` and the 400.0 threshold, the `0.01 < scale` test, the `abs(..) > 5` thresholds (int), the yaw divisor 100.0 and pitch divisor 200.0 (float vs double, and operand order of the difference `prev - now`).
4. Exe 0x5727d0: `gorbe` (exe 0x5839a0 returns an x87 value), `(int)((x - 20.0) + 320.0)` / `(int)((y - 20.0) + 250.0)` truncation, `SetFrame(count / 10 * 1.65)`.
5. Exe 0x574040 (infoszamol): the group loops over 0x15 weapon kinds x 10 groups, the expression `0xa9 - timer * 0xaa / timerSet` (int division; the ELF guards `timerSet != 0`, a divide-by-zero otherwise), the product over gun `+0xa8` floats at `+0x3428`, the rank average `sum / ships` (div by zero when 0 ships), the exe order of the Multi_Send 0x77 check.
6. Exe 0x577c00 / 0x57c9c0: the K/M thresholds (`> 9999`, `> 0xf3e58`), the rank thresholds 4, 0x10, 0x1e, 0x32 (signed int), the `"%d%%, %dhp"` argument order and `/ 0xf`, the slider colour packing `((0x7f - v*0x7f/100) << 8) | (alpha << 24) | (v*0x7f/100)` (signed vs unsigned division at v = 0 and v = 100), the argument order of `FUN_004aba30` (DDFillToScreen3dtRGB) and `FUN_004ab640` (DDFillToScreen3dt).
7. Exe image `fegyvernevek` (0x9269b8): masks at +0x20 of each 0x24 record and the names are zero or placeholder in the file; find what writes the masks (main, infoszamol and JobbClick read them). The ELF values are in 4.3.
8. Exe 0x562680: `GetString(0xb, 3, iVar4 - 10)` and `(0xb, 3, iVar4)` with iVar4 from 0xb (weapon names = indices 1..10, formation names = 11..20); verify against the localisation table 0xb.
9. Exe 0x56d870: weapon column on/off condition (`+0x2588`, `+0x2578`, `count[i]`), slot 9 (`side.+0x70`, `StoryAdat+152`), counters clamp 0x32 and the leaving override (`+0x2544`).
10. Exe 0x5afc50: `SpaceWarkisinfo(x, y, p, local_14)` has four arguments in the exe decompile; the ELF callee takes three (probably a decompile artefact).

#### 8.2.3 Unknowns
- (Settled: the flag-1 cadence, §4.2; the retreat window is a press-drag-release popup, §7.4.)
- `ControlConfig[0]` / `[1]` (exe 0x90B520 / 0x90B521): [0] set = hides the camera cluster hot-spots and the tooltips (new control scheme), [1] set = interpolated smooth camera; their source (ig2.settings) not read.
- Who feeds the MoveX/MoveY/Zoom/Rotate interpolators (pan keys, wheel, edge scroll): not in these functions.
- (`DoKeyboard`, ELF 0x57fd20 / exe 0x4db1a0: input.md §1 and §7.1.5.)
- Unclear fields: `ellenseg` (ELF function) meaning; unit `+0x1ac` (priority 0..4); the `g` key target `+0xd90/+0xd94`; global `chm` (ScrollLock); `+0x24d0`; the side-record fields `+0x44` (fighter stance), `+0x54` (AI flag), `+0x68` (>= 6 enables the formation phase in AddFleet, SP only), `+0x70` (can retreat); `kisinfolapmeshid` (a quad behind the info model).
- The 3D HUD parts: the control cluster model `KezeloId` (shared with the starmap) and its sub-mesh numbering; perspective projection of the weapon column (the hot-spot positions in 3.1 are exact; the drawn positions depend on the 3D camera); `kisinfoobject` file names (`tipusnevek`, `vad11.xvb`, `lap.xvb`).
- Where the starmap sets `AktKepKell = 0xb` after the zoom (entry side of the redirect): not read.

### 8.3 Unresolved
- Not read: `matrixforgat`, `RadarRangeReszKi`; the `Hajtomu` jump table 0xd588a0 (the trail kinds of §6.3.2 come from the C).
- Exe addresses marked (inf) come from the call order in exe `screenfrissites` and `initgfx`; `rescalescene`, `NezetVisszaAllitSet2`, `unloadgfx`, `gorbe`, `renderscene` exe twins were not located.
- Meaning of the property bits 0x10000, 0x10000000, 0x2000000 (depth `1 − 1/−z`, §3), 0x20c0000, 0x40, 0x10 (re/screens/render.md 2.3 documents only 1/2/4/8, 0x200, 0x800, 0x2000 and the shadow bits; 0xc and 0xf clear bits 4 and 8: §6.8.5, §1.1 step 21).
- ELF `random(n)` against the exe CRT `rand()` (MSVC LCG): the star field (`srand(0x66)`), the debris model choice and every explosion jitter differ between the binaries. The star field is reproducible
  only with the MSVC generator (only the camera's turn moves it); the explosion jitter is cosmetic.
- The second argument of `LoadBox` / `LoadMaxLevelBox` (`objnamedump::Add` flag).
- Per-unit +0x24c (ambient light colour) is set in `AddUjUrhajo` (0x68a730), not read (+0x25c..+0x264: §8.7.4).
- `SpaceWar_ZoomFinished`'s exe address (the ELF body: §6.6).

### 8.4 Check in asm
- exe 0x57f800 (`screenfrissites`): the follow-camera and the 0x229e branch use x87 for `tav()/6.0` (`DAT_0091965c = (float)(fVar11 / 6.0)`): extended division then f32 store; the ELF does an f32 division.
- exe 0x576750 (inf, `kamerabeallit`): 0.85f smoothing (ELF asm confirms f32 MULSS at 0x6a3284, constant at ELF 0xd587b4), the f64 tilt compare at ELF 0x6a2d40 (UCOMISD, tilt widened) vs the f32 store 0xbfc7c82d.
- exe 0x44f170 (CreateMatrix) call in `screenfrissites`: argument order (+0xc84 tilt, +0xc80 ^ 0x80000000 = -yaw, 0x3fc90fdb); use of the third matrix column (m02, m12, m22) for the eye offset.
- exe 0x56c420 (inf, `NezetVisszaAllitas`) / ELF 0x6a8460: mixed f32/f64 (`log`, `exp`, the `0.1 < ABS((float)L)` test, `g * (float)L` rounding order); the t formula divisions (`/ (float)(+0x2520)`, then `* mult`, then `/ 50.0`).
- ELF 0x6aa030: `floor(yaw / 6.283185307179586)` f64 wrap and the f32 literals 6.2831855 / 3.1415927 in the shortest-way logic.
- ELF 0x6815d0 / exe 0x57e420: `(float)i * 82.0f` is a float product before `sin`; 3.141592 (not pi) in the sin table; `(int)((float)(i + base) * rndbuf1[i]) % 360` truncation; (int) of an f32 alpha.
- ELF 0x694f90 / exe 0x576380: endpoints of the three segments per grid line (CONCAT44 overlays: which float gets `/ 2.0`), the axis roles of the two loops, which word is zeroed (+0x0c vs +0x1c) per segment; `(count/2)/2` integer division.
- exe 0x575050 star loop: evaluation order of the three `rand()` calls for x, y, z (the exe stores y in the low half of `local_68`, z in the high half); `% 0xff` then `/ 2` on an int.
- ELF 0x6a3380 pan chase: order `d/4`, `len = |d|/4`, cap 1.5; f32 vs f64 literals `-1.5607964`, `-0.2` (the decompile prints them without suffix).
- ELF 0x6a9120 `magassagkirak`: arguments of `vonalmetszespont3d(local_48, local_38, local_54)` and the sign of the t formula.
- ELF 0x6a1690 `urhajomeghalas`: `floor(i * 1.23 / 0.015)` f64 asteroid spin; products `S * 0.05 * 700.0` (f32 order); the counter ranges (15 < c < 18, 25 < c < 46).
- ELF 0x69ebc0 `SpaceWarLovedekKirajzol`: integer arithmetic of the alpha ramp (`(0xff - a) * (c * 0x1ff) / life`), `%` by 2 and 10; the decompile is unreliable for register-held values.
- exe 0x819978 `random` vs inline `rand() % n`: check the cast in `(rand() % 100) / 20.0` (int -> f32 division).

### 8.5 Check in asm
- exe 0x578fc4..0x578fed, 0x579c6f.. (ELF 0x682871..0x682897): all bar and bracket **y** coordinates are f64 (`cvtps2pd`, `mulsd` by 1.2 [exe 0x852300], `subsd` 1.0 [0x8522e0]) then `cvtpd2ps`; x coordinates are f32. Do not unify.
- exe 0x578d74..0x578dfb: `(int)(255 - c*127)` and `*191` are `cvttss2si` of f32; `c = 1.0 - starmaperosseg` f32 (global 0x927994).
- exe 0x57a96e..0x57a9f7: label and number positions use the last `local_54` (stale `X, Y` when no block ran): check whether the exe keeps other register values.
- exe 0x5338d0 (`line3`): the `(int)v0.tv` UV-flip test, depth `1 - zbm/-z`, the `(c*(255-A))/256` integer rgb scaling.
- exe 0x56c8e7 / 0x5839a0 (`gorbe`, x87 return, `fstp [ebp-0x1444]`): cone scale `(1-k)*6+1`, dot scale `(1-k)*9+1` (f32 vs f64).
- exe 0x56c973.. (`SikMetszespont` x4) and the factors 94/384 vs 67/384: the cone uses 94 for both axes, the dots use 94 and 67.
- exe 0x56cfa1..0x56d062: radar backdrop vertices are built from constants 0x85a410 / 0x85a420 plus the immediate 0x40000020; order LineClip (cone) -> line3 (backdrop) -> pixel (dots) -> polygonsfader (fade). Verify with a capture whether the cone shows through the 75 percent backdrop (line2d blend and queue order).
- exe 0x535570 (`pixel`): `(float)(int)x` truncation; dot buffer capacity (stack array, about 119 vertices) against the unit count (the ELF loop in 0x685290 has no bound check).
- ELF 0x69edxx / exe 0x56e1c0..0x56e5f4: the alpha ramps mix int and f32: `(age*255.0)/T` float, `(age-T/2)*0x1ff / T` int, `(T*9)/10` int, 0x9f6 = 2550; `T*10` for shot kind 3; the weapon 12 branch writes `+0x14 = 0`.
- exe 0x56e5f4..0x56e864: kind 6 `d/(d + d/5)` (two `tav` calls; constant 0.41667 for d nonzero, NaN for d == 0); kind 7 divisor 36.0 (ELF const 0xd587a4).
- ELF 0x688630 (particlerobbanas7): exe twin not located; the exe `Hajtomu` 0x567140 calls it: check which of the four `random` draws use 100 vs 20.
- exe 0x580540: `CreateMatrix2v` argument signs (`xor 0x80000000` of +0xf0 / +0xfc / +0x108 in the ELF), `MultipleMatrix(M2, M1, out)` operand order, the 0.05 and a/6 step.
- exe 0x582550: `fract((i*1.23)/0.015)` spin is f64 (`floor`) added to an f32.
- ELF 0x6e2930 / exe 0x593210: `sqrt` in f64 for the normals, `clip()` z - 2.0 and - 0.0001, colour byte scaling by `(1 - starmaperosseg)`.
- exe 0x583830 (`villamfrissit`): the pool bound `0x9260e4`, array `0x926144`, stride 0xa8, the 7 object pointers at shot+0x8c.. and the shot kind test at [shot+0x1c] (exe) vs +0x28 (ELF).

### 8.6 Unknowns
- `szar.bmp` content (the UV rule in 2.1 needs the picture), `nyilszar.bmp`, `nyilhegy.bmp`, `star.bmp`, `texcsor.bmp`: asset-finder.
- Who advances `SpwC+0x24bc` (outro timer) and its unit. Searched `spwc-elf*`, `spacewar-screen-elf*`: only `vegevizsgal` (sets 1) and Init (0) reference it.
- `kijelolesanim` (not in this decompile set): whether `+0x18c` (grow) counts up for non-selected units (then every unit shows bars for 250 ms at battle start) or only when selected; initial values of +0x18c / +0x190. It also writes the sim field attacker (+0x198) for the targets of the selected units.
- Vertex buffer `SpwC+0x21b0` (exe +0x1c54) capacity: allocated in `initgfx`; one unit draws up to 3 + 8 + 4 = 15 quads (30 vertices).
- `local_c4` offset (`-(int)(r/3)`) is dead code after the shield quad. In agyukirak the quad tu/tv are never written (untextured, garbage UVs).
- `GetString(0xb, 1)` text (jammed label) and the KiirStr colour indices 3 and 6; `DDColor[2]` (radar cone colour): asset-finder / exe table 0x8F8710.
- `SpaceWar_ZoomFinished` exe address and its caller.
- Whether the property bits and node order make the three callbacks run before or after the planet shell and the grid (draw order of the callbacks inside `renderscene`).

### 8.7 infoszamol, the selection helpers, SikMetszespont (ELF / exe)
Read from `_local/decomp/spacewar-screen-elf*`, `spwc-elf/`, `spwc-exe*/` and the disassemblies where no .c exists (ELF
`kijelolagyu` 0x6afe20, `kijelolinvertagyu` 0x6afec0, `SpaceWarMouseMove` 0x6b2c90; exe `SikMetszespont` 0x52ca10,
`kijelolescenter` 0x5772c0, `SpaceWarMouseMove` 0x56fa60). Exe offsets: stats `+0x2550..` → minus 0x58c, counters
`+0x16f8..` → `+0x15ac..`, groups `+0x30d8..` → minus 0x6a8; unit stride 0x290 / 600.

#### 8.7.1 `infoszamol` (ELF 0x6a4080 / exe 0x574040), every 6th tick
1. Always: the stats `+0x2550..+0x2568` := 0, rank `+0x256c` := −1, class `+0x2570` := 0; the snapshot `+0x2574..+0x2588`
   := the live counters (total, ships, guns, last, last kind, own; exe `+0x1fe8..+0x1ffc`); each side's firepower 0.
2. Firepower per live ship with an owner: a plan-less ship (slot list `[0] < 0`) sums its mounts' `+0x28`, else the
   plan values `Tervek[+0x1d4][Hajok[slot].design] +0x2c` over its ships; stored in the unit `+0x284` (exe `+0x250`) and
   added to its owner's side. Fighter groups add `plan +0x2c × count`; guns `TalalmanyokrEro[tech]` to the planet owner.
3. Only while something is selected (live counters): `+0x2550` = the human's live ships + its guns (all, not the
   selection); per selected live ship with an owner (any owner): ships +1, damage `+0x18`, `+0x20`, `+0x24`, max hp
   `+0x1c`, class `+0x1b8`, value = the plan values as in 2 (not `+0x284`), rank `+0x1b0 / +0x1bc` (int) summed (the
   first replaces −1); per selected gun: +1, damage, max hp, value. The weapon groups from the selected live OWN ships:
   per mount of kind k ≥ 0, barrels, enabled barrels (`+0x1c8` bit k), reload `0xa9 − timer·0xaa / timerSet` (int,
   truncating) clamped to 0..0xaa (a mount with timerSet 0 keeps the previous value; the exe starts it as a pointer,
   which clamps to 0xaa: corrects 8.2.2); then per group g the sums over its mask's kinds and the state (−1, 6, 7, 3).
   Nothing selected: the groups keep their last values.
4. The guns' armour product `+0x3428` (f32, from 1.0), `selectkisinfourhajo()`; with gfx up: the slider hot-spot 0x19
   on iff nothing is selected, the fighter stance message 0x77 while the slider differs from `side[me] +0x44` (every
   call), the small box's case-C totals (ships, fighters, firepower for own / enemy / other), the sides' flag B (`+0x70`).
5. The one-shot block (`+0x3448`, exe `+0x2d98`, set by `InitSpaceWar`) writes the sim: each side with firepower
   gets flag A (`+0`) when an enemy's (relation 1) is more than a third of its own (`own < enemy · 3`, exe 0x574ee1);
   then, through the pointer the earlier loops left on the last unit slot (an original bug, same in the exe at
   0x574ee1..0x574f2b), `Hajok[s] +4` (the ship's experience) += 1 for each of that unit's ships, once per unit slot,
   when it is alive, owned, has a ship list and its owner has flag A. The firepower of step 2 is the side's `+0x54`
   (exe; ELF `+0x58`): the screen writes it every run, and the sim's AI retreat check (`MIvisszavonulasvizsgalat`)
   reads it; the sim itself never writes it.

#### 8.7.2 Selection helpers (counters T `+0x16f8`, S `+0x16fc`, G `+0x1700`, L `+0x1704`, K `+0x1708`, O `+0x170c`)
`kijelolesleszed(snap)` (0x6ade40 / 0x5774e0): every ship and gun deselected, T = S = G = O = 0, L = −1, K = 0.
`kijelol(i)` (0x698220 / 0x576ee0): deselect all, select i, L = i, K = the hover kind `+0x2234` (stale when not from a
click), T = S = 1, O = 1 if own. `kijelolinvert(i)` (0x69a630 / 0x577770): T == 0 → `kijelol(i)`; else only own ships
toggle (T, S, O ± 1; L, K unchanged). `kijelolagyu(g)` (0x6afe20 / 0x576f50) the same for a gun (G, O if the planet is
mine). `kijelolinvertagyu(g)` (0x6afec0 / 0x577820): T == 0 → the SHIP routine `kijelol(g)` [original bug, both
binaries]; else toggles the gun when the planet is mine. `flottafelallit` (0x68eae0 / 0x573a30): every live own ship
not disabled, then EVERY gun when the planet is mine (no `+0x84` test). `csapat(n)` (0x6b3ee0 / 0x572100): only outside
a view animation; disarm the fire cursor, deselect, select every own ship of group n (no alive test). `csapatkijelol(n)`
(0x6b3e50 / 0x5721c0): with O ≠ 0 every selected ship (any owner, no alive test) gets group n.
`kijelolescenter` (0x6b1a40 / 0x5772c0) as in 2.5: confirmed (n ≥ 2: f32 sums in list order, `max tav · 2.5` in f64).

#### 8.7.3 `SikMetszespont` (ELF 0x5ee770 / exe 0x52ca10)
The inverse of `cmatrix` is a transpose; points at view depth 1 and 10 under the pixel (`(s − centre)/(fov/−z)`,
`s = 639·x/639`), to the world (`M·c + forg`); the height step `dx = B.h − A.h` clamped to at most −0.01 (f64 compare);
`out = (h, (B.y − A.y)/(dx/(h − A.h)) + A.y, same for z)`. On-plane `+0x2244`: `+0x900 < y < +0x904 && +0x908 < z <
+0x90c` (strict f32). `TransformTav(a, z) = a·fov/−z`; `TransformPoint`: `k = fov/−z`, `x·k + xcenter`.

#### 8.7.4 Unit `+0x25c/+0x260/+0x264` (exe `+0x234/+0x238/+0x23c`)
Zeroed by `AddUjUrhajo` only; `frissit2` adds to `+0x260` in f64 (types 8, 9, 10: 0.007, 0.005, 0.01). They are
`seturhajopos`' `CreateMatrix(+0x25c + π/2, +0x260 + π/2, +0x264)`. `CreateMatrix2v(a, v, 0)` (ELF 0x453690): `c =
normalise(a × v)`, `b = normalise(a × c)`, the columns `−a, −b, −c`; `MultipleMatrix(a, b, out)` (0x4518f0) is `out = b·a`.
[obs: one more `seturhajopos` on a resting ship keeps the dumped matrix, or moves it one easing step.]

#### 8.7.5 Settled
- The camera's steady state at an arena corner: `SpaceWar_Autoscroll` (§2.6).
- Who unpauses after the opening zoom: with a formation choice the column's tenth button (`+0x342e`, `SetIdoPause(false)`);
  without one nothing on the screen, only the top bar's pause button (§7.4); in MP the battle is never paused.
- `HandleRetreatWindow`: a press-drag-release popup (§7.4).

## 9. Client status

**Modules** (`engine/crates/ig2-client/src/`):
- `spacewar_screen.rs`: the scene. The battle is drawn inside the starmap's view (`starmap::View`, its background, sky,
  clouds and planets) through `starmap::Camera::look` (the look-at camera of §2 in starmap units, `xcenter` included),
  plus the ship and station models (`type_model`), poses (§6.5), grid and drop lines, overlays (§6.1, §6.2), the battle
  planet with its two cloud shells, the camera-fixed weapon column and info model; still mode from a capture dump.
- `spacewar_hud.rs`: the 2D HUD: small and big info boxes, notices and formation caption, box-select rectangle,
  retreat window, MP banner, tooltips, cursor (`MOUSE.BMP`).
- `spacewar_ui.rs`: `SpaceWarUi`: the 100 Hz tick (§7.2), `kamerabeallit`, view animations, hot-spots and latch,
  `SpwC::main`, `SpaceWarMouse`, the keys, `infoszamol`, the formation pick, `SpaceWar_Autoscroll`.
- `spacewar_fx.rs`: effect sets and spawners, projectiles, trails, explosions, debris, shield rings (shared light),
  lightning, the battle's sounds and loops.
- `spacewar_live.rs`: `Live`: entry from the starmap camera, opening and closing zoom, leaving for Options or another
  screen with the battle parked, re-entry.
- Sim side: `ig2-sim::spacebattle` (`game::realtime`, `SpBattles`); orders as `Command::SpaceOrder`, leaving a finished
  battle `Command::SpaceBattleLeft`; `infoszamol`'s sim writes travel as the battle orders `Strengths` and `Outgunned`;
  the formation pick as `Battle::pick_formation`. Tested by `ig2-net/tests/sp_battle.rs`.

**State**: accepted against the fx8 capture sequence (numbers in `re/screens/diff-results.md`); everything in §1–§7 is
ported. Orders were not compared byte-exact (no MULTIGET harness for SpaceWar); `infoszamol`'s strengths equal the
exe's between-tick writes. Tests (runtime-observed; the dump tests need `IG2_GRAB_DIR`): `infoszamol_matches_the_exe`,
`pose_keeps_the_exe_matrix_of_a_ship_at_rest`, `overview_firepower_matches_the_exe`, `autoscroll_matches_the_exe`,
`only_the_drag_spots_latch`, `the_options_zoom_out_is_five_times_faster`, `shield_rings_share_the_first_rings_light`,
`the_outer_cloud_shell_covers_the_young_shield_rings`, `an_explosion_fades_out_in_54_ticks`,
`a_trail_puff_lives_ten_ticks`; `kamerabeallit_matches_the_exe_camera` only reports.

**Captures**: `tools/trace/run_spwc.py <install> <out> --script tools/trace/scripts/spacewar_fx8.json --planets
--fuzz-designs --weapons 0x1a,0x1b,0x1c,0x1d` (flags `--records`, `--globals`, `--sounds`, `--watch-addr`, `--shield`)
stages one real-time battle on quick.scn and writes `<out>-entry.json`, the grabs and the save; then `run_screendiff.py
<install> S.sav spacewar --script S --exe-seq <out>` (the client takes the exe run's sim phase, `--sim-phase`). Stills:
`--grab-ticks T,..` and `ig2-client --snapshot OUT --save S.sav --screen spacewar --battle <grab>.json`.

**Open**:
- End zoom-out (row 380): the battle planet's disc is rosier in the exe (mean (116, 85, 79) against (88, 81, 77)); not
  the light, the fog byte or the mips; next: probe the sampled texels (vertex uv `+0x18`). With the pointer parked
  during the end zoom the exe's `MousePointerType` (0xDE01B4) is 2, the client's 3 (it hovers an enemy with own units
  selected); next: probe the exe's `+0x16f8` and `+0x2230` there.
- The client's outer cloud shell spans ~1.5 % more than the exe's (the partial shell's box re-measure is not modelled).
- The opening zoom's short info: the planet picture, the fleet marker's glow (row 213) and the planet-info cross-fade
  (row 102); the radar dots (row 104).
- The small info box's fader shine (row 273); the mouse wheel and `ControlConfig` interpolators (row 58); the V key's
  "Not Relevant Anymore" text is not drawn (row 272); the starmap's Belep catch-up ticks on the way back from Options
  are skipped (row 356, keep).
- Not compared against a capture: the MP banner, the chase camera, a selected gun's building model; the big box's
  slide runs −5 / +10 px against the exe's; property 0x2000000's depth is not modelled; the info model's turn counts
  drawn frames (frame-rate dependent, as the exe).
