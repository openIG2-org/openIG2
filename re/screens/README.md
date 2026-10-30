# Screen inventory of the original client

Every screen and menu of the GOG build, how the player moves between them, the functions that draw them and take
their input, and the state of each screen in `ig2-client` (§4). The 1:1 client follows ADR 0009: a screen is **accepted** only
when it passes a capture comparison against ig2.exe (`tools/trace/run_screendiff.py`, method and current results in
[`diff-results.md`](diff-results.md)).

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

| Screen | 3D / frame (`GFX_*`) | 2D passes | Per-frame visual update | Keyboard | Spec |
|---|---|---|---|---|---|
| Top bar, message line, screen-select menu (all game screens) | `GFX_AllandoInit1/2` 0x66d080/0x66d0f0 (exe 0x5543a0/0x5543e0) | `GFX_AllandoCallBack1` 0x66c750, `…2` 0x66caf0 (exe 0x554040), `GFX_IdospeedPenzDatum` 0x66a590 (exe 0x554990), `GFX_Scroll` 0x66f270 (exe 0x5566a0), `GFX_ScrollHistory` 0x66eb30 (exe 0x556ac0), `GFX_SelectScreens` 0x66ace0 (exe 0x554d90), `GFX_DateMoneyScrollFade` 0x66d160 | `FRISSIT_Scroll2` 0x66ebd0 (exe 0x556210) | speed/pause hot-spots in `IG2_gameUpdate`; `MOUSE_SelectScreens` | hud.md §1.1–§2, messages.md §2 |
| Starmap (2) | `GFX_Starmap` 0x6f36c0, minimap `GFX_StarmapRadar` 0x70e490, production `ProductGFX` 0x6e8870 | `StarmapGfx2D_1..4` 0x6dcb30, 0x6de1b0, 0x6d4a30, 0x6d6b20 (exe `_1` 0x5a87d0, `_3` 0x5aaba0); info texts `StarmapRovidInfoKiir` 0x70a920 (exe 0x5afc50), `StarmapHosszuInfoKiir` 0x708440 (exe 0x5ac7a0) | `FRISSIT_Starmap2` 0x719070 | `StarmapKeyboardHandler`, `StarmapGestureHandler` | starmap.md, hud.md §4 |
| Colony (3) | `GFX_Colony` 0x4da050 (exe 0x478760) | `ColonyGfx2D_0..4` 0x4e3100.. (exe `_1` 0x461190, `_2` 0x461900, `_3` 0x461f80) | `FRISSIT_Colony2` 0x4ed9e0 (exe 0x474610), `Frissit2_Colony` 0x4e6010 (exe 0x477dc0) | `ColonyKeyboardHandler` 0x4dcd10 (exe 0x4628c0) | colony.md, hud.md §3 |
| Research (4) | `GFX_Research` 0x5e9180 → `Research3DUpdate` 0x5e4e10 (exe 0x51fd10) | `ResearchGfx2D_1..3` 0x5deeb0, 0x5df030 (exe 0x5221a0), 0x5e0ea0 | `FRISSIT_Research2` 0x5eb780 | `ResearchKeyboardHandler` 0x5e9aa0 | research.md |
| Production (a starmap panel, not a screen) | `ProductGFX` 0x6e8870 (`gyartas.xvb`, head units) | `StarmapGfx2D_2` 0x6de1b0 (faders), `_3` exe 0x5aaba0 (texts), `_4` 0x6d6b20 (design list), `ProductionButtons` 0x6ea3d0, `…2` 0x6ea450 | `ShowStarmap` (frame) | `Starmap_Main` hot-spots 0xc..0x34 | starmap.md §12 |
| Transfer (a starmap panel) | `ShowStarmap` 0x6eca50 (`TransferObjs`, faders) | `StarmapGfx2D_3` exe 0x5aaba0 (texts), `ShowFlottaTankCap` 0x6d4560, `ShowBolygoTankCap` 0x6d4810 | — | `Starmap_Main` hot-spots 0x43..0x58, `TransferOff` 0x6f2810 | starmap.md §13 |
| Design (5) | `GFX_Design` 0x507ed0 (exe 0x4b4090) | `DesignGfx2D_1` 0x4fb840 (exe 0x4af440), `_2` 0x4feea0 (exe 0x4b1220) | `FRISSIT_Design2` 0x50d890 | `DesignKeyboardHandler` 0x508dc0 (exe 0x4b1840), `DesignKeyboardEscPressed` (exe 0x4b17a0) | design.md; orders: design-orders.md |
| Diplomacy (6) | `GFX_Diplomacy` 0x52a9d0 (exe 0x4c4c90), `Diplomacy3DUpdate` 0x51b2f0 (exe 0x4be2d0) | `DiplomGfx2D_1..3` 0x511190 (exe 0x4b89e0; also the input), 0x5198d0 (exe 0x4bc610), 0x51aeb0 | `FRISSIT_Diplomacy2` 0x52b1b0 | — | diplomacy.md |
| Spy (7) | `GFX_Spy` 0x6c6080 (exe 0x586aa0), `Spying3DInit`/`Update` 0x6bad60/0x6c4b10 (exe 0x58fd10/0x590410) | `SpyGfx2D_1..3` 0x6bb6e0, 0x6bb7c0 (exe 0x588bd0), 0x6c3a10 (exe 0x58c1d0) | `FRISSIT_Spy2` 0x6d1e20 | `SpyKeyboardHandler` 0x6c5770 | spy.md |
| Trade (8) | `GFX_Trade` 0x7ad240 (exe 0x606b50) | `TradeGfx2D_1..3` 0x7aa4c0, 0x7aa4d0, 0x7abdb0 (exe `_2` 0x608c70, `_3` 0x6098a0; `_1` folds to an empty ret) | `FRISSIT_Trade2` 0x7b0180 (exe 0x605b60) | — | [`trade.md`](trade.md) |
| Info (9) | `GFX_Info` 0x564580 (exe 0x4e5c30) | `InfoGfx2D_1..3` 0x5564f0, 0x556500, 0x556590 (exe `_3` 0x4e5cc0) | `FRISSIT_Info2` 0x5686f0 | — | info.md |
| Messages (10) | `GFX_Messages` 0x5aad50 | `MessagesGfx2D_1..3` 0x5a1d20, 0x5a1d30 (exe 0x507020), 0x5a3630 | `FRISSIT_Messages2` 0x5aad60 (exe 0x505180) | `MessagesKeyboardHandler` 0x5ab100 | messages.md |
| SpaceWar (11) | `GFX_SpaceWar` 0x67c710 (exe 0x55e850) | in `SpwC` (`SpwC::main`, panels `SpwC::exchangemouse`) | `FRISSIT_SpaceWar2` 0x67c450 (exe 0x55e6e0) | `SpaceWarKeyboardHandler` (exe 0x561ef0) | space-combat.md §7 (sim) |
| Ground war (colony screen in war) | `GFX_Colony` + `WF::WriteStuff` | `ColonyGfx2D_*` | `Frissit2_Colony` (`PlayMusicGrWar`) | `ColonyKeyboardHandler` | ground-combat.md §11 (sim) |
| Options (13) | `GFX_Options` 0x5d6f30 | `OptionsGfx2D_1..3` 0x5d49a0, 0x5d71c0 (exe 0x518a30), 0x5d95b0 | `Frissit2_Options` 0x5da990 | `Options_EscPressed` 0x5dc7d0 | [`options.md`](options.md) |
| Start (14) | `GFX_Start` 0x726c10 | `StartGfx2D_1..3` 0x71ee80, 0x7208e0 (exe 0x5be820), 0x721610 | `FRISSIT_Start2` 0x728600 (exe 0x5bc580) | — | [`start.md`](start.md) |
| MainScr (15) | `GFX_MainScr` 0x59eb00, `Main3DInit` (exe 0x4f96a0), `Mainscreen_Update3D` (exe 0x500270) | `MainScrGfx2D_1..3` 0x593fc0, 0x597670 (exe 0x4fc8d0), 0x59a660 | `FRISSIT_MainScr2` 0x59f360 | `MainscrKeyboardHandler` 0x5a0a60 (exe 0x5000c0) | [`mainscr.md`](mainscr.md) |
| Irc (16) | `GFX_Irc` 0x5701b0 (exe 0x4eff60) | `IrcGfx2D_1` 0x56a920 (exe 0x4eff70), `_2` 0x56d950 (exe 0x4f1990) | — | `Irc_KeyEsc` 0x570280, `Irc_KeyEnter` 0x5702c0 (exe 0x4f2420) | [`start.md`](start.md) (replaced) |
| Help (1) | `GFX_Help` 0x54f160 | `HelpGfx2D_1..3` 0x54ad10, 0x54b300 (exe 0x4d5f00), 0x54c040 | `FRISSIT_Help2` 0x54ea30 | — | [`help.md`](help.md) |
| Tutorial (overlay on the game screens) | `TutorialKeretFrissit` 0x7b4440 (frame), `TutorialSzovegFrissit` 0x7b45a0 (text) (exe window box 0x611ba0 = `TutorialKeretFrissit`) | — | `TutorialFrissit` 0x7bd460 (exe 0x60c020), commands `TutorialGetNextCommand` 0x7b4bc0 (exe 0x60c570), recorded mouse `TutorialPlayMouse` 0x7b3310 | — | formats/text.md §6 (script, `.mou`) |
| Self-playing demo (main menu idle) | `StartDemoMode` 0x59a870 (from `MainScr_Belep`), `DemoMode_Init` (exe 0x60b7f0) | — | — | — | `mainscr.md` §9.5 |

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

