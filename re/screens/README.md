# Screen inventory of the original client

Every screen and menu of the GOG build, how the player moves between them, the functions that draw them and take
their input, and how far `ig2-client` has got. This is the work list for the 1:1 client (ADR 0009): a screen is
**accepted** only when it passes a capture comparison against ig2.exe (`tools/trace/run_screendiff.py`, results in
§5).

Evidence: the ELF oracle `ig2` (names, addresses; `nm -C`, decompiled with `tools/ghidra/DecompileFunctions.java`),
the exe's own dispatcher `IG2_gameUpdate` (exe 0x4e0440, decompiled with `DecompileTree.java`, which gives the exe
addresses of the screen functions by call order even where the ported-symbol table has no name), and
`re/symbols/ig2exe-ported-names.tsv`. Addresses are **ELF** unless marked `exe`. `—` = not located in the exe yet.
MSVC folded identical empty functions in the exe (identical COMDAT folding): several screens' empty `_MouseInit2`
calls land on one address (exe 0x559ad0, ported as `WaitForDrawing`), and the ported name `StartDemoMode` (exe
0x4fe3c0) sits in `MainScr_Belep`'s slot.

## 1. The screen state machine

`IG2_gameUpdate` (ELF 0x587400, exe 0x4e0440) runs once and loops per frame: `HANDLEMESSAGES`, `FRISSITES`
(→ `Fake_FRISSITES`, the 20 Hz sim), `HANDLE_DrawOver`, `UpdateFrame` (render), `Mouse_Lekerdez` + `Mouse_Darabol`
(hit-test the screen's hot-spots → `AktIkon`). A screen change is a request `AktKepKell` (+ `AktPlanetKell`): the
loop sets `Atlepes` (transition) and `Belep` (enter), calls the old screen's `_Kilep`, `ScreenBetolt(AktKep)`
(exe 0x556180), then the new screen's `_Belep`, `_MouseInit`, `_MouseInit2` once and its `_Main` every frame.
`AktKep` 0x11 is a transient "wait for mouse release" state (`AktKepShadow` keeps the real one). Hot-spot ids
common to all game screens: 2 = pause toggle, 3..5 = speed ×1/×2/×3 (`PlaySoundGlobal(10)`, MP: `Multi_Send`
0x406/0x407); `MOUSE_SelectScreens` (exe 0x555470) handles the screen-select menu; keyboard input goes through
`DoKeyboard` (ELF 0x57fd20, exe 0x4db1a0) → `DispatchKeyboard` (0x57f800) → the screen's handler.
`GetScreenCanBeChanged(eScreen)` (0x550990): no screen change during a space battle (`AktKep` 0xb), a ground war on
the shown colony, or the options screen's load-only/battle states; targets ≥ 0xe are never "game" screens.

| `AktKep` | Screen | `_Belep` | `_MouseInit` / `_MouseInit2` | `_Main` (input) | `_Kilep` |
|---|---|---|---|---|---|
| 1 | Help (encyclopedia) | 0x54db70 / exe 0x4d7060 | 0x54e510 / 0x54e8b0; exe 0x4d7640 / (folded) | 0x54e260 / exe 0x4d7510 | 0x54e0e0 / exe 0x4d7440 |
| 2 | Starmap | 0x6f1e10 / exe 0x5b19a0 | 0x6feb70 / 0x7024b0; exe 0x5b99d0 / 0x5b8950 | 0x6f50a0 / exe 0x5b2b70 | 0x6f29c0 / exe 0x5b2200 |
| 3 | Colony (and the real-time ground war) | 0x4d8fd0 / exe 0x465b40 | 0x4e0b80 / 0x4e1d90; exe 0x4680e0 / 0x467990 | 0x4dadf0 / exe 0x4666d0 | 0x4d9b90 / exe 0x466390 |
| 4 | Research | 0x5e8780 / exe 0x523540 | 0x5ea4d0 / 0x5eb060; exe 0x524080 / 0x523e20 | 0x5ea1c0 / exe 0x523c10 | 0x5e8e70 / exe 0x5239b0 |
| 5 | Design (ship/vehicle designer) | 0x5074e0 / exe 0x4b1be0 | 0x50b430 / 0x50be70; exe 0x4b3810 / 0x4b34f0 | 0x509140 / exe 0x4b2190 | 0x507bb0 / exe 0x4b1f20 |
| 6 | Diplomacy | 0x529a50 / exe 0x4bf530 | 0x52b180 / 0x52b1a0; exe 0x4bfcc0 / (folded) | 0x52b160 / exe 0x4bfcb0 (empty: input in `DiplomGfx2D_1`) | 0x52a4d0 / exe 0x4bf810 |
| 7 | Spy (espionage) | 0x6c5a70 / exe 0x58cd50 | 0x6ca930 / 0x6c99c0; exe 0x58f0f0 / 0x58ec70 | 0x6c7360 / exe 0x58d410 | 0x6c5de0 / exe 0x58d250 |
| 8 | Trade | 0x7ace10 / exe 0x60a1e0 | 0x7aed30 / 0x7af340; exe 0x60b110 / 0x60aed0 | 0x7adcd0 / exe 0x60a7f0 | 0x7ad040 / exe 0x60a630 |
| 9 | Info (statistics, empire/planet/fleet lists) | 0x563fc0 / exe 0x4e7d30 | 0x5667b0 / 0x567ce0; exe 0x4e9900 / 0x4e9530 | 0x5647f0 / exe 0x4e81c0 | 0x5642f0 / exe 0x4e7fa0 |
| 10 | Messages (inbox, video messages, dialogue) | 0x5a8a20 / exe 0x507f90 | 0x5aa340 / 0x5aaa00; exe 0x508b10 / 0x508940 | 0x5a9c50 / exe 0x508560 | 0x5a95a0 / exe 0x5083d0 |
| 11 | SpaceWar (real-time space battle) | 0x67bad0 / exe 0x562680 | 0x67c7d0 / 0x67d880; exe 0x562d10 / (folded) | 0x67c2c0 / exe 0x562cb0 (→ `SpwC::main`) | 0x67c090 / exe 0x562b50 |
| 12 | (unused) | | | | |
| 13 | Options (settings, load/save, quit) | 0x5d9830 / exe 0x519790 | 0x5db920 / 0x5dbf60; exe 0x51ace0 / 0x51a8a0 | 0x5dad20 / exe 0x51a320 | 0x5daa80 / exe 0x51a100 |
| 14 | Start (multiplayer game room: players, races, ready, chat) | 0x721fa0 / exe 0x5bf4b0 | 0x724210 / 0x725a60; exe 0x5c0860 / 0x5c07a0 | 0x723140 / exe 0x5c0120 | 0x722680 / exe 0x5bfe10 |
| 15 | MainScr (title, main menu, campaign/race, scenario/skirmish, tutorial, multiplayer entry, high scores) | 0x59b120 / exe 0x4fe3c0 | 0x59d990 / 0x59e880; exe 0x4ff980 / 0x4ff7b0 | 0x59c7f0 / exe 0x4fefc0 | 0x59bec0 / exe 0x4fecc0 |
| 16 | Irc (online chat lobby) | 0x56eb20 / exe 0x4f2280 | 0x56f3d0 / 0x56fd40; exe 0x4f29d0 / (folded) | 0x56ece0 / exe 0x4f25a0 | 0x56ebe0 / exe 0x4f2500 |

The remaster wraps this in its own SDL screens (`controlscreen` hosts the game; `connectscreen` and `mmscreen`
(0x4453c0.., the Gyroscope matchmaking game list) are the remaster's online front end, `shopscreenbase` its IAP
remnant). The GOG build's MP goes through `MainScr` → `Irc`/direct IP → `Start`.

## 2. Draw and input functions per screen

Drawing: `<Screen>_Belep` registers the screen's 3D scene (`GFX_<Screen>`) and its 2D passes (`<Screen>Gfx2D_n`,
called in order after the 3D pass); `FRISSIT_<Screen>2` / `Frissit2_<Screen>` are the per-frame visual updates
(animations, fades) on the UI clock. The top bar and message line (`GFX_AllandoCallBack1/2`, `GFX_IdospeedPenzDatum`,
`GFX_Scroll`, `GFX_SelectScreens`) are drawn on every game screen (re/screens/hud.md §2).

