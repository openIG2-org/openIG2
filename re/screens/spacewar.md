# SpaceWar screen (`AktKep` 11): the real-time space battle

Status: **part 1 done** (the screen at rest: the scene, the camera, the HUD; driven by a capture's dump), part 2 open
(input and orders on the screen, the opening/closing zoom, shots and effects, sounds; `docs/roadmap.md` §6). The
battle itself (tick, setup, end, the orders' messages) is `ig2-sim::spacebattle`, spec `re/rules/space-combat.md` §7.

Sources: the ELF decompiles `_local/decomp/spacewar-screen-elf{,2,3}/` (unstripped -O0, the names), the exe's
`_local/decomp/spacewar-screen-exe/` (`GFX_SpaceWar`, `initgfx` 0x575050, `screenfrissites` 0x57f800, the screen's
entry points) and `objdump -d` of ig2.exe for the rest; drafted by `decomp-extract`, then checked against the asm at the
spots marked "checked" and against captures (§9). ELF and exe differ in layout: the `SpwC` slot stride is 0x3450 in the
ELF, 0x2D9C in the exe (unit 0x290 / 600, gun 0xb0 / 0xa8, fighter group 0xb8 / 0xa8, shot 0xd8 / 0xa8); offsets are
ELF unless marked "exe". World points are (h, x, y), height first (as on the starmap); constants f32 unless marked f64.
Tags: [inf] inferred, [obs] observed in a capture, [u] unverified, (inf) for an exe address found by call order.

Where the drafts disagree, §8.1 (from the overlay pass, which read more of the asm) corrects §1–§3.

Outline: §0 functions; §1 scene set-up; §2 camera; §3 per-frame order; §4 entering, leaving and the hot-spots; §5 the 2D
HUD (weapon column, info boxes, radar, retreat window); §6 overlays and effects; §7 input, timers and sounds; §8
corrections, exe vs ELF, checks and unknowns; §9 the client and the captures.

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
| `bezoomolasztav` / `kijelolescenter` | 0x6a9ed0 / 0x6b1a40 | 0x571c00 (inf) / 0x571270? (unsure) |
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
| GFX_SpaceWar (loops the 10 slots, calls screenfrissites) | 0x67c710 | 0x55e850 |
| SpaceWar_Belep | 0x67bad0 | 0x562680 |
| SpaceWar_Kilep | 0x67c090 | 0x562b50 |
| SpaceWar_MouseInit (MouseInit2 is empty, ELF 0x67d880) | 0x67c7d0 | 0x562d10 |
| SpaceWar_Main (10 slots x SpwC::main) | 0x67c2c0 | 0x562cb0 |
| SpwC::main (the input handler) | 0x6aed10 | 0x57b9e0 |
| SpaceWarKeyboardHandler | 0x67e190 | 0x561ef0 |
| FRISSIT_SpaceWar2 (30 Hz) | 0x67c450 | 0x55e6e0 |
| SpwC::SpaceWarFrissit2 (per-tick HUD state) | 0x6a3380 | 0x56d870 |
| SpwC::screenfrissites (frame) | 0x6a7780 | 0x57f800 |
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
| kijelolesanim | 0x6a8b20 | 0x576fc0 |
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
| ship record: stride 0x290 (exe 600); selected +0x189 (exe +0x181); owner +0x14; group +0x1c4 (exe +0x1b8); disabled +0x1e0 (exe +0x1d4) | | |
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
| engine puff spawner particlerobbanas7 | 0x688630 | not located |
| Hajtomu | 0x69a9c0 | 0x567140 |
| SpaceVadaszKirajzol | 0x694e50 | 0x56d460 (inf) |
| urhajomeghalas / ujparticlerobbanas / robbanasdarabok | 0x6a1690 / 0x688a50 / 0x686830 | 0x582550 / ? / ? |
| frissit2 | 0x694c50 | 0x573b40 |
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
the small info model. Fog/background: none set here; the background is the starmap particle layer plus the 1000-star
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
   `ScaleObja(spwmainmeshid, s, s, s)`, s = +0x2514.
6. Four fighter-sprite models: `nemkellboxszamolas = 1; LoadBox("vad10.xvb", true)` x4 into +0x1750, +0x1798, +0x17e0,
   +0x1828 (= xvb field +0x10 of the four fighter PARTICLESTYPEs at +0x1740/0x1788/0x17d0/0x1818; exe +0x15e4/161c/1654/168c).
7. If +0xd7c != -1 (planet battle; exe +0xc8c): dirs `vector/starmap/`, `textures/starmap/`:
   - planet: `LoadBox(sprintf("b%d2.xvb", (Bolygok[planet*0x188 + 0x22] - 1) % 8), false)` (C remainder; planet idx
     +0x24b4, exe +0x1f28) -> `AddToScene(.., spwmainxvb, R*1.16, &+0x18b0)`, `RotateObj(pi/2, pi/2, 0)`, `MoveObj(P)`;
     R = (float)int +0x93c, P = (float)ints +0x930/934/938 (exe +0x858, +0x84c..854).
   - dirs back to spacewar: `LoadBox("bolygo3.xvb")` twice -> shells +0x18a0 (size R*1.02) and +0x18a8 (R*1.04):
     `RotateObj(0, pi/2, 0)`, `MoveObj(P)`, `SetObjTransparency(0.7)` (0x3f333333); the outer shell
     `SetRenderOther(agyukirak, 0)` (ELF 0x6818c0, exe LAB_00571480; planet guns; not decompiled).
   - `karikaload()` (0x694910 / 0x576e40): `LoadBox("karika.xvb")`, 20 nodes at +0x1fdc + 0xc*i: `AddToScene(.., 0.05)`,
     `SetObjShade(false)`, `RotateObj(pi/2, pi/2, 0)`, `MoveObj(0,0,0)`, `HideObj`, `PropertiesOff(0xc)`, +8 := 0
     (start tick; shield-hit rings, 3.4).
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
    virtual x = 620, y = 240 + (i*42 - 200) [y sign: check].
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

### 1.6 Drop lines (`SpaceWarhajovonalkirajzol` 0x68ecb0 / 0x570960)
`+0x1fd8 := +0x92c`. For each live unit with owner != 0 (or editor) and (owner == own empire +0xd2c, or +0x1e3 clear) one line is appended:
p0 = (h = 18*s + o_h, x = unit.x*s + o_x, y = unit.y*s + o_y), p1 = (h = unit.h*s + o_h, same x, y). Unit fields ELF +0x48 h, +0x4c x,
+0x50 y. Alpha A = 200 when unit +0x1e0 is clear, else min(255, (int)(unit.+0x10c*57 + 200)). RGB by rel[own][owner] (+0xd98: 0 same
empire, 1 allied, 2 other; InitSpaceWar: Viszony[..+0x25] == 1 -> 1 else 2): 0 -> 0x00C000, 1 -> 0x0000E0, 2 -> 0x00C0E0; both ends
A<<24 | RGB. Array room: 0x78 lines beyond the grid.

## 2. Camera
Globals (ELF / exe): `forgaspont` (target/pivot; h,x,y,w) 0x167ec50 / 0xDE0130; `kamerapont` (eye; x,y at +0, z at +8 = `DAT_0167ec68`)
0x167ec60 / 0xDE0148; `elforgatas` (yaw, CameraRotate2 only) 0x167ec70 / 0xDE0158; `DAT_0167ec74` tilt / 0xDE015c; `DAT_0167ec78`
gamma / 0xDE0160; `zbm` 0x118b220 / 0x91965C. The battle renders with `Update3D(NULL, NULL, 1)` -> `renderscene(1)` ->
`CameraRotate(kamerapont, forgaspont, 0, ...)` (the look-at variant, ELF 0x5fcf90): eye = `kamerapont` (ABSOLUTE world position),
target = `forgaspont`, up = +h axis (`CreateMatrix2v(normalise(target - eye), (1,0,0), 0)`), view point `e = M * (p - eye)`.
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
- Smoothed camera (per tick): `spwkamerapont` 0x1717f54 (x,y) / z at 0x1717f5c, `spwforgaspont` 0x1717f64 / z 0x1717f6c; targets `tkamerapont`
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

## 3. Per-frame order (`screenfrissites` ELF 0x6a7780 / exe 0x57f800); `GFX_SpaceWar` runs it for all 10 slots, each guarded by +0x24b8
1. `kisinfogfxfrissit()` (0x6860c0): the small info model. On a `kisinfotipus` change (t): free the old model; race = t/12, type = t%12 (t == 5000: fighter `vad11.xvb`; t >= 50000: building, name from
   `TTeruletek` of the planet building list; else `tipusnevek[race*0x240 + type*0x30]` with the dirs of 1.3); `AddToScene(.., spwmainxvb2, 0.03)`, `PropertiesOn(0x2000000)`, `SetRelative(false)`,
   `RotateObj(pi/2, 0, 0)`, `MoveObj(-545, 365, -1100)`, `UnhideObj`, matrix saved to `kisinfomatrix`. Every frame: `SetObjMatrix(kisinfomatrix)`, `matrixforgat(m, m, 0.05, 0.0)` (turntable),
   `SetObjTransparency(1.0 - spacewarelfade)`.
2. Mouse pointer type (`MousePointerType` 0 -> 1, 2 -> 5, 3 -> 7, 4 -> 6) into `Mouse_Tipus`.
3. `NezetVisszaAllitas()` (2.5): camera animation and fade.
4. `starmapszarok()` (3.3): the starmap background keeps animating.
5. `pulzaljspacewarcsillag()` (3.5): star twinkle.
6. Time: `+0x22f0 = SDL_GetTicks()`, `+0x22f8 = now - previous` (ms), `+0x22fc = dt / 3000.0f`, `+0x22f4 = now`.
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

Per tick (`SpaceWarFrissit2`, ELF 0x6a3380 / exe 0x56d870; `FRISSIT_SpaceWar2` runs it once per call; the sim tick is separate, space-combat.md 7.1):
`acttick2++`; unless first-frame/entering: `spwradarcounter` +1 (cap 100), or -1 (floor 0) when leaving; `futofeny += 1.0`; `spwvillogas++`; `FrissitStarmapParticles()`; `FaderDesignFrissito(starf)`;
camera input/chase (2.4); `KezeloGombFrame` +-1 within 0..10 and `SetFrame` on sub-mesh `MelyikGombot` of `KezeloId` (starmap control panel button, `GombFazis` 1 = press, 2 = release); `infoszamol()` every 5th
tick (+0x258c); icon fade (i < 10): +0x2c flag (enabled), +0x28 counter +-1 clamped 0..50 (50 = fully shown), formation-panel texts via `Mouse_Csak_Szoveg_Set`, panel switch (+0x3430 != +0x3434):
`sprintf("felallas%d", n+1)` + `loadfelallas`; `smkeringeto += 0.0002f`; `kisinfocounter++`; `kamerabeallit()`; `infoelmegy -= 0.02f` while in [0, 1] (floor 0); every 11th tick `SpeakingDirector::Refresh`.
The `frissit2` pass (0x694c50 / 0x573b40, from the sim tick): `karikafrissit(+0xd3c)`; asteroid spin by type: +0x260 += 0.007 (type 8), 0.005 (9), 0.01 (10); planet shells
`RotateObj(+0x18a0, tick/900.0f, 0, 0)` and `RotateObj(+0x18a8, -tick/1200.0f, pi/2, 0)` (tick = +0xd3c) when +0xd7c != -1; `SpaceWarSpecialpodfrissit`.

### 3.1 Ships (`seturhajopos(i)` 0x686aa0; called from the sim `Mozgatas`, `urhajomeghalas` for asteroids, `loadurhajogfx`; exe 0x580540 per space-combat.md)
Dead unit: hide the node unless ships remain (+0x184 != 0) with counter <= 41 and not scenery. Live unit: cloaked (+0x1e3) -> `SetObjTransparency(0.01)` + show; cloaked and own -> 0.6, show; cloaked foreign -> hide.
Editor: alpha 0.5, `ScaleObja(type scale)`. Heading easing: wanted = direction to the target ship / waypoint / next position; mid = normalise(normalise(wanted) + dir) -> +0x124; angle a = vektorszog(wanted, dir);
if a > 0.01 rotate dir (+0x114) about (dir x wanted) by min(a, min(0.1, a/4)) (`creatematrixfromaxis` + `rotatepointmatrix`), normalise. Then `MoveObj(node, pos(+0x48..))`.
Orientation: owner 0 (asteroid): `RotateObj(spin.x (+0x14c), spin.y (+0x150) + 3.1415927f, spin.z (+0x154))`; ships: `M1 = CreateMatrix2v(dir, -up, 0)`, `M2 = CreateMatrix(+0x25c + pi/2, +0x260 + pi/2, +0x264)`, `+0xe8 = M2 x M1`; bank
angle `speed(+0x10c) * +0x168 * 20 * 2.5` about the mid heading, eased per frame by min(angle/6, 0.05); `SetObjMatrix(node, +0xe8)`.
Engine trails (`Hajtomu` 0x69a9c0): per live uncloaked unit: node matrix and position (x1024), thruster offsets from `hajtomuvek[type*0x3c + race*0x2d0]` (5 offsets scaled by `tipusnevek.scale*1000`), effect by trail kind
(1..4, 5) through jump table 0xd588a0 (not read).

### 3.2 Fighters (`SpaceVadaszKirajzol` 0x694e50 / 0x56d460; sprites are particles in the sets at +0x1720 + 8*class)
Per group j: p = set[g.+0xb0][g.+0xb4] (4 sets by owner relation class). Alive and j != aktvadasz: `p.xyz = g.pos * s`, `p.size(+0x14) = p.size(+0x10) * s`, `p.+0x18..0x24 = g.dir (+0x48..)`, `p.+0x28 = 0`; else `p.+0x14 = 0` (hidden).
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
(space-combat.md 7.5). The effects come from `urhajomeghalas` (0x6a1690 / 0x582550), per tick on a dying unit (counter c = +0x184; libc `rand()`, cosmetic; S = type scale = `tipusnevek[..+0x10]`;
q = InvertMatrix(cmatrix) applied to (0, 0, S*400) = a point toward the camera):
- ship (+0x1e0 clear): c in 1..9: 40 % per tick: p = pos + (rand(100) - 50)/100 + q -> `ujparticlerobbanas(set 0, S*0.05*700, p)`, sound `SpWar_Hangok[8 + rand(6)]` (volume field 0x37), 2 x `robbanasdarabok(p, S*r/25, 1.5/r)`,
  r = rand(100)/100*5 + 1. c in 16..17: p = pos + (rand(100) - 50)/80 + q, size S*0.1*700, debris (S*r/18, 3.0/r), sound as above. c == 20 and gfx loaded: sound `SpWar_Hangok[rand(3)]` (the big boom).
  c in 26..45: 40 %: q*2, offset /50, set = rand(2), size S*0.05*700, debris (S*r/18, 3.0/r), sound. c > 45: c := 0.
- scenery with +0x1e0 set: c < 2: `ujparticlerobbanas(0, S*0.4*700, pos)` + sound; 2 <= c < 20: `ujparticlerobbanas(0, (1/(c+5))*S*700, pos)` + sound.
- `ujparticlerobbanas(set, size, p)` (0x688a50): next ring slot (`+0x1690 + 4*set`, array set `+0x1528 + 0x48*set`, `maxhanyefektparticle[set]` slots, wraps): pos = p * s; colour word +0xc = 0x00800000; size +0x14 =
  +0x10 = size * s * 1.3 * 11 * 4; +0x2c = 0, +0x30 = 1.0 (frame), velocity +0x18.. = normalise(random) * 0.0 (zero), +0x34 = 0.88, +0x38 = 0.98, +0x3c = 0.01, +0x40..+0x48 = 0.
  Set 0 uses `animated.bmp`, set 1 `anim2.bmp` (25-frame animations).
- `robbanasdarabok(p, size, speed)` (0x686830): first free debris node of 40 (+0x18 == 0): size = min(size, 0.002); `ScaleObja(size)`, `MoveObj(p)`; spin (+8, +0xc, +0x10) = (rand(200) + 50)/400 * speed each with a random sign
  (rand(2)); +0x18 := 1. `Darabok()` (0x69a7d0), per tick for each of 40: counter 0 -> hide; else show, `MoveObj(pos*1024 + spin)`, `RotateObjr(spin)`, `SetObjTransparency(0)` while counter <= 30 else `(counter - 30) * 0.02`, counter++, > 80 -> 0.
- Fighter death (`vadaszmeghalas` 0x6a2300 / `VadaszRobban` 0x69c1a0) not read.

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
  If the target is 0xd (options): `KilepesElott2 = KilepesElott = 0; GFX_Starmap(); SaveScreen(2); OptionsHarcFolyik = 1` (exe 0xBAE24F). (The ELF also lists 10 and 1 in that test; after the redirect they can never hold, so same behaviour.)
  Battle not over (`+0x2546` clear, exe +0x1fba): `nemlehetidotallitani = 1; SetIdoPause(true, false)`; `DeInitSounds(SpWar_Hangok, 0x2a)`; return (the battle continues, e.g. after options).
  Battle over: SP -> `SpwC::SpaceWarVege` (ELF 0x6a1560 / exe 0x5706f0); MP -> `Multi_Send(0x44d, EN, 0)` (space-combat.md 7.13); `spacewarbanvanbarkacs = 0`;
  `KepernyokLetiltva[i] = backKepernyokLetiltva[i] & 1`; `nemlehetidotallitani = 0`; `DeInitSounds`.

### 4.2 Hot-spots (SpaceWar_MouseInit, ELF 0x67c7d0 / exe 0x562d10)
Globals first: `MouseSzovegSzam = 0x2e` (exe 0xDB4680), `MouseHanyasIkonKep = 0x1f` (exe 0xDB4684). `colour` (arg 2, stored at +0x7c): 0 = the rectangle is the
hot-spot; non-zero = the pixel value of mask kep 0x1f that selects it (research.md section 4: `Mouse_Darabol` ELF 0x5b7a40; the lowest id wins).
`flag` (+0x74): 1 = acts once on the mouse change (assumed: on release), 0 = the handler runs every frame while the button is held [u, see section 7].
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
2D parts are text, rectangles, lines and fader boxes. No picture is blitted from kep 0x1f (it is only the hit mask). No top bar buttons exist here (ids 2..5 are off).
Frame, only when `+0x24b8` (gfx up):
1. `kisinfogfxfrissit` (ELF 0x6860c0 / exe 0x5778c0): cursor kind: `MousePointerType` 0 -> `Mouse_Tipus` 1; 2 -> 5; 3 -> 7; 4 -> 6 (exe 0xDB9414 / 0xDE01B4).
   The info ship model (3D): `kisinfotipus` (ELF global; chosen by `selectkisinfourhajo`: max over the selected ships of `class*12 + type`, else 50000 + building id of the
   selected planet guns, else 5000 = "vad11.xvb" fighter) -> `vector/spacewar/<tipusnevek[...]>` (types below 8; others `vector/talalmanyok/`); added at scale 0.03,
   rotated 1.5707964 about x, placed at (-545, 365, -1100), spun by `matrixforgat(.., 0.05)` per frame, transparency `1 - spacewarelfade`.
2. `NezetVisszaAllitas` (camera animation, 3D), `starmapszarok` (starmap borders; 3D), `pulzaljspacewarcsillag` (star pulse).
3. Time: `+0x22f0 = SDL_GetTicks()`, `+0x22f8 = dt ms`, `+0x22fc = dt/3000`.
4. Retreat window: if `!Mouse_Bal && SPWRetreatWindow` -> `HandleRetreatWindow`; if `SPWRetreatWindow` -> `ShowRetreatWindow` (the box), later `WriteRetreatWindow` (the text, last).
5. `fegyverpanelfrissit` (weapon column, 3D + hot-spot placement), `kijelolesanim(dt)` (selection ring animations), `animracs` (empty).
6. `+0x2268 += dt`, `+0x226c += dt` (press timers); if `!IdoPause`: `animtick += dt/3000*3000`.
7. `xcenter = +0x254c`; `KeretBeallit` (empty); `SetBoxLine`; `magassagkirak` (height marker: while height mode `+0x2241`, a "lap.xvb" quad (mesh +0x20ec) at the chosen point and a line (`+0xd00` vertices) in colour 0xff00 = green; hidden otherwise).
8. `GFX_DateMoneyScrollFade` (tooltip box, only ControlConfig[0] == 0).
9. Camera for the 3D pass (see the 3D half): normal view = `spwkamerapont`; chase camera if `spwcameramode != 0 && aktvadasz >= 0 && vadaszlinked` and not in a view animation; in a view animation: orbit camera `CreateMatrix(pitch, -yaw, pi/2)` around `+0xd4c` at distance `+0xd78`. `zbm = dist/6`. `Update3D(0, 0, 1)` renders the scene including the callbacks (RadarFrissit, lovedekekkirak, agyukirak) and the starmap layers that call `SpaceWarkisinfo/SpaceWarnagyinfo` (section 3.2).
10. `setsounds` (weapon loop sounds, 3D half), `SpaceWarMouse` (input, section 4).
11. If the intro flag `+0x2524` (set by initgfx): `+0x2520 = 0x50` frames, pitch target -0.8 (0xbf4ccccd), distance `bezoomolasztav() * scale(+0x2514)`, look-at targets 0, `NezetVisszaAllitSet`, intro flag cleared, fade-in flag `+0x2545 = 1`, `+0xd6c = round(+0x24d4 / (pi/2)) * (pi/2)` (view yaw snapped to a quarter turn), `visszaallitasidoszorzo = 5.0` if `spwoldkep == 0xd` else 1.0, all 20 screens locked, `nemlehetidotallitani = 1`, `spwckezdotimer = SDL_GetTicks()`.
12. Text lines (font size 2, len 70, flags 6, colour 2, x = 200, right aligned inside 200..620), `KiirStrSzinSzorzo = (int)(spacewarelfade * 100)` for all (restored after):
    - `+0x3438` set: `sprintf(GetString(0xb,1,2), 0)` at y = 30 (own ships retreating); then y += 10 per line printed.
    - `+0x3439` set: `GetString(0xb,1,3)` (enemy ships retreating).
    - `+0x342d` (formation phase): `GetString(0xb,1,4)` with the colour multiplier additionally scaled by `weapon slot 0 counter * 100 / 50` (fades with the column).
    - MP and `+0xf28 > 1` (more than one side) and `now - spwckezdotimer < 9000` ms: `"%s (%s) vs. %s (%s)"` = `Birodalom_Nev[side a]`, `PlayerNev[a]` (stride 0xb), same for b, passed through `a2az` (ELF 0x4f24c0; text conversion helper, exe address not found), at (200, 30), size 2, flags 4, colour 2; multiplier = `min(100, (9000 - elapsed) / 5)` percent of the fade. So it fades out over the first 9 s.
13. If `SPWRetreatWindow`: `WriteRetreatWindow` (section 3.4).

### 5.1 The weapon column (fegyverpanelfrissit ELF 0x6a7150 / exe 0x5727d0; SpaceWarFrissit2 for the counters)
Ten slots i = 0..9 (meshes: `ikon<i>.xvb` button at scale 0.02, `ora.xvb` reload clock at 0.014, `no.xvb` "off" overlay at 0.014; vector/spacewar/ and textures/spacewar/; created in initgfx at x = 300, y = 42*i - 200 (+15 for i = 9), z = -600).
Per tick (SpaceWarFrissit2, 30 Hz; only when no intro/fade and `+0x2545` clear): target on = `!(+0x342e) && ( +0x342d || ( count[i] != 0 && +0x2588 != 0 && +0x2578 != 0 ) )`; slot 9 (Retreat): `!(+0x342e) && ( +0x342d || ( side[+0xd2c].+0x70 != 0 && StoryAdat+152 == 0 ) )`;
forced off while leaving (`+0x2544`). Counter `c = slot+0x28`: +1 if on else -1, clamped 0..50 (50 ticks = 1.67 s). When c hits 0 while `+0x342e`: formation phase ends (`+0x342e = +0x342d = 0`), the icon materials are restored and the tooltips reset to `fegyvernevek[i]` (`Mouse_Csak_Szoveg_Set`, hot-spot id 3 + i).
Per frame: `f = (1 - gorbe(1 - c/50)) * 50`; `x = f*150/50 + 150` (150 closed -> 300 open), `y = 42*i - 200` (+15 for slot 9), `z = -200 - f*400/50` (-200 closed -> -600 open); all three meshes `MoveObj(x + 320 - xcenter*639/639, y, z)` (xcenter = 320).
Icon frame `SetFrame(+8 mesh, (maxreload[i]/10) * 1.65)` (the reload clock), mesh +8 shown with transparency 0. Not in formation mode: state[i] (0x3150): 3 -> overlay (+0x10) alpha 0.75 shown (some mounts of the group disabled); 6 -> alpha 0.5 shown (all disabled); 7 -> overlay hidden (all enabled); -1 (no weapon of the group) -> unchanged. `panelkiirsetszin(state)` stores it at +0x24a8.
c == 0: meshes hidden, hot-spot off (`Mouse_Szoveg_Off(3 + i)`); else button shown and hot-spot on, repositioned by `Mouse_Csak_Pos_Set(3 + i, (int)(x - 20 + 320), (int)(y - 20 + 250), -1, -1)` (size stays 40x40). Open column: x = 300 -> hot-spots at x = 600, y = 30 + 42*i (+15 for i = 9), i.e. a right-edge column.
Blink: while the fire cursor is armed (`MousePointerType == 4`) the armed group `+0x24c8` blinks: `spwvillogas % 50 < 26` shown, else hidden (button, and the overlay if its state showed one).
Slot 9 never shows the clock/overlay. Formation phase: all ten hot-spots on, overlay hidden, frame 0 for the chosen formation `+0x3430` else 32, button texture `final2x.bmp`; tooltip text = `felallasnevek[i]` every tick.

### 5.2 The info boxes (called from the shared starmap 2D layer)
`StarmapGfx2D_3` (ELF 0x6d4a30 / exe 0x5aaba0) runs while `AktKep == 0xb`: `SpaceWarnagyinfo(0x140, 0x22)` and `StarmapRovidInfoKiir(0x32, 0x1a8, 0)`, which calls `SpaceWarkisinfo(x, y, flags)` first and
draws the starmap short info under it only while `spacewarelfade < 1` (starmap multiplier `(1 - spacewarelfade) * 100`). [u: what else StarmapGfx2D_3 draws during a battle: tanks/product gombs, brush at (6, 0x1ad), IO string: all conditional on starmap state; check the 2D pass in the asm.]
`infoszamol` (ELF 0x6a4080 / exe 0x574040) runs every 5 ticks (`+0x258c` counter) and fills the selection statistics, the weapon group arrays and the fighter slider sync (below).

**Small box `kisinfokiir(x = 50, y = 424, p = 0)`** (ELF 0x6b4220 / exe 0x577c00). Fade: `KiirStrSzinSzorzo = (int)(spacewarelfade * 100)` (restored). `KiirStrAlap(text, x, y, len = 0x13, size, flags = p + 0xc, colour)`; numeric columns use flags p + 0xe, len 4. Strings `GetString(0xb, 1, k)`.
Case A, something selected (`+0x2574 != 0`) and at least one ship (`+0x2578 != 0`, exe `0x1fec`): 
- title y+3, size 2, colour 3: one selected unit -> ship name `Tervek[owner][class]` (plan record +0x84*class, name first); if exactly one ship selected and its plan type is set: tech name `Talalmanyok[ship+0x10]` (record 0x114) at y+0xc, size 2, colour 0x21. More than one -> `sprintf(GetString(0xb,1,5), ships + guns)`.
- y+0x17 size 1 colour 0x21: owner `Birodalom_Nev[...]` (selected ship owner if kind 1; own empire for guns kind 2... exactly: kind 2 -> planet owner `+0xd7c`, kind 1 -> owner of the last selected ship, else 1).
- if selected ships != 0 and max hp (`+0x2564`) != 0: if own-selected count (`+0x2588`) == 0 the rank text is empty; else `avg = rank sum / selected` -> `GetString(0xb,1,6)` if avg < 4, `7` if < 0x10, `8` if < 0x1e, `9` if < 0x32, else `10`; negative -> "-". y+0x20: `sprintf(GetString(0xb,1,0xb), value(+0x2568), rank text)`; y+0x29: `sprintf(GetString(0xb,1,0xc), 0)` label and `DrawHpStripe(maxhp - damage, maxhp, x + 0x2c, y + 0x29, 0x42, (int)(spacewarelfade * 255))` (ELF 0x45f6a0 / exe 0x472020).
Case B, only planet guns selected (exe `0x1fec == 0`, guns `+0x257c != 0`, kind 2): title = building tech name (single gun) or `sprintf(GetString(0xb,1,0xd), count)`; owner `+0xd7c`; y+0x20 `GetString(0xb,1,0xe)` with `+0x2568`; y+0x29 `GetString(0xb,1,0xf)` and the same HP stripe.
Case C, nothing selected (`+0x2574 == 0`): battle overview. Title y+3 `GetString(0xb,1,0x10)` (size 2, colour 3); row y+0xe: label `GetString(0xb,1,0x11)` + firepower of own (x+0x2a, colour 0x29), enemies (x+0x42, colour 0x2a), others (x+0x5a, colour 0x21, only if others have any ships/fighters/firepower) as `"%d%s"`
with unit divisor 1 (max <= 9999), 1000 ("K") if max < 0xf3e59 (exe test `> 0xf3e58` -> 1e6), else 1000000 ("M"); row y+0x17 label `GetString(0xb,1,0x12)` + ship counts (`KiirInt`, len 4, size 1); row y+0x20 label `GetString(0xb,1,0x13)` + fighter counts;
then the Defend/Attack slider `v = gfxvadaszdefendattack` (0..100): gradient bar `DDFillToScreen3dtRGB(x, y + 0x2a, 0x71, 3, (alpha << 24) | ((0x7f - v*0x7f/100) << 8) | (v*0x7f/100), alpha, 0)` (exe 0x4aba30), marker `DDFillToScreen3dt(x - 1 + v*0x71/100, y + 0x28, 3, 7, colour 2, alpha, 0)` and `(x + v*0x71/100, y + 0x29, 1, 5, colour 0, alpha, 0)` (exe 0x4ab640), label `GetString(0xb,1,0x14)` at y+0x29 (size 1, colour 0x21). alpha = `(int)(spacewarelfade * 255)`. So the slider rect is x 50..163, y 466..469, matching hot-spot 0x19 (50, 461, 113 x 9).
Side counts (infoszamol): own = ships whose owner == `+0xd2c`; "enemy" = `side[owner]...[0xd98 + owner + me*0x13] == 1` (the relation matrix); others otherwise; fighters from the `0xb8`-stride array; firepower per ship `+0x284` (sum of mount firepower `+0x28` for plan-less ships, else the plan weapon values `Tervek[..]+0x2c` per hull slot), fighters `plan+0x2c * count`, guns `TalalmanyokrEro[tech]`.

**Big box `nagyinfokiir(0x140, 0x22)`** (ELF 0x6b5e50 / exe 0x57c9c0), only when `+0x2547 != 0` (big-info mode; opened by infomodeonoff/NezetVisszaAllitas). x0 = `(int)((1 - f) * 350 + 320)` with `f = +0x2548` (0 = off-screen right, 1 = centred at 320). Background box (`nagyinfofader`, ELF 0x6b26e0): `FadeKerettelKerekit((int)((1 - f) * 350 + 319), 0x21, 0x12e, 0x146, 0, 0xf, 0x14, 0x3c, 0, 10, 0xf, 0x1e, false, 0, 0, 0, 0)` (302 x 326 at y 33, translucent blue). `f` = `infoelmegy` if it is in 0..1 (it is -1 normally, set by Starmapvisszaallit; decremented 0.02 per tick while in 0..1, then 0), else the view-animation factor (inverted when `+0x229d`).
Text (font size 3, flags 0xc, labels len 0xb colour 1 at x0+10, values len 0x1a colour 0x21 at x0+0x62): rows only if the last selected unit is a live ship (kind 1): y = 0x22 + 10: `GetString(0xb,1,0x15)` / ship name; +0x1a: `0x16` / tech name `Talalmanyok[+0x10]`; +0x2a: `0x17` / owner; +0x3a: `0x18` / fleet name (flotta, via the ship fleet index); then rows start at y = 0x22 + 0x4a, else at 0x22 + 10:
`sprintf("%d%%, %dhp", hp*100/maxhp, ...)` with label `GetString(0xb,1,0x19)` (hull, from `+0x2564/+0x2560`; the hp shown is `(max - damage)/0xf`), then +0x10: `GetString(0xb,1,0x1a)` with `"%d%%"` of `100 - (+0x2558*100)/(+0x255c)` or "-" ; +0x10: `GetString(0xb,1,0x1b)` with `"%d"` of value `+0x2568`; +0x10: rank text (`GetString(0xb,1,6..10)` as above) with label `GetString(0xb,1,0x1c)`; then for a single ship with a plan: the weapon list, one row per plan weapon with count > 0 (max 10 rows, step 0x10): name `Talalmanyok[plan weapon id]` (len 0x1e, colour 1) at x0+10 and count `KiirInt(len 3, size 3, flags 0xe, colour 0x21)` at x0+0x104.

### 5.3 Radar (RadarFrissit, ELF 0x685290 / exe 0x56c890; render callback of the null box +0x2164, so it draws inside `Update3D`)
Active when `+0x24b8`. Centre (321, 421), half-size 47 x 33.5; scale x: `94 / (+0x904 - +0x900)`, y: `67 / (+0x90c - +0x908)` (arena extents). `th = +0xd6c` (snapped view yaw), `s = sin th`, `c = cos th` (f64 sin/cos, results as f32).
1. Zoom-in intro: `spwradarcounter` (0..100, +1 per tick, -1 while leaving; frozen during intro `+0x2524` and fade-in `+0x2545`); `g = gorbe(counter / 100)`, `k = (1 - g) * 6 + 1` for the view frame, `(1 - g) * 9 + 1` for the dots.
2. View frustum: the plane (`SikMetszespont`, height `+0x928 * +0x2514 + +0x24f0`) is intersected with the four screen corners (0,0), (640,0), (0,480), (640,480); each point (plane y, z) gives `a = -y`, `b = z`; `X = (c*a + s*b) * 94 / (+0x904 - +0x900) + 321`, `Y = (c*b - s*a) * 94 / (+0x904 - +0x900) + 421` (the Y of the frustum uses the x extent and 94, the dots use 67 and the y extent: an original quirk or a decompile slip, check in the asm). While leaving (`+0x2544`) the last frame's corners (`lastcs`) are reused. Corners scaled by k (about the origin, as written), clipped to the radar window `HatarX 0x112..0x16f, HatarY 0x184..0x1c6` and drawn as four `LineClip(.., 2.0, 0xff)` lines (width 2, colour 0xff).
3. Background: `line3` quad from (274, 388) to (368, 455), z 0.5, colour 0x40000020 (ARGB; alpha 0x40, dark blue).
4. Dots: every live unit with `owner == EN || (+0x1e3 clear)` (ELF; the exe unit offset differs), position (y, z) = unit +0x4c / +0x50 (ELF): `X = (-c*y + s*z) * 94 * k2 / (+0x904 - +0x900) + 321`, `Y = (c*z + s*y) * 67 * k2 / (+0x90c - +0x908) + 421` (k2 = the dot zoom `(1 - g) * 9 + 1`); drawn only if `274 < X < 368` and `388 < Y < 455`; one pixel each (`pixel(.., n, false, true)`, up to the buffer size), colour 0xFF00 for units of the viewer empire (`+0xd2c`), 0xFF for all others (the decompile shows these as float denormals 9.14768e-41 and 3.57331e-43; byte order of the colour unchecked).
5. Battle-end fade (`+0x24bc != 0 && > 0x2134`): full-screen black quad (0,0)-(640,480) via `polygonsfader`, alpha `((0x5dc - (timer - 0x2134)) * 255) / 0x5dc`, min 1: 1500 ms ramp starting 8500 ms after the end timer started.

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
4. **Backdrop**: `line3` with one vertex pair (274,388) - (368,455), z 0.5, colour **0x40000020** (A 0x40, R 0x20: dark red), normal blend with alpha 0xbf (75 %), no texture, no depth test. Drawn *after* the cone lines
   (so the cone is dimmed if line2d and line3 are queued in order; the exe has the same order).
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
   Then `line3(..., texture szar.bmp, n, depth_test = true, additive = true)` (units are depth tested with `z = 1 - zbm/-Z`).
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
- Visibility: dead unit: `HideObj` unless gfx loaded, `+0x184` nonzero, `+0x184 <= 0x29` and not retreating (then `UnhideObj`). Live: not cloaked -> `SetObjTransparency(0.01)` + show; cloaked and own -> 0.6 + show; cloaked foreign -> hide. Editor: transparency 0.5 and `ScaleObja(type scale from DAT_011907e0[type*0x30])`.
- `MoveObj(node, pos(+0x48..+0x50))`. Owner 0 (asteroid): `RotateObj(node, spin.x(+0x14c), spin.y(+0x150) + 3.1415927, spin.z(+0x154))`.
- Ships (owner nonzero): `bank e = speed(+0x10c) * turn(+0x168) * 20 * 2.5`. `M1 = CreateMatrix2v(heading(+0x114..), v2 = (-m[0][2], -m[1][2], -m[2][2]) i.e. the previous matrix elements +0xf0, +0xfc, +0x108 sign-flipped, 0.0)`,
  `M2 = CreateMatrix(+0x25c + pi/2, +0x260 + pi/2, +0x264 + 0)` (pi/2 = 1.5707964f), `matrix(+0xe8..) = MultipleMatrix(M2, M1)`.
  If `|m(+0xe8)| >= 0.01 || |e| >= 0.01` and `type.turn(+0x24)` is nonzero: target `Mt = MultipleMatrix(CreateMatrix(pi/2, pi/2, 0), CreateMatrix2v(heading, (1,0,0), 0))`; if `|e| >= 0.01`:
  `axis = normalise(cross(midHeading(+0x124), (+0xf0, +0xfc, +0x108)))`, `Mt = Mt * creatematrixfromaxis(axis, -e)`. Compare column 0 of `Mt` (elements 0, 3, 6) with column 0 of the current matrix (+0xe8, +0xf4, +0x100):
  angle `a = vektorszog(Mt.col0, cur.col0)`; if `|a| > 0.01`: axis = normalise(cross(Mt.col0, cur.col0)) (x axis if null), `step = min(a, min(0.05, a/6))`, `cur = cur * creatematrixfromaxis(axis, -step)`.
  Then `SetObjMatrix(node, +0xe8)`. The bank builds up over frames because the next frame up vector is the previous matrix third column.
- (The heading easing is in space-combat.md 7.2 / §1–§3 3.1.)

### 6.6 Small ones
- `frissit2` (ELF 0x694c50 / exe 0x573b40), when gfx is loaded: `karikafrissit(tick)`; per live unit by type: 8 -> `+0x260 += 0.007`, 9 -> `+= 0.005`, 10 -> `+= 0.01` (the asteroid third rotation angle; f32 += double constant, f32 store);
  planet battle (+0xd7c >= 0): `RotateObj(shell1 +0x18a0, tick/900.0f, 0, 0)`, `RotateObj(shell2 +0x18a8, -tick/1200.0f, 1.5707964, 0)`; always `SpaceWarSpecialpodfrissit`.
- `SpaceWar_ZoomFinished` (ELF 0x67fc50): if `ControlConfig[0]` and `ControlConfig[1]` (two option bytes: smooth rotate and zoom): `Rotate_Interpolator.SetValue(SpwC+0xd70)` and `Zoom_Interpolator.SetValue(SpwC+0xd78)`
  (`cValueInterpolator2::SetValue` stores `+0x24 = v` and recalculates): snaps the mouse-wheel interpolators to the camera final angle and zoom after the zoom animation. Exe address not located.

## 7. Input, timers and sounds
### 7.1 Input

#### 7.1.0 Frame structure
`IG2_gameUpdate` (ELF 0x587400 / exe 0x4e0440) runs frames until the mouse state changes, then calls `SpaceWar_Main` once per outer iteration (cadence: section 7). `Mouse_Lekerdez` + `Mouse_Darabol` give `AktIkon` (the hot-spot under the pointer; `Mouse_Megfogva` keeps the previous one while a button stays down). Flags: `Mouse_Bal/Jobb` (held), `Mouse_Lenyom(_Bal/_Jobb)` (press edge), `Mouse_Felenged(_Bal/_Jobb)` (release edge); exe 0xDB4651/52, 0xDB465F/60/61, 0xDB465C/5D/5E; `Mouse_Megfogva` exe 0xDB466A.
In `SpwC::main` (only when `+0x24ba`):
- release edges: `Felenged_Bal` -> `+0x223f = 1` (left release event), `+0x223a = 0` (left held off); `Felenged_Jobb` -> `+0x223d = 1`, `+0x2238 = 0`.
- `+0x2258/+0x2260 = Mouse_x/y`.
- `AktIkon == 0x4c` (battlefield): `Lenyom_Bal` -> `+0x223e = 1` (left press event), `+0x223a = 1`, `Mouse_Megfogva = 1`; `Lenyom_Jobb` -> `+0x223c = 1`, `+0x2238 = 1`, `Megfogva = 1`.
Then (in screenfrissites, after Update3D) `SpaceWarMouse` runs the state machine (4.2) on those flags.

#### 7.1.1 Radar and the control cluster (SpwC::main ELF 0x6aed10 / exe 0x57b9e0)
| AktIkon | Action |
|---|---|
| 0x1f (radar, id 1) | every call: `dx = Mouse_x - 321`, `dy = Mouse_y - 421`, `th = +0xd6c` (f64 sin/cos to f32); wanted look-at `+0xd60 = -((cos th * dx + sin th * dy) / (94.0 / (+0x904 - +0x900)))`, `+0xd64 = (cos th * dy - sin th * dx) / (67.0 / (+0x90c - +0x908))` (exe 0xc70 / 0xc74); if `Lenyom_Bal` or `Lenyom_Jobb`: `Mouse_Megfogva = 1` (drag continues). No message, no sound. SpaceWarFrissit2 moves the look-at `0xd4c..` to the wanted point `0xd5c..` by delta/4 per tick, the step limited to length 1.5 after the /4, snapping when all three deltas < 0.01; the look-at is first clamped inside the arena minus 2 cells per axis. [u: the decompile has no button condition on the target update: hovering over the radar and moving may already move the camera; check against a capture] |
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
Box: while the left button is held: the rectangle is drawn with four `Line(.., 7, 0xff)` (colour 7) clipped to `HatarX1..HatarY2` (the full 640 x 480 viewport); the selection is rebuilt every frame: `kijelolesleszed(false)`; with Shift (`KeyShift`) the snapshot of own units is re-added first; then every live own, not yet selected, not disabled (`+0x1e0` clear) unit whose projected centre (`GetObjCenter` + `TransformPoint`, z < 0) is strictly inside the rectangle is selected (speech kind 1 for units that were not selected in the snapshot: `i % 3 + 1`, condition `+0x24b8 && unit +8 < 8 && beszedlasttick + 0x1e < acttick2`); planet guns of the human planet owner (`+0xd7c == +0xd2c`) likewise (`RotateAndTransformPoint`). Counters `+0x16f8/fc/1700/1704/1708/170c` are updated as in kijelol.
Box end: a right press during the box (`+0x223c`) cancels it (selection restored from the snapshot, counters recounted, `Box = 0; Sup = 1; Rot = 1`); when the left button goes up (`+0x223f` seen in the Box branch) `Box = 0` and, if the fire cursor was armed (`MousePointerType == 4`), it is restored (`MousePointerType = +0x24c4`, armed group and marker -1).
After the machine: if neither button is held `Sup = 0`; if `Rot && !(+0x229e)`: yaw `+0xd70 -= (+0x225c - x) / 100.0` and pitch `+0xd74 -= (+0x2264 - y) / 200.0` (f32 arithmetic; prev = last frame mouse), pitch clamped to [-1.5607964, -0.2]; then `SpaceWarMouseMove`; `prev = now`; the four event flags are cleared.
SpaceWarMouseMove (not Rot/Hgt/Box; corrected in 8.7.5): `+0x2234 = 0`; `best = 400` pixels; for each live unit whose owner == me or `+0x1e3` clear: centre projected (`GetObjCenter`, `TransformPoint`, z < 0, the 639/479 round trip), radius `max(10, TransformTav(ship type scale (+0x230 -> +0x10) * 1000, z))`; `d = tav2` (a pixel distance, `sqrt` in f64); accepted if `best > d && radius > d` (strict): `best = d`, `+0x2230 = idx`, `+0x2234 = 1`; then planet guns (`+0x84 != 0`, radius 10, `RotateAndTransformPoint`) share `best`, so a gun wins only when nearer (`+0x2234 = 2`). Nothing -> `+0x2230 = -1`, `+0x2234 = 0`.
Cursor (`MousePointerType`, only when < 4, i.e. no fire cursor armed): nothing hovered -> 0; hovered own unit (or a gun when the planet is mine) -> 2; hovered other unit or gun: 2 if nothing is selected (`+0x16f8 == 0`) or if any selected live unit is the human's enemy (`ellenseg(u)` = `rel[EN·0x13 + u.owner] == 1`: the selected unit's owner, not the hovered one's), else 3 (attack cursor). `Mouse_Tipus` per frame: 0 -> 1, 2 -> 5, 3 -> 7, 4 -> 6 (exe 0xDB9414).

#### 7.1.3 Clicks and the messages (BalClick ELF 0x6b13c0 / exe 0x566810; JobbClick ELF 0x6b0210 / exe 0x569190; weapon buttons in main)
BalClick (left release; needs `AktIkon == 0x4c`, `+0x24bc == 0` (battle not over), `!(+0x229e)`): `PlaySoundGlobal(0)`; if the fire cursor is armed: cancel it (`MousePointerType = 0`, `+0x24c4 = 0`, `+0x24c8 = +0x24cc = -1`).
- hovered kind 0: `kijelolesleszed(false)` (deselect all).
- kind 1 and the unit not disabled (`+0x1e0`): without Shift `kijelol(idx)` (deselect all, select it: `+0x16f8 = +0x16fc = 1`, `+0x1704 = idx`, `+0x1708 = kind`, `+0x170c += 1` if own) plus speech kind 1 (`idx % 3 + 1`; own unit, `+0x24b8`, `unit +8 < 8`, `beszedlasttick + 0x1e < acttick2`); with Shift: if nothing own is selected clear first, then `kijelolinvert(idx)` (toggle).
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
| ch `a` / `A` | down | `flottafelallit`: deselect, then select all own non-disabled units (+ all planet guns when I own the planet) |
| key 0x31..0x39 (top row only) | down | no Ctrl: `csapat(n - 1)`: cancel an armed fire cursor, deselect, select all own units with group == n - 1. With Ctrl: `csapatkijelol(n - 1)`: if the own-selected count `+0x170c != 0`, every selected unit (also non-own ones) gets group n - 1 (unit `+0x1c4` ELF / `+0x1b8` exe). No sound, no message. Groups start at 0 for every unit, so key 1 selects the never-assigned units |
| key 0x91 | down | `chm = !chm` (global `chm`, not gated by DEBUGX in either binary; meaning unknown) |
| arrows 0x25 / 0x27 | not editor; down and up | `+0x2304 = down ? -1.0 : 0` (left) / `+1.0` (right): yaw rate; SpaceWarFrissit2: `yaw += +0x2304 / 60` per tick |
| arrows 0x26 / 0x28 | same | `+0x2308 = -1.0` (up) / `+1.0` (down): pitch rate; per tick `pitch += +0x2308 / 250`, clamped to [-1.5607964, -0.2] |
| key 0x6b / 0x6d | down and up | `+0x2300 = +1.0` / `-1.0`: zoom (same as the zoom button). FRISSIT_SpaceWar2 clears `+0x2300/04` after every tick, so a held key only works with key-repeat events [u] |
| DEBUGX only (exe 0xBAE587): ch `q` | down | `SpwC::initgfx` |
| DEBUGX: ch `u` | down | toggle `side[EN].+0x54` (AI control of my side) |
| DEBUGX: ch `r`, not `TutorialPaused` | down | `Multi_Send(0x72, EN, 4,&AktSpaceWar, 0)` = `Retreat` |
| DEBUGX: key 8 | down | for every selected unit: `felrobban(.., 0, 0, true, true)` once per ship (`+0x1bc` count): blow up |
| editor mode only | down | ch `c`: reverse the order of the selected units; arrows: move selected units by one racs cell in y/z (Shift + up/down: x); PgUp/PgDn change the unit type (`+8`, 0..0xb); key 0x2e (Delete) deletes (with Ctrl: sets the unit single waypoint) |
No other key is handled here. Esc, space (pause), speed keys come from the shared `DoKeyboard` (ELF 0x57fd20, exe 0x4db1a0) -> `DispatchKeyboard`: not read [u].

### 7.2 Timers and animations
`FRISSIT_SpaceWar2` (ELF 0x67c450 / exe 0x55e6e0; `AktKep == 0xb && AktSpaceWarp != 0`; 30 Hz, dt 0.033333335 = 1/30 in the interpolators): `SpwC::SpaceWarFrissit2` (below), then `+0x2300 = +0x2304 = 0`. The pitch rate `+0x2308` is not cleared (neither binary): a pitch key stays set until its key-up event. If `ControlConfig[0] && ControlConfig[1]`: `Refresh(1/30)` of the four interpolators (MoveX, MoveY, Zoom, Rotate); if `!(+0x2524) && !(+0x229e)`: zoom `+0xd78 = Zoom.GetValue()`, yaw `+0xd70 = Rotate.GetValue()`; `SpaceWar_Move(MoveX.GetSpeed(), MoveY.GetSpeed())` pans the look-at (the exe and the ELF differ here, see 7.1). The interpolator inputs (pan keys / pad) are set elsewhere [u].
`SpwC::SpaceWarFrissit2` (ELF 0x6a3380 / exe 0x56d870), only with `+0x24b8`, per tick:
- `acttick2 += 1` (speech throttle clock: `beszedlasttick + 0x1e < acttick2` = 30 ticks = 1 s between voice lines); `spwradarcounter` +1 (to 100), or -1 (to 0) when leaving (`+0x2544`); frozen during intro and fade-in; `futofeny += 1`; `spwvillogas += 1` (blink counter, period 50 ticks, shown for the first 26); `FrissitStarmapParticles`; `FaderDesignFrissito`.
- No view animation (`+0x229e` clear): zoom `-= zoom / 25 * +0x2300`, clamp `< 5.0 -> 5.0`, zooming out (`+0x2300 < 0`) `> 400 -> 400`; look-at clamp and ease (4.1); `yaw += +0x2304 / 60`; `pitch += +0x2308 / 250`; pitch clamp [-1.5607964, -0.2].
- Control cluster animation: `if !Mouse_Gomb: GombFazis = 2`; `GombFazis == 1`: `KezeloGombFrame += 1`, max 10.0; `GombFazis == 2`: `-= 1`, min 0.0; then `SetFrame(sub-mesh MelyikGombot of KezeloId, KezeloGombFrame)`: the pressed button of the 3D cluster sinks over 10 ticks and rises again over 10. MelyikGombot: 0 zoom, 1 default view, 2 yaw right, 3 pitch, 4 top-down, 5 yaw left.
- `+0x258c -= 1`; below 0: `infoszamol` and `+0x258c = 5` (every 6th tick = 5 Hz).
- Weapon column counters (3.1) when not in intro/fade-in; formation file reload when `+0x3430 != +0x3434`.
- `smkeringeto += 0.0002`; `kisinfocounter += 1`; `kamerabeallit`; `infoelmegy`: if in [0, 1]: `-= 0.02` (min 0).
- `spwcounter`: after 10 -> 0 and `SpeakingDirector::Refresh(SpaceWarSpeaker)`; `StarmapLastCommandTime` counts down.
Frame timers (screenfrissites): SDL ticks `+0x22f0`; `+0x2268/+0x226c` (+= dt ms; unused by the code read); `animtick`; the view animation uses `(now - +0x22ac) / +0x2520 frames * visszaallitasidoszorzo / 50` through `gorbe` (3D half). `spwckezdotimer` (set at the end of the intro) drives the MP versus banner (9 s). The battle-end timer `+0x24bc` drives the black fade (3.3). View durations: buttons 0x46 / 0x49: 0x28 frames; info mode: 0x14; leaving to the starmap: 0x50; intro: 0x50.

### 7.3 Sounds and music
- Music: `PlayMusicSpWar(true, false)` in Belep (exe 0x540920), skipped if `SelfPlaying` or a battle is already running.
- `SpWar_Hangok` (exe 0x91a838, 42 records, volume 100; files under `dataraw/sounds/spacewar/...`): loaded by `InitSounds` in Belep, freed by `DeInitSounds` in Kilep. The HUD never plays from it; the records are played by the 3D/sim half (weapon sounds in `lovedeksound`, explosions in `urhajomeghalas`): 0..2 Destructions/BigBlast1-3, 3..7 Blast1-5, 8..13 Chainblast1-6, 14 Weapons/Paralyzator1, 15..20 Specialis/Spec1-6, 21..23 Antianyag0/1(loop 4)/2, 24..25 Bomba1/2, 26 Foton_Gun, 27 Halalsugar-kiloves, 28 Halalsugar-loop, 29..33 Laser01..05, 34 Mezongun, 35 Neutron_Gun, 36 Paralyzator0, 37 and 38 Paralyzator2 (the same file twice), 39..41 Torpedo1..3.
- HUD clicks: `PlaySoundGlobal(0)` = `Global_Hangok[0]` ingame/Click1.wav (volume 90). Played by BalClick, JobbClick (once per right release that passes the conditions), camera buttons 0x46 and 0x49 (on every call), 0x47 / 0x48 / 0x4a / 0x4b on `Mouse_Lenyom` only. Not played for: radar, small info box, slider, weapon buttons, retreat window, formation buttons. No other `PlaySound*` index occurs in these functions.
- Voice lines: `SpeakingDirector::Play(&SpaceWarSpeaker, i % 3 + 1, kind)`: kind 0 on a move order, 1 on a select (click or box), 2 on an attack order; kinds 5..9 are combat events (SpwC::dechp, not HUD). Conditions: unit owned by me, `+0x24b8`, `unit +8 < 8` (capital ship class), 30 ticks since `beszedlasttick`. `SpeakingDirector::Refresh` every 11th tick. Voice files not read (audio.md).
- The starmap zoom-out sound (`StarmapKiZoomol` plays Global 5) is not in these functions.

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
- Side records (`+0xd2c`-indexed arrays): ELF stride 0x78, exe 0x6c (fighter stance at ELF `+0x44`, exe `+0x40`, space-combat.md 7.12). Layouts: slot stride 0x2D9C vs 0x3450; unit record 600 vs 0x290 (selected flag +0x181 vs +0x189, group +0x1b8 vs +0x1c4, disabled +0x1d4 vs +0x1e0, ship count +0x1b0 vs +0x1bc, owner +0x14 both); weapon slot 0x94 vs 0xb0; the tables (Epuletek, Tervek, Talalmanyok, Hajok) at other addresses; field offsets in the map of section 0.
- `GetString` index arguments exist only in the exe decompile (all indices in this spec come from it); the ELF decompile loses them.
- Belep: the exe copies whole bytes of `KepernyokLetiltva` to the backup, the ELF stores `& 1`; the exe inlines the three interpolator `Setup` calls (and a `pow` call for the Zoom value that the ELF does not show).
- Kilep: the exe tests `AktKep == 0xd` for the GFX_Starmap + SaveScreen branch directly; the ELF lists 0xd, 10, 1 but after its own redirect only 0xd can hold: same behaviour.
- `SpaceWar_Move`: exe `FUN_00563450` moves the look-at (`+0xc60/+0xc64`) by `speed * (cos yaw, sin yaw) * (+0x1f88 * +0x844 + dist) / 600 * 0.6` and copies it into `0xc6c..0xc78` at once; the ELF `SpaceWar_Move` (0x67c5e0) calls `SpaceWar_Move0` (ELF 0x67d890) in two loops that shrink the step by `sign/5` until it is accepted. The exe has no such loop: the arena bound is then only the look-at clamp in SpaceWarFrissit2 (or the exe inlined Move0).
- KeyboardHandler, main, BalClick, JobbClick, WriteRetreatWindow, screenfrissites text lines: same logic in both (checked line by line against the exe decompiles listed in the table).
- ELF `fegyvernevek` carries the group masks and names in its data; the exe file image has zeros (names filled by Belep, masks by something else).

#### 8.2.2 Check in the asm
1. Exe 0x57b9e0: the mapping of `DAT_00db4651/52/5C/5F/60/61/5D/5E/6A` to `Mouse_Bal/Jobb/Felenged/Lenyom/Lenyom_Bal/Lenyom_Jobb/Felenged_Bal/Felenged_Jobb/Megfogva` (inferred from the code shape; `IG2_gameUpdate` exe 0x4e0440 and `Mouse_Lekerdez` are the producers), and in which frame `SpaceWar_Main` runs for flag-1 hot-spots (press or release).
2. Exe 0x56c890 (RadarFrissit; not a Ghidra function, needs a manual function at that label): the frustum Y scale (94 / x-extent in the ELF vs 67 / y-extent for the dots), the dot colours (0xFF00 / 0xFF), `LineClip` width 2.0 and colour 0xff, the factors `(1 - g) * 6 + 1` and `* 9 + 1`; f32 vs f64 (sin/cos return f64, cast to f32).
3. Exe 0x56f420 / 0x56fa60: the `639.0 / 639.0` and `479.0 / 479.0` factors (identity), the pick radius `TransformTav(scale * 1000, z)` and the 400.0 threshold, the `0.01 < scale` test, the `abs(..) > 5` thresholds (int), the yaw divisor 100.0 and pitch divisor 200.0 (float vs double, and operand order of the difference `prev - now`).
4. Exe 0x5727d0: `gorbe` (exe 0x5839a0 returns an x87 value), `(int)((x - 20.0) + 320.0)` / `(int)((y - 20.0) + 250.0)` truncation, `SetFrame(count / 10 * 1.65)`.
5. Exe 0x574040 (infoszamol): the group loops over 0x15 weapon kinds x 10 groups, the expression `0xa9 - timer * 0xaa / timerSet` (int division; the ELF guards `timerSet != 0`, a divide-by-zero otherwise), the product over gun `+0xa8` floats at `+0x3428`, the rank average `sum / ships` (div by zero when 0 ships), the exe order of the Multi_Send 0x77 check.
6. Exe 0x577c00 / 0x57c9c0: the K/M thresholds (`> 9999`, `> 0xf3e58`), the rank thresholds 4, 0x10, 0x1e, 0x32 (signed int), the `"%d%%, %dhp"` argument order and `/ 0xf`, the slider colour packing `((0x7f - v*0x7f/100) << 8) | (alpha << 24) | (v*0x7f/100)` (signed vs unsigned division at v = 0 and v = 100), the argument order of `FUN_004aba30` (DDFillToScreen3dtRGB) and `FUN_004ab640` (DDFillToScreen3dt).
7. Exe image `fegyvernevek` (0x9269b8): masks at +0x20 of each 0x24 record and the names are zero or placeholder in the file; find what writes the masks (main, infoszamol and JobbClick read them). The ELF values are in 4.3.
8. Exe 0x562680: `GetString(0xb, 3, iVar4 - 10)` and `(0xb, 3, iVar4)` with iVar4 from 0xb (weapon names = indices 1..10, formation names = 11..20); verify against the localisation table 0xb.
9. Exe 0x56d870: weapon column on/off condition (`+0x2588`, `+0x2578`, `count[i]`), slot 9 (`side.+0x70`, `StoryAdat+152`), counters clamp 0x32 and the leaving override (`+0x2544`).
10. Exe 0x5afc50: `SpaceWarkisinfo(x, y, p, local_14)` has four arguments in the exe decompile; the ELF callee takes three (probably a decompile artefact).
11. ELF vs exe `SpaceWar_Move` (7.1): which one the shipped game does.

#### 8.2.3 Unknowns
- Cadence and edge of `SpwC::main` for flag-1 hot-spots. As decompiled, the weapon buttons and the camera buttons run only on `!Mouse_Felenged` frames, while `IG2_gameUpdate` holds a "release pending" loop for flag 1. Open: does the weapon toggle fire on press or release? And the retreat window: `SPWRetreatWindow = 1` is set by a left click on button 9, `HandleRetreatWindow` runs on the first frame with the left button up, with the pointer still on the button column (x about 600, outside 401..), so as written the window would close at once unless it opens after the release. Needs a trace (Frida `SPWRetreatWindow`, exe 0xE72DD1, with `Mouse_*` flags).
- `ControlConfig[0]` / `[1]` (exe 0x90B520 / 0x90B521): [0] set = hides the camera cluster hot-spots and the tooltips (new control scheme), [1] set = interpolated smooth camera; their source (ig2.settings) not read.
- Who feeds the MoveX/MoveY/Zoom/Rotate interpolators (pan keys, wheel, edge scroll): not in these functions.
- Esc, space (pause), speed keys, function keys: the shared keyboard dispatcher (`DoKeyboard` ELF 0x57fd20, exe 0x4db1a0) was not read.
- Unclear fields: `ellenseg` (ELF function) meaning; unit `+0x1ac` (priority 0..4); the `g` key target `+0xd90/+0xd94`; global `chm` (ScrollLock); `+0x24d0`; the side-record fields `+0x44` (fighter stance), `+0x54` (AI flag), `+0x68` (>= 6 enables the formation phase in AddFleet, SP only), `+0x70` (can retreat); `kisinfolapmeshid` (a quad behind the info model).
- What else `StarmapGfx2D_3` draws while `AktKep == 0xb` (tanks/product buttons, brush at (6, 0x1ad), IO string): conditional on starmap state; read the 2D pass.
- `Mouse_Csak_Pos_Set` for the weapon buttons ignores `xcenter` (`+0x254c`), while the meshes are moved by `x + 320 - xcenter`; in the info-mode view animation xcenter changes, so the hot-spots and the drawn buttons may part while it runs.
- The 3D HUD parts: the control cluster model `KezeloId` (shared with the starmap) and its sub-mesh numbering; perspective projection of the weapon column (the hot-spot positions in 3.1 are exact; the drawn positions depend on the 3D camera); `kisinfoobject` file names (`tipusnevek`, `vad11.xvb`, `lap.xvb`).
- Where the starmap sets `AktKepKell = 0xb` after the zoom (entry side of the redirect): not read.

### 8.3 Unresolved
- Not decompiled: `lovedekekkirak` ELF 0x6823c0 (exe LAB_00578d20), `RadarFrissit` 0x685290 (exe LAB_0056c890), `agyukirak` 0x6818c0 (exe LAB_00571480), the jump-table targets of `SpaceWarLovedekKirajzol`
  (table ELF 0xd589c0, kinds 1..8) and `Hajtomu` (0xd588a0), `fegyverpanelfrissit` 0x6a7150, `villamfrissit` 0x6821c0, `matrixforgat`, `RadarRangeReszKi`. Shots, beams, lightning, selection brackets, hit bars,
  order arrows, planet guns and the radar are drawn there. The main session must decompile them (ELF names, then the ported exe addresses).
- Exe addresses marked (inf) come from the call order in exe `screenfrissites` and `initgfx`; `rescalescene`, `NezetVisszaAllitSet2`, `unloadgfx`, `gorbe`, `renderscene` exe twins were not located.
- Meaning of the property bits 0x10000, 0x10000000, 0x2000000, 0x20c0000, 0x40, 0xc, 0x10 (re/screens/render.md 2.3 documents only 1/2/4/8, 0x200, 0x800, 0x2000 and the shadow bits).
- Camera sign convention: up = +h with the default tilt -0.8 gives an eye offset of zoom*sin(tilt) < 0 in h (below the h = 0 plane); whether +h is down on screen as on the starmap needs a render comparison.
- Colour byte order of the line words (ARGB assumed from `a<<24 | rgb`); the `Lines` conversion was not read. Which byte of 0x402000 is red.
- ELF `random(n)` against the exe CRT `rand()` (MSVC LCG): the star field (`srand(0x66)`), the debris model choice and every explosion jitter differ between the binaries. The star field is reproducible
  only with the MSVC generator; the explosion jitter is cosmetic.
- Whether +0x2308 (tilt delta) is cleared by the mouse code each frame (FRISSIT_SpaceWar2 does not).
- The tick rate `SpaceWarFrissit2` runs at (the interpolators use dt = 1/30).
- The second argument of `LoadBox` / `LoadMaxLevelBox` (`objnamedump::Add` flag).
- Per-unit +0x24c (ambient light colour) and +0x25c..+0x264 (rotation fields) are set in `AddUjUrhajo` (0x68a730), not read here.
- `SpaceWar_ZoomFinished` and `nagyinfofader` not read.

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
- `villamgenesis` draw (virtual slot 3 in the ELF / slot 2 in the exe, at exe object +0x8): not read; vtable not located (the Init at exe 0x616190 has no vtable store).
- The engine particle renderer: how `+0xc` colour, `+0x2c` frame and `+0x10..+0x14` size become quads and which blend; whether A is a transparency there as in line3. Form particle textures come from the form xvb.
- `szar.bmp` content (the UV rule in 2.1 needs the picture), `nyilszar.bmp`, `nyilhegy.bmp`, `star.bmp`, `texcsor.bmp`: asset-finder.
- Who advances `SpwC+0x24bc` (outro timer) and its unit. Searched `spwc-elf*`, `spacewar-screen-elf*`: only `vegevizsgal` (sets 1) and Init (0) reference it.
- `kijelolesanim` (not in this decompile set): whether `+0x18c` (grow) counts up for non-selected units (then every unit shows bars for 250 ms at battle start) or only when selected; initial values of +0x18c / +0x190. It also writes the sim field attacker (+0x198) for the targets of the selected units.
- Vertex buffer `SpwC+0x21b0` (exe +0x1c54) capacity: allocated in `initgfx`; one unit draws up to 3 + 8 + 4 = 15 quads (30 vertices).
- `local_c4` offset (`-(int)(r/3)`) is dead code after the shield quad. In agyukirak the quad tu/tv are never written (untextured, garbage UVs).
- `GetString(0xb, 1)` text (jammed label) and the KiirStr colour indices 3 and 6; `DDColor[2]` (radar cone colour): asset-finder / exe table 0x8F8710.
- `SpaceWar_ZoomFinished` exe address and its caller.
- Whether the property bits and node order make the three callbacks run before or after the planet shell and the grid (draw order of the callbacks inside `renderscene`).

### 8.7 infoszamol, the selection helpers, SikMetszespont (part 2's reading; ELF / exe)
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
5. The one-shot block (`+0x3448`, exe `+0x2d98`) writes the sim: each side whose firepower is below a third of an
   enemy's gets flag A (`+0`); then, through a stale pointer to the last ship record (an original bug, same in the exe
   at 0x574ee1..0x574f2b), `Hajok[slot] +4` += 1 for each of that ship's slots, n times. Not ported (a screen writing
   the sim; `re/deviations.md`).