## 4. Client status

Every screen below is implemented and reachable in `ig2-client` (`engine/crates/ig2-client/src/`); the client starts
at the title video and the main menu, like the exe. Accepted = passed the capture comparison (`diff-results.md`).
Smaller per-screen differences are rows in `re/deviations.md`; each spec's last section ("Client status") lists them.

| Screen | Spec | Client module(s) | State |
|---|---|---|---|
| Top bar, message line, screen-select and quick-research menus | [`hud.md`](hud.md), [`messages.md`](messages.md) §2 | `hud`, `topbar_menus`, `kit/` | done |
| Starmap (2), incl. production view, transfer panel, radial menu, long info, select menus, fleet/vehicle panels, planet list 0x32 | [`starmap.md`](starmap.md) | `starmap`, `starmap_hud`, `starmap_info`, `starmap_menus`, `starmap_panels`, `starmap_planet`, `starmap_radar`, `starmap_tip` | partial: the radial menu's story icons 0x10/0x11/0x12 (gigabomb, story wormhole, virus alert) are not offered/acted on (`starmap_menus.rs`) |
| Colony (3), incl. planet info mode | [`colony.md`](colony.md) | `colony`, `colony_ui`, `colony_weather`, `colony_smoke` | done |
| Ground war (colony screen in war) | [`colony.md`](colony.md) §9.1, §11; sim `rules/ground-combat.md` §11 | `colony_war` | done |
| Research (4) | [`research.md`](research.md) | `research` | done |
| Design (5) | [`design.md`](design.md); orders `design-orders.md` | `design`, `design_edit`, `design_pods`, `design_scene` | done |
| Diplomacy (6) | [`diplomacy.md`](diplomacy.md) | `diplomacy`, `diplomacy_scene` | done (no lip sync in the original, §2) |
| Spy (7) | [`spy.md`](spy.md) | `spy_screen`, `spy_screen/`, `spy_scene` | done (the 2D video path is dead code, §1.1) |
| Trade (8) | [`trade.md`](trade.md) | `trade`, `trade_scene` | done |
| Info (9) | [`info.md`](info.md) | `info_screen` | done |
| Messages (10) | [`messages.md`](messages.md) | `messages_screen` | done |
| SpaceWar (11) | [`spacewar.md`](spacewar.md); sim `rules/space-combat.md` §7 | `spacewar_screen`, `spacewar_hud`, `spacewar_ui`, `spacewar_fx`, `spacewar_live` | done (accepted) |
| Options (13) | [`options.md`](options.md) | `options` | done (accepted) |
| Start (14) / Irc (16) | [`start.md`](start.md) | `mp_lobby`, `mp_wait`, `chat` | replaced by our MP room (ADR 0014) |
| MainScr (15), incl. race, scenario/skirmish, new game | [`mainscr.md`](mainscr.md) | `mainscr`, `mainscr_scene`, `mainscr_race`, `mainscr_scenario`, `new_game` | done |
| Help (1) | [`help.md`](help.md) | `help` | done |
| Tutorial overlay | [`tutorial.md`](tutorial.md); interpreter `rules/tutorial.md` (`ig2_sim::tutorial`) | `tutorial_ui` | partial: the tutorial's input locks (deviations 141, 174, 198) are missing |
| Video (title, intros, messages, story, endings, credits) | [`video.md`](video.md) | `video/` | done |
| Audio (music, UI/world sounds, voices, speech) | [`audio.md`](audio.md), [`sound.md`](sound.md) | `audio/` | done |
| Self-playing demo | `mainscr.md` §9.5 | — | unreachable in the GOG build (`demomode` check returns false, `demo.tut` not shipped); nothing to port |

## 5. Shared specs

- Capture comparisons: [`diff-results.md`](diff-results.md) (method, current state per screen); frame sequences and
  input scripts: [`sound.md`](sound.md) §3.
- Overlay screens (research, design, diplomacy, spy, trade, info, messages, options, help) lay a dimmed `SaveScreen`
  picture of the screen before under their scene: [`overlay-background.md`](overlay-background.md) (ADR 0015).
- Widescreen: 2D stays in the centred 4:3 frame, the 3D scenes and background pictures span the full width, narrower
  windows are letterboxed (`ig2_resize` exe 0x4d84d0, `ModifyViewPort` 0x4ac4f0): [`widescreen.md`](widescreen.md), ADR 0010.
- Keys and hot-spots per screen: [`input.md`](input.md).
- The remaster's GL layer (shaders, materials, the shadow map, the `VIDEO` and `CLOUD` materials): [`render.md`](render.md).
- Music and sounds: [`audio.md`](audio.md); the per-screen sound-id tables and the tracer: [`sound.md`](sound.md).