| Screen | 3D / frame (`GFX_*`) | 2D passes | Per-frame visual update | Keyboard | Spec | `ig2-client` status |
|---|---|---|---|---|---|---|
| Top bar, message line, screen-select menu (all game screens) | `GFX_AllandoInit1/2` 0x66d080/0x66d0f0 (exe 0x5543a0/0x5543e0) | `GFX_AllandoCallBack1` 0x66c750, `…2` 0x66caf0 (exe 0x554040), `GFX_IdospeedPenzDatum` 0x66a590 (exe 0x554990), `GFX_Scroll` 0x66f270 (exe 0x5566a0), `GFX_ScrollHistory` 0x66eb30 (exe 0x556ac0), `GFX_SelectScreens` 0x66ace0 (exe 0x554d90), `GFX_DateMoneyScrollFade` 0x66d160 | `FRISSIT_Scroll2` 0x66ebd0 (exe 0x556210) | speed/pause hot-spots in `IG2_gameUpdate`; `MOUSE_SelectScreens` | hud.md §1.1–§2, messages.md §2 | 🚧 drawn: bar frame, research button, speed buttons, money, date, message line; the bar matches every capture (0.05–0.21, gamma-space 2D pass, hud.md §1.1). Missing: history drop-down, screen-select menu, tooltips, "message waiting" text, input except speed/pause |
| Starmap (2) | `GFX_Starmap` 0x6f36c0, minimap `GFX_StarmapRadar` 0x70e490, production `ProductGFX` 0x6e8870 | `StarmapGfx2D_1..4` 0x6dcb30, 0x6de1b0, 0x6d4a30, 0x6d6b20 (exe `_1` 0x5a87d0, `_3` 0x5aaba0); info texts `StarmapRovidInfoKiir` 0x70a920 (exe 0x5afc50), `StarmapHosszuInfoKiir` 0x708440 (exe 0x5ac7a0) | `FRISSIT_Starmap2` 0x719070 | `StarmapKeyboardHandler`, `StarmapGestureHandler` | starmap.md, hud.md §4 | 🚧 playable: camera, stars, planets, fleets, routes, names, selection, background, top bar; the control panel (`kezel.xvb`), the planet/fleet short info, the tanks and production buttons, the minimap; speed, pause, selection, move/attack/waypoint orders and minimap clicks through `MULTIGET_Starmap`'s commands (starmap.md §10–§11); the production view and the transfer panel with their orders (§12–§13); the radial icon menu, the planet's long info with its tax / auto-build / priority / station-sale menus and the redirect mode (§14–§16, capture diffs 6.3–14.6 per region, `ig2-client::starmap_menus`); the fleet and vehicle info panels (§19–§20, capture diffs fleetinfo/tanksinfo, `ig2-client::starmap_info`). Missing: the story icons (0x10..0x12), the stations' models in the long info, the hot-spot tooltips, the info box's planet picture, markers, wormholes. The background (§6: clouds, drift, twinkle, radar ranges, sky sphere) matches (view 2.8 in the remaster's gamma pass, render.md) |
| Colony (3) | `GFX_Colony` 0x4da050 (exe 0x478760) | `ColonyGfx2D_0..4` 0x4e3100.. (exe `_1` 0x461190, `_2` 0x461900, `_3` 0x461f80) | `FRISSIT_Colony2` 0x4ed9e0 (exe 0x474610), `Frissit2_Colony` 0x4e6010 (exe 0x477dc0) | `ColonyKeyboardHandler` 0x4dcd10 (exe 0x4628c0) | colony.md, hud.md §3 | 🚧 drawn and driven: terrain, buildings, roads, units, planet panel with its turning building, radar (click glides), names; building selection with its marker, the repair/demolish windows and the on/off switch, the build menu's pick list, the build cursor, the building's or planet's short info, PgUp/PgDn between colonies; orders through `MULTIGET_Colony` (colony.md §9–§10). Drawn through the remaster's render layer (render.md: gamma pass, the engine's light, the shadow map); capture diff 5.5–13.0 per region. The planet info mode with its menus is ported and capture-verified (colony.md §12; `colony:42 --colony-info` info_panel 5.5, settings 4.3). Missing: the small model in the building info, weather |
| Research (4) | `GFX_Research` 0x5e9180 → `Research3DUpdate` 0x5e4e10 (exe 0x51fd10) | `ResearchGfx2D_1..3` 0x5deeb0, 0x5df030 (exe 0x5221a0), 0x5e0ea0 | `FRISSIT_Research2` 0x5eb780 | `ResearchKeyboardHandler` 0x5e9aa0 | research.md | 🚧 drawn and driven (open state): console, wheels, ball, the category's models, info box, lists, labs, running project; the hit-mask hot-spots, lists, models and keys send `MULTIGET_Research`'s orders (verified); opened by the top bar's button 1005 or K; the opening/closing (`resframe`, lasers, sparks), press animations, `DesignKeret1` and tooltips (§5); capture diff 3.1–12.4 per region at rest (lists and labs match); the opening (frames 12/30/45/55), closing (40) and tooltips captured with `--anim-frame`/`--anim-mouse` (frame 1.0–6.3; diff-results.md `research-anim-captures`). The button lines are ported (§5.6). Missing: the side lights, sounds, the select-menu entry |
| Production (a starmap panel, not a screen) | `ProductGFX` 0x6e8870 (`gyartas.xvb`, head units) | `StarmapGfx2D_2` 0x6de1b0 (faders), `_3` exe 0x5aaba0 (texts), `_4` 0x6d6b20 (design list), `ProductionButtons` 0x6ea3d0, `…2` 0x6ea450 | `ShowStarmap` (frame) | `Starmap_Main` hot-spots 0xc..0x34 | starmap.md §12 | 🚧 drawn and driven (0x1d edits, planet stepping, design list); capture diff (M3 sweep 2026-10-24) frame 3.84, panels 5.0–5.4, name_plate 7.40 (background; both sides Hungarian since `client-lang`). The planet list (0x32, §17) is drawn and driven. Missing: pressed-button sink, the head units' wireframe twins |
| Transfer (a starmap panel) | `ShowStarmap` 0x6eca50 (`TransferObjs`, faders) | `StarmapGfx2D_3` exe 0x5aaba0 (texts), `ShowFlottaTankCap` 0x6d4560, `ShowBolygoTankCap` 0x6d4810 | — | `Starmap_Main` hot-spots 0x43..0x58, `TransferOff` 0x6f2810 | starmap.md §13 | 🚧 drawn and driven (0x1c transfers, split, tanks button, scroll bar); capture diff (M3 sweep 2026-10-24) left_panel 10.71 / frame 6.23 (4.54 phase-free: the faders' shine phase, deviation 208). Opened by the radial icons 1, 0xd and 0xe (§14) |
| Design (5) | `GFX_Design` 0x507ed0 (exe 0x4b4090) | `DesignGfx2D_1` 0x4fb840 (exe 0x4af440), `_2` 0x4feea0 (exe 0x4b1220) | `FRISSIT_Design2` 0x50d890 | `DesignKeyboardHandler` 0x508dc0 (exe 0x4b1840), `DesignKeyboardEscPressed` (exe 0x4b17a0) | design.md; orders: design-orders.md | 🚧 parts 1–3 done (part 3: `design_pods`, [`design.md`](design.md) §12b, §14.2; exe `Megkeres` 0x4b5890 (the ported name 0x4a3730 is a ground-war function), `SzinezPodok` 0x4b6b90, `KijelolPod` 0x4b4690, `Betolt` 0x4accf0, `DesignKeret2` 0x4cd4c0; capture `design-pods.json` matches except fader flashes and the hull; viewing + editing modes 1–3, orders 0x33/0x34 byte-exact, [`design.md`](design.md) §14.1; part 1: the viewing mode drawn and driven ([`design.md`](design.md) §14; capture diff at rest every region 0.05–3.7 except the exe's random fader flash and the hull's spin phase; 14-grab sequence and 9 sounds match); hull look done (part 3b, [`design.md`](design.md) §14.3: tank hull size, Destroyer body and lit windows; hull region 0.9–5.7); left: the exe's random fader flashes |
| Diplomacy (6) | `GFX_Diplomacy` 0x52a9d0 (exe 0x4c4c90), `Diplomacy3DUpdate` 0x51b2f0 (exe 0x4be2d0) | `DiplomGfx2D_1..3` 0x511190 (exe 0x4b89e0; also the input), 0x5198d0 (exe 0x4bc610), 0x51aeb0 | `FRISSIT_Diplomacy2` 0x52b1b0 | — | diplomacy.md | 🚧 playable: the 3D frame at rest (cages, portraits, the round frame, the leader's video), info panel, names, every proposal/answer menu and the dialogue texts; proposals and answers as lockstep commands; capture diff info 8.5, menu 9.6 (22.6 before the client picked the exe's language). Done 2026-10-23: the opening/closing animation, the special menu 0x32 and the sounds (held frames 20/50 match except info at 50; `diplomacy --script scripts/diplomacy.json`: opening t60-230 match, dipframe equal to the exe; sounds equal; `diff-results.md`). Missing: the MP-only menus 0x2e, lip sync, DEBUGX, demo mode; the sim does not write SpecialLista |
| Spy (7) | `GFX_Spy` 0x6c6080 (exe 0x586aa0), `Spying3DInit`/`Update` 0x6bad60/0x6c4b10 (exe 0x58fd10/0x590410) | `SpyGfx2D_1..3` 0x6bb6e0, 0x6bb7c0 (exe 0x588bd0), 0x6c3a10 (exe 0x58c1d0) | `FRISSIT_Spy2` 0x6d1e20 | `SpyKeyboardHandler` 0x6c5770 | spy.md | 🚧 drawn and driven: carousel, figures, names, status icons, head video, panels, planner, lists, dialogs; orders as `Command::Spy` (+ the screen's stream-0 rolls). Carousel and panels match the captures; the opening/closing animation (frames 12/30/45, closing 20 compared) and the hot-spot tooltips are drawn; the verdict roll verified on 600 exe presses. Missing: sounds |
| Trade (8) | `GFX_Trade` 0x7ad240 (exe 0x606b50) | `TradeGfx2D_1..3` 0x7aa4c0, 0x7aa4d0, 0x7abdb0 (exe `_2` 0x608c70, `_3` 0x6098a0; `_1` folds to an empty ret) | `FRISSIT_Trade2` 0x7b0180 (exe 0x605b60) | — | [`trade.md`](trade.md) | 🚧 parts 1–2 done: drawn and driven (F8 opens, F2 closes animated; offer list, quantity, destination, purchases through `Command::Buy`; [`trade.md`](trade.md) §13; capture diff `trade` 0.0–4.4, `trade-buy` part-1 regions 0.1–2.6, purchase identical to the exe, 7 sounds match); part 2 done: hull model, info card, pod list, row-frame first morph, tutorial event 0x27, Options return (`trade-pods`: card 0.65–0.85, lists 0.4–3.2, model 4.4–4.9; deviations #134, #135); missing: the material transparency on the other screens |
| Info (9) | `GFX_Info` 0x564580 (exe 0x4e5c30) | `InfoGfx2D_1..3` 0x5564f0, 0x556500, 0x556590 (exe `_3` 0x4e5cc0) | `FRISSIT_Info2` 0x5686f0 | — | info.md | 🚧 drawn and driven: the 3D frame (`info.xvb`, race heads, chart marker, triangle knob), the empire panel, spending limit, triangle texts, lines, the chart, race names; input: race hover, chart and star-log buttons; lit by the engine's lights in the gamma pass (info.md §1.1); the planet/fleet list (rows, `PutFaders`' fills under the frame, the planet and fleet models, scrolling with `AtteszMatrixokat`; info.md §6), a row's detail mode (§6.6) and the star log (§6.8); capture diff 0.05–6.0 per region in chart mode, detail mode (planet and fleet) and star-log mode, every region matches; the orders (§7): the planet settings menus (0x19, open menus capture-compared, select 0.85–1.04), the triangle (0x5b) and the limit (0x5c), `MULTIGET_Info` verified. Missing: the opening animation, the sixth left button's blank face in chart mode |
| Messages (10) | `GFX_Messages` 0x5aad50 | `MessagesGfx2D_1..3` 0x5a1d20, 0x5a1d30 (exe 0x507020), 0x5a3630 | `FRISSIT_Messages2` 0x5aad60 (exe 0x505180) | `MessagesKeyboardHandler` 0x5ab100 | messages.md | 🚧 drawn and driven: the frame (`messbck.xvb` at 15/30, the engine's lights in the gamma pass; capture diff 0.06–0.30), the inbox list and its row frame, the open message's dialogue (typed, wrapped, coloured) and answers; input: rows, arrows, answers, hurry, leave. Story hand-offs and automatic-message opening are sent/shown as orders. Missing: talking heads/videos and voices, a capture with messages (quick.scn sends none) |
| SpaceWar (11) | `GFX_SpaceWar` 0x67c710 (exe 0x55e850) | in `SpwC` (`SpwC::main`, panels `SpwC::exchangemouse`) | `FRISSIT_SpaceWar2` 0x67c450 (exe 0x55e6e0) | `SpaceWarKeyboardHandler` (exe 0x561ef0) | space-combat.md §7 (sim) | 🚧 parts 1–5 done (part 4: `ParticleFrissit` exe 0x52c500, `lovedeksound` exe 0x57adb0, `Hajtomu` exe 0x567140 / `hajtomuvek` 0x922188, effects/sounds/grid look §6.7, §9.6; part 3: frame-sequence acceptance vs the exe, `SpaceWar_Autoscroll` exe 0x5624c0 / ELF 0x67dac0, §2.6, `FRISSIT_SpaceWar2` every UI tick; table §9.5): drawn at rest from a capture's dump (frame 6.1–8.1; topbar 5.2, view 5.2/8.2, info_box 5.4–6.3; [`spacewar.md`](spacewar.md) §9) and live (camera, zoom, selection, orders, hot-spots, retreat window, entry from the starmap, §9.4); part 5 done (§6.8, §9.8: lightning, debris, rings, fighters, gun markers, boxes, tooltips, cursor, voices, fade, chase camera); missing (§9.7): zoom-time cursor and starmap names, planet-station models (dev 144), info view; formation re-placement done |
| Ground war (colony screen in war) | `GFX_Colony` + `WF::WriteStuff` | `ColonyGfx2D_*` | `Frissit2_Colony` (`PlayMusicGrWar`) | `ColonyKeyboardHandler` | ground-combat.md §11 (sim) | 🚧 units drawn on the colony; war buttons, selection, orders, rings, banner, unit info done (colony.md §11); landing clicks, landing zone, shadows, unit picture, self-destruct window, cursor shapes and camera keys done (§11.7–§11.10); missing: voices, effect models; orders ported (colony.md §9.1) |
| Options (13) | `GFX_Options` 0x5d6f30 | `OptionsGfx2D_1..3` 0x5d49a0, 0x5d71c0 (exe 0x518a30), 0x5d95b0 | `Frissit2_Options` 0x5da990 | `Options_EscPressed` 0x5dc7d0 | — | ✅ accepted 2026-10-22 (`options.md`; `ig2-client::options`; left: the faders' hover glow, deviations 68–74) |
| Start (14) | `GFX_Start` 0x726c10 | `StartGfx2D_1..3` 0x71ee80, 0x7208e0 (exe 0x5be820), 0x721610 | `FRISSIT_Start2` 0x728600 (exe 0x5bc580) | — | — | ⏳ not started; our MP room replaces it (ADR 0009 §2) |
| MainScr (15) | `GFX_MainScr` 0x59eb00, `Main3DInit` (exe 0x4f96a0), `Mainscreen_Update3D` (exe 0x500270) | `MainScrGfx2D_1..3` 0x593fc0, 0x597670 (exe 0x4fc8d0), 0x59a660 | `FRISSIT_MainScr2` 0x59f360 | `MainscrKeyboardHandler` 0x5a0a60 (exe 0x5000c0) | — | 🚧 part 2 done (race, difficulty, scenario/skirmish list, name entry, tutorial start, exit animation; MP lobby M5; `mainscr.md` §14) |
| Irc (16) | `GFX_Irc` 0x5701b0 (exe 0x4eff60) | `IrcGfx2D_1` 0x56a920 (exe 0x4eff70), `_2` 0x56d950 (exe 0x4f1990) | — | `Irc_KeyEsc` 0x570280, `Irc_KeyEnter` 0x5702c0 (exe 0x4f2420) | — | ⏳ not started; our lobby replaces it (ADR 0008/0009) |
| Help (1) | `GFX_Help` 0x54f160 | `HelpGfx2D_1..3` 0x54ad10, 0x54b300 (exe 0x4d5f00), 0x54c040 | `FRISSIT_Help2` 0x54ea30 | — | — | ⏳ not started |
| Tutorial (overlay on the game screens) | `TutorialKeretFrissit` 0x7b4440 (frame), `TutorialSzovegFrissit` 0x7b45a0 (text) | — | `TutorialFrissit` 0x7bd460 (exe 0x60c020), commands `TutorialGetNextCommand` 0x7b4bc0 (exe 0x60c570), recorded mouse `TutorialPlayMouse` 0x7b3310 | — | formats/text.md §6 (script, `.mou`) | interpreter verified (`rules/tutorial.md`, `ig2_sim::tutorial`); overlay done 2026-10-23 ([`tutorial.md`](tutorial.md), `ig2-client::tutorial_ui`; window box exe 0x611ba0 = `TutorialKeretFrissit`; window region diff 0.77-0.88; open: deviations 199-207) |
| Self-playing demo (main menu idle) | `StartDemoMode` 0x59a870 (from `MainScr_Belep`), `DemoMode_Init` (exe 0x60b7f0) | — | — | — | — | ⏳ not started |

### 2.1 Sub-states inside the screens
- **MainScr** (spec: `mainscr.md`; `Mode_*` flags set in `MainScr_Belep`): title video once per run (`title.bik` via `OpenBink`/`PlayBink`,
  skipped by `MainSkipAllAnims`), main menu (`Mode_Main`), single player (`Mode_SinglePlayerSelected`) → campaign race
  choice (`Mode_Fajvalasztas`, `InitFajvalasztas` 0x58bef0, the race videos `misc/race_0N_loop/wait.bik`, difficulty)
  → `CampaignGameStart` 0x58a8a0 (the intro video `intro_human/politikus/brutal.bik`); tutorial choice
  (`Mode_Tutorial_Valasztas` → `ElinditTutorial` 0x59c3b0 (inlined into `MainScr_Kilep` exe 0x4fecc0; the symbol row exe 0x553c20 is wrong) → `TutorialStart`); scenario / skirmish choice
  (`Mode_Scenario_Valasztas`, `InitScenarioValasztas` 0x5899a0, skirmish settings `SelectedScenario*`), high-score
  name entry (`EnterHighScoreNameMode`, `SaveHighscores` exe 0x5539f0, ELF 0x629080; re/rules/game-end.md); multiplayer (`Mode_MultiPlayerSelected`: name,
  IP entry `EnterIPAddressMode`/`MainScr_IPEntered`, IRC entry → `Irc`, host/join → `Start`); exit (`Mode_Kilepes`);
  credits (`misc/credits.bik` from `Mainscreen_Update3D` 0x58fc80 / exe 0x500270). Music `PlayMusicMenu`.
- **Options**: `OptionsMelyikMod` picks the page; sliders `OptCsuszkak` (music, sound, gamma, …), load and save slots
  (`SaveAktivSlot`, `SaveLeiro`, `LoadSave` exe 0x543c80), "are you sure to quit" (`OptionsAreYouSureToQuitMode`);
  `OptionsCsakLoadLehet` = load only (from the main menu), `OptionsHarcFolyik` = a battle is running.
  Entered from any game screen (`OptionsElottiKep` remembers where), Esc returns.
- **Starmap**: planet short/long info (`StarmapRovidInfoKiir`, `StarmapHosszuInfoKiir`), fleet info
  (`StarmapLoadInfoFlottak`), vehicles info (`StarmapTanksInfo*`), production (`ProductGFX`,
  `StarmapProductPlanetChange`), minimap, top/side view (`StarmapSetTopView/SideView`), zoom (`StarmapBeZoomol`/
  `KiZoomol`), problem list (`StarmapProblemaRendez`), planet explosion (`StarmapBolygoFelrobban`).
- **Messages**: inbox list, an opened message plays its video (`StartAnim` 0x5a1340 → `OpenBink` + `PlayMessagesHang`; next
  in chain `StartAnim_EndOfMessage`, `VegeAzElsoAnimnak`) and shows dialogue choices.
- **Game end**: `MEGHALAS` (defeat, `misc/end_gameover.bik`), `MEGNYERES` (victory, `misc/end_victory.bik`,
  `misc/end_campaign_win.bik`), `StartAnim_EndOfMessage` (`misc/end_%d.bik`, `misc/end_x.bik`), story videos
  `st_ShowAnim` 0x739500 (exe 0x602bb0; `politikus_win.bik`, … from `st_Elem_include`).

## 3. Flow

```
start ─► MainScr(15) ─ title.bik ─► main menu
           ├─ single player ─► race + difficulty ─ intro_<race>.bik ─► Starmap(2)  (campaign)
           ├─ scenario / skirmish ─► settings ─► Starmap(2)
           ├─ tutorial ─► Starmap(2) + tutorial overlay
           ├─ load ─► Options(13, load only) ─► the saved screen
           ├─ multiplayer ─► Irc(16) / direct IP ─► Start(14) ─► Starmap(2)
           ├─ credits (credits.bik)          └─ exit
game screens (2..10, 13, 1) ◄─ screen-select menu / hot-keys / F-keys ─► each other
  Starmap(2) ─ planet ─► Colony(3) ─ ground war in place
  arrival with a human side ─► SpaceWar(11) ─► back to the screen before
  any ─► Options(13) (settings, save, load, quit → MainScr)
  any ─► Help(1) ─► back (HelpElottiScreen)
  victory / defeat ─► end videos ─► MainScr(15) (high score)
```

## 4. Missing entirely in `ig2-client`

Tracked in `re/STATUS.md` (U29–U31):
- **Audio** (part 1 in the client, `re/screens/audio.md`: mixer, music, crossfade, research/messages/diplomacy/spy enter-exit sounds; the rest is part 2; part 3 done 2026-10-23: speakers, message speech, alien voices, thunder): music (`PlayMusicMain/Menu/SpWar/GrWar` 0x61f360.., `SE_Music`, SDL_mixer streams), UI and world
  sounds (`PlaySoundGlobal` exe 0x55d0b0 with 51 call sites on the starmap alone, `PlaySound`, `sounds/`), voices
  (`Speech::Play` exe 0x563d50, `PlayWav` exe 0x563e60, the colony speaker's lines, the tutorial's `PlayMp2`
  exe 0x563df0 `mfx/tutorial/*.mp2`), message audio (`PlayMessagesHang`). The per-screen sound-id tables and the tracer: `re/screens/sound.md`.
- **Video** (done 2026-10-22, spec `video.md`): the `.bik` names are played from `anim/mpeg4/*.mp4` (H.264/AAC through the remaster's `videoentity`,
  `OpenBink` exe 0x7325b0, `PlayBink` exe 0x732660): title, intros, race loops, message videos, story videos,
  endings, credits.
- **Menus before the game**: MainScr, the campaign/scenario/skirmish setup, Options (settings, load/save), Help,
  credits, high scores, the tutorial interpreter. The client starts from a save today.

## 5. Capture comparisons

Run with `tools/trace/run_screendiff.py` (ADR 0009 §4): ig2.exe through Frida (`screen.js`) and
`ig2-client --snapshot` on the same save and screen at 800 × 600, compared by `ig2-client --diff`. Images stay under
`_local/screendiff/`; only the numbers are recorded here. Frame sequences and input scripts (`--script`, per-grab diffs): `re/screens/sound.md` §3.

See `re/screens/diff-results.md` for the latest results per screen.

Widescreen: 2D stays in the centred 4:3 frame, the 3D scenes and background pictures span the full width, narrower
windows are letterboxed (`ig2_resize` exe 0x4d84d0, `ModifyViewPort` 0x4ac4f0): `re/screens/widescreen.md`, ADR 0010.
Keys and hot-spots per screen (input audit 2026-10-23): `re/screens/input.md`.

Every 3D screen goes through the remaster's own GL layer (shaders, materials, the shadow map, the `VIDEO` and
`CLOUD` materials): `re/screens/render.md`.