Verified: `spacewar_ui::infoszamol_matches_the_exe` (four dumps: the counters, damage, max hp, rank and the ten groups'
barrels, enabled barrels, reload and state, with the ELF table's masks).

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
Verified: `spacewar_ui::pose_keeps_the_exe_matrix_of_a_ship_at_rest` (56 resting ships of four dumps: one more
`seturhajopos` keeps the dumped matrix, or moves it one easing step).

#### 8.7.5 Unknowns left
- The camera's steady state in grab4: the dumped pivot is (−188.8, 188.8) at t300 and t800 while the exe's smoothed
  target stays at (−186.7, 186.7): something clamps the camera targets (not the ±186 of 2.4, and grab3's pivot −189.03
  is followed exactly). Check `kamerabeallit` (exe 0x576750) and `SpaceWarFrissit2`'s clamp in the asm.
- Who unpauses the battle after the opening zoom when there is no formation choice (`SpaceWarIndit` pauses in SP);
  the client unpauses at the end of the zoom [inf].
- `HandleRetreatWindow`'s trigger: "every frame without the left button" would close the window the frame after it
  opened; the client takes the next left release after the opening click's [inf].

## 9. The client (`ig2-client::spacewar_screen`, `spacewar_hud`) and the captures

### 9.1 How the screen is drawn
- **The battle lives in the starmap's scene** (§1.0): after the opening zoom (`rescalescene(2500)`, §2.5) a battle point
  `q` (h, x, y) is the starmap point `origin + q/2500` (`origin` = `starmapeltolas`, exe slot `+0x1f74`; scene scale exe
  `+0x1f88`). The client draws the starmap view (`starmap::View`, its background, sky, galaxy clouds and planets) through
  `starmap::Camera::look`, the battle's look-at camera expressed in starmap units, then adds the battle's objects to the same
  scene. Checked: the battle planet, the ships and their brackets land where the exe draws them (§9.3).
- **Camera** (`CameraRotate` ELF 0x5fcf90, `CreateMatrix2v` 0x453690, checked in the C): `d = normalise(target − eye)`,
  `c = normalise(d × (1, 0, 0))`, `b = normalise(d × c)`; view rows `c`, `−b`, `−d` (`e = R·(p − eye)`, ahead is `e.z < 0`,
  screen `600·e/−e.z + (xcenter, 250)`, +y down). Eye and target are the smoothed `spwkamerapont`/`spwforgaspont` (exe
  0xE72DD4 / 0xE72DE4, checked in exe `screenfrissites` 0x57f800) when the dump has them; else `kamerabeallit`'s targets
  from the fields (`M = CreateMatrix(tilt, −yaw, π/2)`, eye `= M·(0, 0, zoom) + pivot`, pivot exe `+0xc5c`, yaw `+0xc80`,
  tilt `+0xc84`, zoom `+0xc88`; checked in exe 0x57f800: the third column of M, f32). `zbm = |eye − target|/6`, the near plane
  `1.05·zbm`.
- **Lights**: the scene's parallel light 0x808080 along (10, 10, −10) lights the battle planet and the ships from (−10,
  −10, 10) [obs: of the eight sign choices this one fits, mean diff 7.3 against ≥ 17.6]; the control panel `kezel.xvb` under
  the ambient light only [obs: the exe's panel changes colour with the moving side lights; ambient-only is closest in both
  captured battles].
- **Colour words** (§8.1): `A<<24 | 0xBBGGRR`, red in the low byte, `A` a transparency, also for the grid and the drop lines
  [obs: frame diff 7.10 against 7.47 with A read as the opacity]. The radar window's word 0x40000020 shows navy (its 0x20
  is blue there) over the starmap's own dark minimap window (`StarmapGfx2D_1`'s `FadeScreen`), which the exe keeps
  drawing [obs: window luma 10.8 against the exe's 10.9].
- **Depth**: the grid and the drop lines are thin bands in the 3D pass (the engine's `Lines` are depth tested), the
  brackets and bars 2D quads over the scene (`line3` with depth test in the exe: not depth tested in the client), the
  camera-fixed models (the weapon column, the info box's ship) rendered into textures and laid over the scene.
- **Ship models**: the live type record each unit points at (exe unit `+0x214`) names the model and its scale (the dump's
  `statics.types`; the battle's setup leaves the station types 8/9 with other names than the exe file's `tipusnevek`);
  types ≥ 8 from `vector/talalmanyok/`. Pose: the node's matrix exe unit `+0xe4` (9 f32, `seturhajopos`) times the
  `AddToScene` scale. The overlays sit at the posed model's box centre (`GetObjCenter`).
- **The star field** (§1.1 step 19): the exe's MSVC `rand()` (`seed·214013 + 2531011`, bits 16..30) after `srand(0x66)`;
  only the camera's turn moves it.

### 9.2 Captures (`tools/trace/run_spwc.py --grab-ticks T,.. [--grab-settle F] --state-out S.sav`)
`spwc.js` stages one real-time battle on quick.scn (as for the tick traces, §8 of space-combat.md); at battle tick `T` it
stops the battle (`IdoSpeedSzorzoBattle` 0: `SetIdoPause` takes the exe off the battle screen), lets the screen settle `F`
frames (default 120: the camera's easing), reads the back buffer at `SwapBuffers` and dumps the slot's regions in the
'init' shape plus the screen's globals (`cam`, `fade`, `zbm`, `erosseg` = `starmaperosseg` 0x927994, `futofeny`,
`acttime`, `clouds`) to `<out>-t<T>.json` beside `<out>-t<T>.png`; the first grab also writes the exe's own save (the top
bar's state). `ig2-sim::spacebattle_raw` decodes the dump into a `Battle` (the trace replay's decoder, moved into the
library). `ig2-client --snapshot OUT --save S.sav --screen spacewar --battle <grab>.json` draws it;
`run_screendiff.py <install> S.sav spacewar:<grab>.json` compares (regions: topbar, view 0,20,640,340, console
210,360,220,120, info_box 5,423,162,52, radar 274,388,94,67, weapons 595,20,45,440).

### 9.3 Results (2026-10-22, two battles × two moments, 800 × 600)
| grab | frame | topbar | view | console | info_box | radar | weapons |
|---|---|---|---|---|---|---|---|
| grab3 t600 | 6.07 | 5.21 | 5.19 ✔ | 10.04 | 5.51 ✔ | 10.55 | 10.92 |
| grab3 t1200 | 6.20 | 5.21 | 5.21 ✔ | 11.37 | 5.39 ✔ | 10.57 | 10.92 |
| grab4 t300 | 8.08 | 5.33 | 8.18 | 10.34 | 6.33 | 10.18 | 8.74 |
| grab4 t800 | 8.04 | 5.33 | 8.18 | 9.84 | 6.16 | 10.22 | 8.74 |

Positions match: the planet's limb, the ships, the weapon buttons, the radar's dots and frustum, the info box's text (the
brackets within 2 px, slightly larger). What differs is light and colour (the planet a little brighter, the panel's
moving side lights, the radar's frustum line fainter, the weapon buttons brighter), the grid's look (harder edge, more
contrast), the info model's spin phase (not dumped), the cursor (exe only) and the top bar's speed buttons (§7 of hud.md:
their battle state isn't drawn). `re/deviations.md` lists them.

### 9.4 The live screen (part 2, 2026-10-22)
- **The sim** (`game::realtime`, `SpBattles`): single player's real-time battles run when the client asks
  (`sp_battles.live`; off for the replays): a real-time route starts the battle as `SpaceWarIndit` does in SP (speed 1,
  paused, `RealTimeEvent::SpaceStarted`), the slots tick only while the battle screen is up (`on_screen`, `AktKep ==
  0xB`) with the screen's fade in the end check, the strategic clock runs on beside it, the human's orders come through
  `Command::SpaceOrder` (`Game::space_order`, own units only), and leaving a finished battle's screen ends it
  (`Command::SpaceBattleLeft` → `SpaceWarVege`). Tested by `ig2-net/tests/sp_battle.rs`.
- **The screen** (`spacewar_ui::SpaceWarUi`, `spacewar_live::Live`): entered from the starmap camera at scene scale
  0.0004 (§1.1 step 3), the first frame's block and the opening zoom (§2.5, `rescalescene(2500)` at its end), the 30 Hz
  tick (§2.4, §7.2: zoom/yaw/tilt rates, the pivot's chase, the radar counter, the column's slide counters, the
  cluster's button frames, `infoszamol` every 6th, `kamerabeallit`), the per-frame selection timers (§3.6), the poses
  after each battle tick (`seturhajopos`, §6.5), the closing zoom when the end check fires or on Esc. A battle point is
  the starmap point `origin + q/2500` at every scale (`rescalescene` changes the camera's units, not the battle's
  place). Input: the hot-spots by `STAR_M.RAW`'s colours and rectangles (§4.2), `SpwC::main` (radar drag, the camera
  cluster, zoom/pitch buttons, the small info box, the stance slider, the weapon column: right arms the fire cursor,
  left toggles the group's mounts, the tenth opens the retreat window), `SpaceWarMouse` (select, shift-toggle, box
  select with its snapshot, both-button rotate, the right-drag height mode, `JobbClick`'s attack / gun / direct-fire /
  move orders with the arena clamp), the keys of §7.1.5 (arrows, numpad ± one tick per key event, A, 1–9 with Ctrl,
  DEBUGX R and Backspace). The retreat window (§5.4) is drawn and answered. Orders leave as commands in SP too.
- **Scripted runs**: `ig2-client --snapshot OUT --save S.sav --screen spacewar --battle <grab>.json --script S` puts the
  dumped battle into slot 0 as a live battle, enters from the save's starmap camera and runs the script on the 10 ms UI
  clock (sim ticks every 50 ms while unpaused, the screen's 30 Hz tick, a frame per UI tick), writing `OUT-t<T>.png`
  per grab and printing what the screen sent (orders, clicks) per tick; script keys `left right up down add subtract a
  1..9 shift`.
- **Tests** (runtime-observed, `IG2_GRAB_DIR` = the captures): `pose_keeps_the_exe_matrix_of_a_ship_at_rest`,
  `infoszamol_matches_the_exe`; `kamerabeallit_matches_the_exe_camera` reports, it checks nothing yet (the captured
  cameras are not settled, 8.7.5).
- **Acceptance so far**: the four stills of §9.3 give the same numbers after the drawing moved to the shared live/dump
  path; the exe-side battle input script and the frame-sequence comparison are part 3 (9.5).

### 9.5 Not yet (part 3)
The exe side of a battle input script (`spwc.js` staging + `screen.js`'s script frames on AktKep 11) and the
frame-sequence comparison with the client's scripted run; the sound-id tracer's (action → sound) table
(`SpWar_Hangok`, §7.3) and the screen's sounds through client audio (the client plays only the click now); the
shots, beams, lightning, engine trails, explosions, shield rings, fighters and planet-gun markers (§6); the big info
box (§5.2) and its camera mode; the formation phase's formation picking (`loadfelallas`) and its texts; the fighter
chase camera (`c`); the cursor shapes (`MousePointerType`); the small box's case B/C texts; the MP versus banner; the
starmap's fade-out (`starmapszarok`, §3.3) beyond `starmaperosseg`.
