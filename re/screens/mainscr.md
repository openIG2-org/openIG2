# The main menu screen (`AktKep` 15)

From the ELF decompiles (`_local/decomp/mainscr/`, `mainscr2/`) and the exe decompiles (`_local/decomp/mainscr-exe/`, `mainscr2/`);
the exe wins. Addresses are exe unless marked ELF. `[elf]` = read in the ELF only; `[guess]` = inferred; `[asm]` = checked
in the exe disassembly; `[obs]` = observed at runtime (section 13). The main menu itself has **no 2D draw list**: every
button is a 3D ring with a label texture; 2D (faders, texts) appears only in the sub-states (section 7). Time unit: "tick"
= one 10 ms UI tick (100 Hz) unless said otherwise. Port: `ig2-client::mainscr` (title + main menu + the ring menus;
the race, scenario, tutorial and multiplayer screens behind them are not ported yet, section 14).

## 0. Functions

| Part | ELF | exe |
|---|---|---|
| `MainScr_Belep` (+ `StartDemoMode` inlined at its head; the port names the exe function `StartDemoMode`) | 0x59b120 (+0x59a870) | 0x4fe3c0 |
| `MainScr_MouseInit` / `MainScr_MouseInit2` | 0x59d990 / 0x59e880 | 0x4ff980 / 0x4ff7b0 |
| `MainScr_Main` | 0x59c7f0 | 0x4fefc0 |
| `MainScr_Kilep` | 0x59bec0 | 0x4fecc0 |
| `MainscrKeyboardHandler` | 0x5a0a60 | 0x5000c0 |
| `GFX_MainScr` -> `Mainscreen_Update3D` (per drawn frame) | 0x59eb00 -> 0x58fc80 | 0x500270 |
| `Main3DInit` | 0x58d7f0 | 0x4f96a0 |
| `FRISSIT_MainScr2` (10 ms UI tick) | 0x59f360 | **0x4f65f0** (decompiled in pass 2: section 5) |
| `FRISSIT_MainScr` (20 Hz) | 0x59eff0 | 0x4f7300 (bytes: `if AktKep==15 && YouAreDisconnected(0xdb94f4) { YouAreDisconnectedVolt(0xbbd144) = 1; YouAreDisconnected = 0 }`) |
| `MainScrGfx2D_1` (faders) / `_2` (texts) / `_3` (disconnect box) | 0x593fc0 / 0x597670 / 0x59a660 | not given / 0x4fc8d0 / not given |
| `CampaignGameStart` | 0x58a8a0 | 0x4f5e90 |
| `InitFajvalasztas` / `InitScenarioValasztas` | 0x58bef0 / 0x5899a0 | inlined (see 10) |
| `SetCampaignGameOptions` / `SetScirmishGameOptions` | 0x59bcd0 / n.a. | 0x503d40 / 0x503e80 (not decompiled) |
| `TutorialSetCampaignGameOptions` | 0x59c5f0 | 0x504250 |
| `ElinditTutorial(int)` (ELF); **the exe's `ElinditTutorial` 0x553c20 is `UnLoadScenarioList`** | 0x59c3b0 | 0x553c20 |
| `MainInitSounds` / `MainDeInitSounds` / `MainPlaySound` | 0x593b60 / 0x593d40 / 0x593860 | 0x4fa9f0 / 0x4fa970 / inlined |
| Decompiled in pass 2 (`_local/decomp/mainscr2/`) | `HozzaadKorok` 0x58d2d0, `ForgatFajvalasztasDolgai` 0x58af10, `LoadFajValasztasVektor` 0x58bc90, `VisszaAllitFajvalasztasKezdoPoziciok` 0x5893a0, `HandleLoadingMesh` 0x58d670, `MainScrInitNullBoxes` 0x593f00, `MainScr_CancelIPEnter` 0x59ec00, `MainScr_ESCPressedWhenMultiplayerselect` 0x59ecc0, `GenerateUrhajoPosok` 0x58c1d0, `MainSetSoundPan` 0x5939b0, `ChangeGalaxyColors` 0x58c700, `GenerateGalaxy` 0x58cde0, `GenerateKod` 0x58c960, `DeGenerateKod` 0x58cb50, `GenerateKod2` 0x58cc60, `MainInitSounds` 0x593b60 | exe 0x4f8780 (3.1), 0x4f7390 (9.2), 0x4f9410 (9.2), 0x4f89b0 (9.2), 0x4f8620 (4), 0x4fe2c0 (3.1), 0x4fe6f0 + 0x4fe780 (6.4), 0x4f8380 (5.5), 0x500040 (8), 0x4f6040 (3.2); exe `GenerateGalaxy` 0x4f7e60, `GenerateKod` 0x4f8240 not decompiled (3.2, ELF read; Kod* from the ELF disassembly) |
| Still not decompiled | `ScenarioGombraRanyomtak` 0x59af90, `KiirFajInfoSzoveg` | exe 0x503bf0, 0x4f9040 (also `FrissitFajInfoSzoveg` 0x4f7c60, `InitKiirInfoSzoveg` 0x4f8ea0, `InfoBelerakSzoveg` 0x4f8910, `FUN_00504380`, `MozgatKor`, `ElkerFajvalasztasKezdoPoziciok`, `FejFenyekBeallitasa`) |

## 1. State

Screen flags (bytes). `Belep` clears all but `Mode_Main`. Floats are f32.

| ELF name | exe address | meaning |
|---|---|---|
| `Mode_Main` | 0xbbb3d1 | the 6-ring main menu |
| `Mode_SinglePlayerSelected` | 0xbbb3d2 | the 3-ring single player menu (campaign / scenario / back); together with `Mode_Fajvalasztas` the race pick and the difficulty rings |
| `Mode_MultiPlayerSelected` | 0xbbb3d3 | the 3-ring multiplayer menu |
| `Mode_Fajvalasztas` / `_Becsukas` | 0xbbb17f / 0xbbb228 | race choice open / closing |
| `Mode_Tutorial_Valasztas` | 0xbbb3f4 | the 6-ring tutorial menu |
| `Mode_Scenario_Valasztas` / `_Becsukas` | 0xbbb3f7 / 0xbbb229 | scenario / skirmish list open / closing |
| `Mode_Kilepes` | 0xbbb22a | exit animation running |
| `Main_NemKellZoom` | 0xbbb22b | skip the exit animation (debug Enter) |
| `Mode_Tutorial_Start` / `TutorialStartMelyik` | 0xbbb22c / 0xbbb230 | start tutorial n (1..5) when the screen is left |
| `Mode_Scenario_Vagy_Scirmish_Start` | 0xbbb22d | start the selected scenario / skirmish when left |
| `FajValasztasStartGame` | 0xbbb17c | start the campaign when left |
| `FajValasztasFajKivalasztva` / `...Becsukas` | 0xbbb163 / 0xbbb3f5 | race confirmed (difficulty rings) / closing |
| `FajValasztasSelectedRace` / `...NehezsegiSzint` | 0xbbb178 / 0xbbb180 | race 0..2 / difficulty index 0..2 |
| `LastKijelolt` / `VanFajVLight1` | 0xbbb3e0 / 0xbbb161 | race highlighted (-1 none) |
| `FajVSzetnyilas` (+`Cel`) | 0xbbb16c (+0xbbb170) | race-sphere spread 0..100 |
| `AktKijelolt` | 0xbbb3dc | ring under the pointer this frame (-1 none) |
| `ScenarioHighScoreMode` | 0xbbb22e | scenario panel shows the high-score table |
| `EnterIPAddressMode` / `EnterNameMode` / `EnterHighScoreNameMode` / `EnterIRCAddressMode` / `YouAreDisconnectedVolt` | 0xbbb365 / 0xbbb364 / 0xbbb366 / 0xbbb367 / 0xbbd144 | text-entry sub-states and the "disconnected" box |
| `MainKorokTavolsag` (+`Cel`) | 0xbbb050 (+0xbbb0bc) | ring distance factor, 1.0 = in place; `Cel` the target |
| `MaxMainKorok` | 0xbbb0c0 | rings shown (3 or 6; 10 slots) |
| `MainInactivityTimer` | 0xbbb0c4 | see 5.4 (never incremented) |
| `MainUrhajoTimer` / `MainAszteroidaTimer` | 0xbbb0c8 / 0xbbb0d0 | ticks since a ship / meteor respawn |
| `GlobalTransparency` | 0xbbb0cc | scene fade-in 0..1 (0 at entry) |
| `MainScrMozgato` | 0xbbabe0 | animation clock, +0.1 a tick |
| `MainElindult` | 0xbbac53 | set by the first `Update3D`; gates the tick |
| `MainScrKilepesValtozo` | 0xbbb360 | exit animation 100 -> 0, -0.5 a tick |
| `ScenarioKepNyitottsag` (+`Cel`) / `ScenarioValasztasNyitottsag` | 0xbbb048 (+0xbbb04c) / 0xbbb2ac | scenario picture fade 0..100 / list opening 0..100 |
| `SelectedScenario` / `SelectedScenarioOld` / `ScenarioStart` / `ScenarioListMut` | 0xbbb350 / 0xbbb354 / 0xbbb188 / 0xe40c44 (list ptr 0xe40c40, loaded 0xe40c48) | |
| `SelectedScirmishRace` 0xbbb358 (1..8), `...ComputerPlayers` 0xbbb35c (1..7), `...Difficulty` 0xbbb234 (1..4), `...NumberOfPlanets` 0xbbb238 (0..4), `...TechnologyLevel` 0xbbb23c (0..3), `...StartingMoney` 0xbbb240 (0..5), `...Population` 0xbbb244 (0..3) | | skirmish options |
| `AktKep` 0xbae0fc, `AktIkon` 0xbae23c, `AktIkon2` 0xbae240, `Belep` 0xbae24c, `Atlepes` 0xbae24d, `OldKep` 0xbae19c, `OptionsElottiKep` 0xbae2b4, `STARTSCREEN` 0x927a88 (= 2, the starmap), `EN` 0x912a3c, `ProgramMode` 0xbae544 (0 campaign, 1 internet, 2 LAN) | | |
| `Mouse_x/y` 0xdb4654 / 0xdb4658 (virtual 640x480), `Mouse_Lenyom` 0xdb465f (press edge), `Mouse_Bal` 0xdb4651 (left button), `Mouse_Megfogva` 0xdb466a, `Mouse_FelengedesMajdMegvar` 0xdb4669, `Mouse_Tipus` 0xdb9414 (1 hand, -2 hidden) | | |
| `SelfPlaying` 0xbae267 (command line `demomode`, stored at 0x4dfac7), `DEBUGLEHET` 0xbae659 (argument parse at 0x4dfb6d), `VoltGTLogo` 0xbbb22f, `MainSkipAllAnims` 0xbbb3f6, `IdoPauseMertAtlepesAlattVan` 0xbae265, `OptionsCsakLoadLehet` 0xbae24e | | |
| `SOUND` 0x919930; `METEOROK` 0x9147f9, `URHAJOK` 0x9147f8 (both 1); `METMAX` 0x9140e4 = -50000.0; `GlobalTransparencyNovelo` 0x913de4 = 0.003; `METKOZ` = -48000.0 (ELF 0x1184638; the exe's copy not found); `MINDENEGYSZERRE` (0, debug) | | |

`AktIkon` / `AktIkon2` (`IG2_gameUpdate`, ELF 0x587400 / exe 0x4e0440): `AktIkon = Mouse_Darabol()` = the hot-spot index, which for a
`Mouse_Szoveg_Set(id, ...)` box is `id + 0x1e`; `AktIkon2 = AktIkon - 0x1e` = the id under the pointer, every frame. A box whose 8th argument is 1
(`MouseSzoveg[...+0x74]`) fires on the release (`Mouse_FelengedesMajdMegvar`), so `MainScr_Main` sees `AktIkon` = `id + 0x1e` once. So its
`AktIkon` 0x1f..0x32 are box ids 1..0x14 and its `AktIkon2` 0x15..0x27 are box ids.

## 2. Enter and leave

### 2.1 `MainScr_Belep` (exe 0x4fe3c0)
1. If not `DEBUGLEHET` and not `VoltGTLogo` (once per run): `VoltGTLogo = 1; IdoPauseMertAtlepesAlattVan = 1;` unless `MainSkipAllAnims`:
   `OpenBink("title.bik", 1, 0, 0, 0)`; if it opened: `PlayBink(1, 0, 0)`, `CloseBink()`; then `IdoPauseMertAtlepesAlattVan = 0`.
   (Credits: `OpenBink("misc/credits.bik", 1, 0, 0, 1)`, 5th argument 1; meaning unknown.)
2. `InitFaderDesignStruktura(mainf = 0xbbcb70)`. `LoadScenarioList()`; the exe inlines `LoadScenarioKep`: list non-empty and `SelectedScenario < count`:
   `SetKepName(0x3e, entry+8)`, `UnLoadKep(0x3e)`, `LoadKepToMemory(0x3e)`. `LoadFajInfoText()`.
3. `SelfPlaying` -> demo (9.4), return. Else:
4. `Mode_Main = 1`; every other `Mode_*`, `starmaphovaleptovabb(2)` (0xf176e0/e4), `ScreensScrollHistoryMode` (0xe40c5e) = 0; three `xvbp_id = null`
   (`MainScrGfxCallbackHordozo1..3`); `MainInitSounds()`; `IrcMainAtlatszosaga = MainMainAtlatszosaga = 1.0`; `AktBrushok = MainScrBrushok`
   (0x913de8), count 11 (0xbaf32c); `OptionsCsakLoadLehet = 0; StarmapVoltInit (0xb99339) = 0`; the five text-entry flags = 0; `SetIdoSpeed(1, false)`;
   `autojoin_count_progress (0xbbb0d4) = 0`.
5. If `OldKep` is 0x10 or 0xe, or `OldKep == 0xd && OptionsElottiKep == 0xf`: keep the old 3D scene: `MainKorokTavolsag = 10.0; Cel = 1.0; MainInactivityTimer = 0`;
   and for `OldKep` 0x10/0xe: `Mode_Main = 0; Mode_MultiPlayerSelected = 1`. Else (fresh): `Main3DInit()`; `MainScrMozgato = GlobalTransparency = MainUrhajoTimer = MainAszteroidaTimer = 0`;
   `PlayMusicMenu(false, true)` (= `PlayMusicExistCheck(0xc, false, true, true)`, ELF 0x61f630); `MainElindult = 0`.
6. `MainScrInitNullBoxes()` (3.1); `ScenarioKepNyitottsag = 0; Cel = 100.0`.
7. If `scenario_nevbevitelkell` (0xbad10c; back from a finished scenario): `Mode_Main = 0; ScenarioGombraRanyomtak(scenario_registry_name 0xbad0e8); EnterHighScoreNameMode = 1;`
   "Player" copied into `HighScoreNev` (0x914790); `IOMode = 1; IOAzonosito = 0x99; IOString = HighScoreNev; IOMaxLen = 0x14; IOX = 0x91; IOY = 0xdf; IOCharSize = 3; IOColor = 2;
   IOKurzor = strlen; IOGombnyomasraTorles = 0`. (The exe copies the literal "Player" at 0x914800; the ELF copies `MultiplayerNev`.)
8. `Mouse_Lenyom = 0`.

### 2.2 `MainScr_Kilep` (exe 0x4fecc0; `AktKep` is already the target)
`SelfPlaying` -> return.
1. If `Main_NemKellZoom` or the target is 0xe / 0x10 / 0xd (Start, IRC, Options): no exit animation, the scene stays. Otherwise:
   - picture `mcd` 0..2 = index into `LoadingTexturak[3]` (`load1..3.bmp`, exe 0xbbac54): race start -> `FajValasztasSelectedRace`; scenario / skirmish start -> entry race field
     (exe +0x38, ELF +0x6c) 3 -> 1, 5 -> 2, else 0 (skirmish entries, kind > 0: `SelectedScirmishRace` the same way); tutorial -> 0.
     `GetSubMeshid(LoadingMesh1 0xd84368, sub 0xd84498, 0)`; `ChangeTexture(sub, LoadingTexturak[mcd])`.
   - `Mode_Kilepes = 1; MainScrKilepesValtozo = 100.0; AktKepBackup = AktKep; AktKep = 0xf`; loop while `MainScrKilepesValtozo > 0`: `Mouse_Tipus = -2; ForceUpdateFrame (0xbae2ac) = 1;
     Cel = 6.0; UpdateFrame(); FRISSITES(); ForceUpdateFrame = 0`. Then restore `AktKep`, `Mode_Kilepes = 0`. (200 ticks = 2 s of what 4 and 5 describe.)
2. `Main_NemKellZoom = 0`. If the target is not 0xe / 0x10 / 0xd: `mainOldalsoFenyekKi()`, `MainDeInitSounds()`, `RemoveFajVLight()`. `MainScrRemoveNullBoxes()`.
3. What is started:
   - `FajValasztasStartGame`: `CampaignGameStart()` (9.1).
   - `Mode_Tutorial_Start`: `Mode_Tutorial_Start = 0; Cel = 6.0; TutorialSetCampaignGameOptions(TutorialStartMelyik); TutorialStart("main", n)`. `TutorialSetCampaignGameOptions` (exe 0x504250): `ProgramMode = 0`,
     `SingleStartGame()`, `SetIdoSpeed(1, 0)`, `ProgramMachines = 1`, `ProgramPlayers = 8`, `ProgramPlayers2 = 0x12`, players 1..18 as in 9.1, `EN = 1`, `PlayerKi[1] = 1`, `ReadDebugData()`,
     `InitNewGame(SDL_GetTicks(), 1, 0, 0)` [mode 1 from the ELF; the exe decompile shows the seed only], `TutorialLoadScenario(n)`, `StartStory(exe 0xb77260)` if set,
     `OldIdo = OldIdo2 = (ticks / 0x32) * 0x32`, `ProgramNowStarting = 1`.
   - `Mode_Scenario_Vagy_Scirmish_Start` and a non-empty list: entry kind (exe +0x90, ELF +0xf0) == 0 -> `Betolt_Scenario(file)`, `StoreScenarioDescription(SelectedScenario)` (exe 0x504120),
     `Info_AddStarLog(MainScenarioDescriptor, 2, "", "")` (exe 0x4e7970), `scenario_registry_name = entry registry name`, `scenario_kezdesi_ido (0xbad108) = Ido (0xbae04c)`.
     Else a skirmish: `SetScirmishGameOptions(SelectedScirmishRace, ...ComputerPlayers, kind, ...TechnologyLevel, ScenarioKezdoBolygoSzamok[...NumberOfPlanets], ScenarioKezdoPenzek[...StartingMoney],
     ...Difficulty, ...Population)` (exe 0x503e80). Tables: exe 0x9140d0 = {20, 40, 60, 80, 99}; exe 0x9140b8 = {10000, 25000, 50000, 100000, 200000, 500000}.
   - then `UnLoadScenarioList()` (exe 0x553c20) in every case.

## 3. The 3D scene (`Main3DInit`, exe 0x4f96a0; ELF 0x58d7f0)
Fresh entry only. `LoadBox(name, lit)`, `AddToScene(xvb, scene, scale, mesh)`; directory changes noted.
1. Camera pre-set (the formulas of 4.1, with y amplitude 200 and z = sin(...)*1000 + 1500 here; `Update3D` overwrites them). `FreeMainXVB(); CreateMainXVB()` (scene `mainxvb`, exe 0xde003c).
2. `textures/design/screen/`: `fadertexture = loadtexture("dt.bmp", 0)` (0xde02a4). Then `textures/main/`, `vector/main/`.
3. `loadi1.xvb` / `loadi2.xvb`: scale 0.21 (0x3e570a3d), `MoveObj(0, 0, -900)`, `SetRelative(false)`, properties off 2, 4, 8, on 0x200, `SetObjShade(false)`, `RotateObj(0,0,0)`, hidden
   (`LoadingMesh1` 0xd84368, `LoadingMesh2` 0xd84278). Textures `load1.bmp..load3.bmp` (`loadtexture(name, 1)`) -> `LoadingTexturak`. `MainMouseOld = Mouse_x/y` (0xbbb3d4/d8).
4. `galaxy.xvb`: scale 0.6, `SetObjTransparency(0.5)`, properties off 8 and 4, on 0x20, `SetObjShade(false)` -> `Maingombmesh` (0xbbd584).
5. Four galaxy clouds (null box + `SetParticle`, record field 100.0 at +0x20; `GenerateGalaxy(particles, n, 100, r, g, b, alpha, ox, oy, oz, scale)`, ELF source in `_local/decomp/starmap/GenerateGalaxy.c`, exe 0x4f7e60):
   `n2.bmp` n 3500 (0xdac) rgb (0.1, 0.1, 0.1) alpha 200 scale 1.4; `felho2.bmp` 3500 (1, 1, 1) 100 scale 2.0; `n2.bmp` 3500 (0.5, 0.7, 1.0) 200 scale 3.0; `n2.bmp` 3300 (0xce4) (0.5, 0.7, 1.0) 200 scale 0.8.
   The particles are static random points (the colour word is built with a factor 0.0, so black until recoloured); `ChangeGalaxyColors` recolours them every frame (4.5). Layout, draw order and distribution: 3.2.
6. Three background billboards (one particle each): `texcsor.bmp` at (-400, -400, -100) size 1350 colour 0x64ffffff; `galaxy_1.bmp` at (-500, 1500, -3000) size 300 colour 0x8cffffff; `galaxy_1.bmp` at (-900, 1900, -3000) size 200 colour 0x8cffffff.
   `hanyhattergalaxis` (0xbbd13c) = **2** (the index of the last; `Update3D` moves entries 0 and 1 only, so the third stays where it was made).
7. Three dust systems ("kod"; exe 0xcc1470 / 0xd023c0 / 0xd43310): `GenerateKod` n 3500, no texture, (0.5, 0.5, 0.5); `GenerateKod2` n 1750 `torpedo.bmp` (0, 0, 0); `GenerateKod2` n 1750 `n2.bmp` (0.5, 0.5, 0.5);
   all with offset (-1000, -800, -1800) and scale 30.0 (3.2; their `srand(400)` fixes the RNG state for step 8).
8. `meteor.xvb` x 3 (`Meteorok` 0xd84260): scale `((3/(i + 0.5))/3)*1.5` = 3.0, 1.0, 0.6 (0.1 if <= 0), times `(rand%3)/10 + 0.5`; `SetRelative(false)`; position x = `rand%5000 - 5000`, y the same, z = `-(rand % (int)(-METMAX))`
   (**the exe uses the 15-bit CRT `rand()`, so z is in -32767..0**); spin per axis `(rand%10)/55/scale`; `MeteorSebessegek[i] = 25.0`; `MeteorVoltHangjamar = 0`.
   Arrays (exe): position 0xbbabe4 (stride 0x10), spin 0xbbac14 (0x10), speed 0xbbac44 (4), sound-played 0xbbac50.
9. `textures/spacewar/`, `vector/spacewar/`: eight ships `flag{n}0.xvb` (`"flag"` + digit + `"0.xvb"`), n = 1..8, `LoadBox(name, true)`, scale 0.5, `SetRelative(false)` -> `Urhajok` 0xd84320 (8 bytes each);
   `UrhajoElforgatasok[i] = 0` (0xbbae90); `GenerateUrhajoPosok(i)` (5.5); `UrhajoForoghate` (0xbbae60) = 1 only for n = 3, `UrhajoForoghate2` (0xbbae68) = 1 only for n = 2; a parallel light per ship:
   `CreateLightParalell(colour 0x323238, dir (-1000, 0, 2000), ship mesh, UrhajoFenyek[i])` (0xd84290, 0xc each).
10. `textures/main/`, `vector/main/`: `MainKorokTavolsag = 10.0; Cel = 1.0; LoadFajValasztasVektor();` ELF `InitFajvalasztas()` / exe `VisszaAllitFajvalasztasKezdoPoziciok()` only; `HozzaadKorok()` (creates the 10 rings, 3.1).
11. Button textures (`loadtexture(name, 1)`): `textures/main/eng/unselected/` -> `SzovegTexturak*`, `textures/main/eng/selected/` -> `KSzovegTexturak*` (`eng` is a literal in both binaries). Name tables (exe pointer tables):
    set 1 (0x913d90, 6): `0_credits.bmp`, `0_exitus!.bmp`, `0_loadgam.bmp`, `0_multipl.bmp`, `0_singlep.bmp`, `0_tutoria.bmp`;
    set 2 (0x913da8, 3): `0__camp.bmp`, `0__scenar.bmp`, `0__back.bmp`; set 3 (0x913db4, 3): `0___local.bmp`, `0___inet.bmp`, `0__back.bmp`;
    set 4 (0x913dc0, 3): `fokozatok/easy.bmp`, `fokozatok/hard.bmp`, `fokozatok/normal.bmp`; set 5 (0x913dcc, 6): `tutorial/part1.bmp`..`part5.bmp`, `0__back.bmp`.
    Handles: unselected 0xbbaeb8 / 0xbbaee0 / 0xbbaf08 / 0xbbaf30 / 0xbbaf58; selected 0xbbaf80 / 0xbbafa8 / 0xbbafd0 / 0xbbaff8 / 0xbbb020.
12. Eight heads `korke1.xvb`, scale 0.027 (0x3cdd2f1b), `SetRelative(false)`, `MoveObj`: heads 0..3 at (-220, i*94 - 150, -500), heads 4..7 at (225, (i-4)*94 - 150, -500), properties on 8, hidden; head 0 gets
    `SetRenderOther(mesh, StartKirakJatekosFejek, 0)` (`mFejek` exe 0xcc12e4, stride 0x2c). They belong to the Start screen; hidden here.
13. Projection: `fxn = 30.0, fyn = 25.0, fzn = -210.0, zbm = 0.05, valtashatar = 3600` (exe 0x919658, 0xde01cc, 0xde01d4, 0x91965c, 0x919660), `xcenter = 320.0, ycenter = 240.0` (0x919650/54);
    `d = ((sdl_width*480)/sdl_height - 640)/2` (int); `HatarX1 = -d, HatarX2 = d + 639, HatarY1 = 0, HatarY2 = 479, HatarXm = HatarX2 - HatarX1 + 1, HatarYm = 480` (exe 0xb9a980..); `elforgatas = 0`, pitch, yaw = 0;
    `ModifyViewPort()`; `mainOldalsoFenyekBe()` (side lights on); `forgadd (0xbbb3cc) = 0`; `MaxMainKorok = 6`; exe `WaitForDrawing()`; directories back to `textures/`, `vector/`. The ELF also calls `reloadtextures()`.

### 3.1 The rings (`HozzaadKorok`, exe 0x4f8780; ELF 0x58d2d0; both agree)
`MainKorok[i]` (exe 0xd84370 + 0x1c*i, i = 0..9, loop end 0xd84488): mesh A (+0), mesh B (+8), position x, y, z (+0x10, +0x14, +0x18). Called from `Main3DInit` with the directories `textures/main`, `vector/main`. Per ring, in this order:

| mesh | model (`LoadBox(name, lit)`) | `AddToScene` scale | the rest (identical for A and B) |
|---|---|---|---|
| A (+0), the ring | `korke1.xvb` (the same file for all 10), lit = false | 0.7 (exe 0x3f333333) | `MoveObj(m, rec.x, rec.y, rec.z)`; `SetRelative(false)`; `SetObjTransparency(0.3)` (0x3e99999a); `PropertiesOff(8)`, `Off(4)`, `Off(2)`, `On(0x20)`; `SetObjShade(false)` |
| B (+8), the button face | `szrke{i}.xvb` = `szrke0.xvb` .. `szrke9.xvb` (the digit is the ring index; the exe builds `"szrke"` + `'0'+i` + `".xvb"`), lit = false | 0.3 (0x3e99999a) | the same calls with B's own `MoveObj` (same record position) |

- The label goes on **sub-mesh 0 of B** (`Update3D` step 10: `GetSubMeshid(B_i, sub, 0)`; `ChangeTexture(sub, tex)`); the ring A has no label. Whether the ten `szrke{i}` files differ in geometry: not checked (asset-finder: `vector/main/szrke0..9.xvb`, `korke1.xvb`).
- Transparency 0.3 on both; `Update3D` never touches the rings' transparency again (only meteors, ships and the loading plates get `SetObjTransparency` there). The engine's transparency is "0 = opaque" [evidence: meteors use `1 - GlobalTransparency` and fade IN as `GT` goes 0 -> 1].
- Property bits 8, 4, 2 (off) and 0x20 (on): no spec documents them (`re/screens/render.md` only has the inherit rule); `galaxy.xvb` uses the same pattern (off 8, 4; on 0x20). Meaning [unknown]; copy the pattern, don't interpret it.
- The rings are **unlit** (`LoadBox(.., false)` + `SetObjShade(false)`): the scene lights don't change them. Scene lights, created once (guard byte ELF 0x1618b51) by `mainOldalsoFenyekBe` (ELF 0x58c5b0; exe address not read; `mainOldalsoFenyekKi` = ELF 0x58c520): two `CreateLightParalell` on the global mesh at ELF 0x11abab4 [probably `mainmeshid`, unverified], handles ELF 0x1618030 / 0x161803c: L1 colour bytes (0x64, 0x64, 0xc8) direction (200, -1000, 500); L2 colour (0xff, 0x9c, 0x82) direction (0, 1400, 500) (constants read from the ELF data; colour byte order as stored in the struct). `mainMozgatMainFenyek` (ELF 0x58c690) is an **empty function** (`ret`): the side lights never move. They light only lit models (ships, race scene, probably the loading plates [their lit flag not checked]).
- `MainScrInitNullBoxes` (exe 0x4fe2c0; ELF 0x593f00): `GFX_AllandoInit1(); InitFader1(); AddNullBox(mainxvb, cb1 = exe 0xd84360); SetRenderOther(cb1, MainScrGfx2D_1 = exe 0x4fab30, 0); InitFader2(); AddNullBox(mainxvb, cb2 = exe 0xcc145c); SetRenderOther(cb2, MainScrGfx2D_2 = exe 0x4fc8d0, 0); AddNullBox(mainxvb, cb3 = exe 0xcc1438); SetRenderOther(cb3, MainScrGfx2D_3 = exe 0x4fe190, 0); GFX_AllandoInit2(); InitFader3()`. The three 2D passes are draw callbacks of null boxes added after the 3D objects; exe `_1` / `_3` = 0x4fab30 / 0x4fe190 (not decompiled).

### 3.2 Particle generators (clouds and dust of `Main3DInit` steps 4, 5, 7; recoloured in `Update3D` steps 2, 5)
Particle record (`PARTICLETYPE`), stride 0x4c: +0 x, +4 y, +8 z (f32, object-local), +0xc colour word (u32), +0x14 size (f32); the other fields are not written by these functions. The system header precedes the first record by 0x48 bytes (`count` at +0x1c, a field = 100.0 at +0x20).
`random(n)` (ELF 0x4f0200) = `n > 0 ? rand() % n : 0` (**no rand call when n <= 0**); the exe uses the CRT `rand() % n`.

**`GenerateGalaxy(p, n, _, r, g, b, alpha, ox, oy, oz, S)`** (ELF 0x58cde0; exe 0x4f7e60 not decompiled, its constants (180000.0 f64, 25.0, 100.0 f32, +0.3 f64, /2.5 f64) are consistent; r, g, b only feed a 0.0 factor). Per particle, draws in this order:
1. `a = random(2)`: (kx, ky) = (1, 0) if a == 0 else (0, 1) (a bar along x or along y, 50% each).
2. `h = random(30)` (thickness); `u1 = random(600)`, `v1 = random(50)`: `fx = kx*(u1 - 300) + v1 - 25`; `u2 = random(600)`, `v2 = random(50)`: `fy = ky*(u2 - 300) + v2 - 25` (the narrow axis is uniform +-25).
3. `d = sqrt(fx^2 + fy^2)`; `t = d/100`; `q = random((int)(t*100))` (skipped when the argument is 0); `ang = t + (q/100 - t/2)/2.5` (a twist of about d/100 radian, noise +-t/5).
4. `x = S*((h - 15)*((1 - d/sqrt(180000)) + 0.3)) + ox` (thickness, thinner at the rim: factor 1.3 at the centre, 0.3 at d = 424); `y = S*(cos(ang)*fx - sin(ang)*fy) + oy`; `z = S*(cos(ang)*fy + sin(ang)*fx) + oz`.
5. `size = random(8) + 12`; colour = `alpha << 24` | three `random(20)` draws each `(x + 235)*0.0` = 0 (so **black with alpha only** until `ChangeGalaxyColors` recolours). 11 draws per particle (10 when `(int)(t*100) == 0`).
Shape: a bar up to 650 long and 50 wide per cloud, twisted by up to ~3 rad at the tips, thickness along **x** (+-15*1.3*S), the disc in the y-z plane. All four calls use (ox, oy, oz) = (0, 0, 0) (ELF `Main3DInit` lines 92-129), S = 1.4, 2.0, 3.0, 0.8; counts 3500, 3500, 3500, 3300; alpha 200, 100, 200, 200. No `srand` here (stream = whatever the process had; statistically equal is enough).

**`ChangeGalaxyColors(p, n, _, pr, pg, pb, pa)`** (every frame; ELF 0x58c700; exe 0x4f6040, same maths, **all f32 in the exe**: `mulss` by 255.0, 100000.0 and 0.2 as f32 constants): `A = (int)(GT*pa)`; per particle `f = min(1.0, (x*x + y*y + z*z)/100000 + 0.2)`; colour = `A << 24 | (int)(GT*pb*255*f) << 16 | (int)(GT*pg*255*f) << 8 | (int)(GT*pr*255*f)`, `GT` = `GlobalTransparency` (exe 0xbbb0cc; ELF 0x1618888). Parameter order = (low byte, middle byte, high byte): the call `(0.6, 0.6, 0.9, 130)` puts 0.9 in bits 16..23. No clamp to 255 (all factors <= 1). Points with radius^2 >= 80000 (|p| >= 283) get the full tint, the centre is dimmer (floor 0.2). **Which byte is red depends on the vertex colour format** (`0xAARRGGBB` would make the "blue" calls reddish): [unknown, check against a screendiff capture].

**`GenerateKod(p, n, _, r, g, b, a, ox, oy, oz, S)`** (ELF 0x58c960; exe 0x4f8240 not decompiled; the exe has `push 0x190; call` at 0x4f8254 [byte scan], matching the ELF): `srand(SDL_GetTicks()); srand(400)` (the third argument is overwritten with 400, so **fully deterministic**); per particle: `x = ox + S*rand(100)`, `y = oy + S*rand(100)`, `z = oz + S*rand(100)` (a 3000-unit cube from (-1000, -800, -1800) with S = 30), size (+0x14) = 1.0 (ELF constant 0xd3f088), colour = `a << 24 | (rand(127) + 128)*b << 16 | (rand(127) + 128)*g << 8 | (rand(127) + 128)*r` (3 draws, order b, g, r; a = 0 in the call).
**`GenerateKod2`** (ELF 0x58cc60; the two 1750-point systems): `srand(400)`; `x`, `y`, `z` = `o + S*rand(150)` each (a 4500-unit cube from the offset), size = `rand(30) + 30`, colour = constant `0x32787878`.
**`DeGenerateKod(p, n, _, r, g, b, a)`** (every drawn frame; ELF 0x58cb50; the exe has `push 0x190; call` at 0x4f8185): `srand(400)`; per particle colour = `(int)(GT*a) << 24 | (int)((rand(127) + 128)*b*GT) << 16 | (int)((rand(127) + 128)*g*GT) << 8 | (int)((rand(127) + 128)*r*GT)` (draws in order b, g, r; f32, left to right). The calls pass a = 0, so the alpha byte is **0** and the dust fades through its channels only (blend mode presumably additive, not checked). System 1: (0.5, 0.5, 0.5), 3500 points; system 2 (`torpedo.bmp`): (0, 0, 0) = black; system 3 (`n2.bmp`): (0.5, 0.5, 0.5). Per-particle brightness is a fixed pseudo-random 128..254 (same every frame).

**RNG trap**: every drawn frame ends with `srand(400)` + 5250 draws (the last `DeGenerateKod` call, 1750 x 3), so the stream sits at draw #5250 whenever a 10 ms tick runs. The ticks' `rand()` users (meteor respawn, `GenerateUrhajoPosok`) then read draws #5251.. unless another consumer in the same inter-frame window moved it. With the MSVC `rand` (`s = s*214013 + 2531011` mod 2^32, result `(s >> 16) & 0x7fff`, `srand(400)` sets s = 400) draws #5251..#5262 are 18972, 24342, 5586, 2451, 15269, 15840, 27791, 1105, 17870, 19072, 30363, 17057 [computed, not observed]. If a respawn is the first consumer after a frame: meteor z = -50972, x = 842, y = -1414; ship: unhidden (#5251 % 6 == 0), start (-658, 1086, -8549), speed factor 3.9. So the original's respawns are largely deterministic and repeat the same positions [hypothesis; verify with a Frida trace of the meteor array before copying]. In `Main3DInit` the meteors / ships are drawn after the last `GenerateKod2` (`srand(400)` + 7000 draws): draws from #7001: 2724, 30661, 9044, 10050, 10580, 21632, 28165, 711, 18746, 11688, 18238, 6992 [computed].

## 4. Per-frame 3D (`Mainscreen_Update3D`, exe 0x500270; ELF 0x58fc80; per drawn frame)
Called from `GFX_MainScr` unless `SelfPlaying`. Order [exe; ELF agrees unless noted]:
1. `MainElindult = 1`. **Camera** (`kamerapont` exe 0xde0148, `elforgatas` 0xde0158; double math, stored f32; M = `MainScrMozgato`):
   x = cos(M/50)*100 + 100; y = sin(M/82)*100 - 100; z = sin(M/165 + pi/2)*100 + 2000; `elforgatas` = cos(M/150 + pi)/3 + 0.2; `forgaspont` (0xde0130, 3 floats) = 0.
   While `Mode_Kilepes`, with q = 100 - `MainScrKilepesValtozo`: z -= q*30; `forgaspont.x` = q*5.
2. `DeGenerateKod` for the three dust systems (args (0.5, 0.5, 0.5), (0, 0, 0), (0.5, 0.5, 0.5), alpha arg 0): each call re-seeds `srand(400)` and recolours; see 3.2 (RNG trap).
3. `AktKep == 0xf`: `Mouse_Szoveg_Off_Range(0x11, 0x14)`.
4. Background billboards i < `hanyhattergalaxis` (= 2): `MoveObj(mesh, p.x, p.y, camz/1.2 - 3000 + i/10)` (integer i/10 = 0); colour word `A<<24 | c<<16 | c<<8 | c`, `A = (int)(GT*140*GT)`, `c = (uint)(GT*255)`, GT = `GlobalTransparency`
   (ELF: `ChangeHatterGalaxyColors(p, GT*140.0)`).
5. Rotations (`RotateObj(mesh, 0, yaw, roll)`, then a second rotation about the axis (1, 0, 0), `RotateObj2` in the exe):
   `Maingombmesh`: roll (-M/25)/2, then axis angle 1.0471976 (pi/3);
   cloud 0: yaw 1.5707964, roll (-M/20)/4, axis angle 1.0; `ChangeGalaxyColors(cloud, 3500, 100, 0.6, 0.6, 0.9, 130.0)`;
   cloud 1: roll (-M/30)/4; colours (0.2, 0.4, 0.6, 170.0); cloud 2: (-M/40)/4; (0.2, 0.3, 0.5, 150.0); cloud 3 (3300 points): (-M/10)/4; (0.7, 0.7, 0.7, 100.0). The exe values match.
6. Meteors i = 0..2 (`METEOROK`; hidden if off): `RotateObjr(mesh, spin)`; `MoveObj(mesh, pos)`; if z > `METMAX` (-50000): `UnhideObj`, transparency
   T = (z >= METKOZ) ? 1 - GT : clamp((z - METKOZ)/(METMAX - METKOZ), 0, 1)*GT + (1 - GT) [ELF; the exe decompile dropped the argument; METKOZ = -48000]; else `HideObj`.
   Sound: first time z > -9000: `MainPlaySound(meteor, i, v)`, v = 0 for speed > 0.6, 1 for (0.3, 0.6], 2 for <= 0.3 (speed is 25.0, so v = 0); `MeteorVoltHangjamar = 1`.
   `MainSetSoundPan(meteor, i, x, y, z)` every frame (exe passes x only: `FUN_00500040(1, i, x)`); it pans through a no-op (section 8).
7. Ships i = 0..7 (`URHAJOK`; hidden if off): `MoveObj(mesh, UrhajoPosok[i])`; `SetObjTransparency(UrhajoAtlatszosagok[i]*GT + (1 - GT)*1.0)` [ELF]; d = normalize(`UrhajoCelPontok` - `UrhajoKezdoPosok`);
   matrix A = `CreateMatrix2(d, ang)`, ang = sin(`UrhajoElforgatasok[i]`) + pi/2 if `UrhajoForoghate[i] == 0`, else `Foroghate*M/20 + 1.5707964`; matrix B = `CreateMatrix(0, 0, UrhajoForoghate2[i]*M/20 + 1.5707964)`;
   `SetObjMatrix(mesh, B*A)`. Sound once while -3000 < z < 3000 (variant by `UrhajoSebessegek.z`: > 6 -> 0, (1, 6] -> 1, <= 1 -> 2; `UrhajoVoltHangjamar`); pan every frame.
8. Rings i = 0..9: if i < `MaxMainKorok`: unhide A and B; theta = M/50 + (2*pi/`MaxMainKorok`)*i (double); x = sin(theta)*800*Tav, y = cos(theta)*800*Tav, z = -3000 - (Tav - 1)*400 (Tav = `MainKorokTavolsag`);
   `MoveObj(A, x, y, z)`; `MoveObj(B, x, y, z)`; `RotateObj(A, ((Tav - 1)/5)*3.1415927, 0, (6.2831855/MaxMainKorok)*i + M/(i*2 + 18))` [ELF; exe lost args 2-3]. Else hide both.
9. `AktKijelolt = -1`. If `AktKep == 0xf` and none of `EnterIPAddressMode`, `EnterNameMode`, `YouAreDisconnectedVolt`, `EnterIRCAddressMode` is set: `Mouse_Szoveg_Off_Range(1, 0xd)`, then the ring boxes and hit-tests of 6.1.
10. Label textures, when `Cel != 5.0`: for each ring i < `MaxMainKorok`: `GetSubMeshid(B_i, sub, 0)`; `ChangeTexture(sub, selected[i])` if `AktKijelolt == i` (ELF: and `ControlConfig[0]` (touch mode) clear), else `unselected[i]`.
    Set: `Mode_Main` -> set 1; `Mode_SinglePlayerSelected` and neither race confirmed nor closing -> set 2; single with race confirmed or closing -> set 4; `Mode_MultiPlayerSelected` -> set 3; `Mode_Tutorial_Valasztas` -> set 5;
    `Mode_Main` loops 6 rings, the others `MaxMainKorok`.
11. `mainMozgatMainFenyek()` [ELF], `ForgatFajvalasztasDolgai()`, `HandleLoadingMesh()` if `AktKep == 0xf`; `AktKep == 2` is treated as 0xf around `Update3D(null, null, 0)` (this scene draws while the starmap is being entered);
    `KiirFajInfoSzoveg()` (exe 0x4f9040); exe: `if FajValasztasStartGame && FajVSzetnyilas == 100.0 && !Mode_Kilepes { AktKep = STARTSCREEN; Atlepes = 1 }` (ELF: `CheckCampaignGameStart()`).
`HandleLoadingMesh` (exe 0x4f8620; ELF 0x58d670), only while `Mode_Kilepes && K < 50.0` (the last 100 ticks of the exit animation, K = `MainScrKilepesValtozo`): `T = K/50` (exe: **f64** division `divsd 50.0` at 0x4f8654, stored f32; ELF f32). Both plates (`LoadingMesh1` 0xd84368, `LoadingMesh2` 0xd84278): `UnhideObj`, `SetObjTransparency(T)` (T falls 1 -> 0 = fades in). Plate 1: `MoveObj(0, -(T*100), -1400)`, `RotateObj(0, T*(pi/2), 0)`; plate 2: `MoveObj(0, +T*100, -1400)`, `RotateObj(0, -T*(pi/2), 0)` (exe: `(float)((double)T*1.5707963267948966 + 0.0)`, `mulsd` 0x4f86ed; ELF f32 1.5707964). So the two plates slide together from y = -+100 to 0 and swing from +-90 degrees yaw to 0 at z = -1400 while the rings are flying away (`Cel = 6.0`); their textures are `LoadingTexturak[mcd]` (2.2).

## 5. Timers (`FRISSIT_MainScr2`, **exe 0x4f65f0 decompiled; these are the exe values**; ELF 0x59f360 agrees except where section 10 says so)
Runs in the 10 ms UI loop of `Fake_FRISSITES` (exe 0x4de720: `while (10 < now - OldIdo2) { OldIdo2 += 10; ... }`, 100 Hz), after `TutorialFrissit`.
Flags (exe): `AktKep` 0xbae0fc; `MainElindult` 0xbbac53; `IrcElindult` 0xbb6f29; `OptionsCsakLoadLehet` 0xbae24e; `Mode_Kilepes` 0xbbb22a; `MainScrKilepesValtozo` (K) 0xbbb360; `Atlepes` 0xbae24d, `Belep` 0xbae24c; `MINDENEGYSZERRE` 0xbbb3d0; `Mode_Main` 0xbbb3d1, `Mode_SinglePlayerSelected` 0xbbb3d2, `Mode_MultiPlayerSelected` 0xbbb3d3; `Mode_Scenario_Valasztas` 0xbbb3f7, `_Becsukas` 0xbbb229; `Mode_Fajvalasztas` 0xbbb17f, `_Becsukas` 0xbbb228; `MaxMainKorok` 0xbbb0c0; `Tav` (`MainKorokTavolsag`) 0xbbb050, `Cel` 0xbbb0bc; `GlobalTransparency` (GT) 0xbbb0cc, step `GlobalTransparencyNovelo` = f32 at 0x913de4 = 0.003; `MainScrMozgato` (M) 0xbbabe0; `MainUrhajoTimer` 0xbbb0c8; `MainAszteroidaTimer` 0xbbb0d0; `MainInactivityTimer` 0xbbb0c4; mouse x / y / pressed 0xdb4654 / 0xdb4658 / 0xdb465f.
**Gate** (the body 5.1 runs when): `AktKep == 0xe`, or `0x10 && IrcElindult`, or `0xf && MainElindult`, or `0xd && OptionsCsakLoadLehet`. **5.2 runs on every tick with `AktKep == 0xf`** (after the body when the gate is open, directly when it is closed); other `AktKep` values do nothing else.
Precision key (from the instruction operands, byte-scanned): **f64** = `cvtss2sd`, then `addsd|subsd|mulsd|divsd` with an f64 constant, `cvtsd2ss` store back (the decompile hides these casts!); **f32** = `addss|subss|mulss|divss`, `comiss` with f32 constants.

### 5.1 Per tick, inside the gate (exe order)
1. `FaderDesignFrissito(0xbbcb70)` (exe 0x4d2c20).
2. Scenario list (`Mode_Scenario_Valasztas`, value `ScenarioValasztasNyitottsag` 0xbbb2ac): not closing: `+= 1.0` (f32) while `< 100.0`, clamp to 100.0; closing (`_Becsukas`): while `> 0`: `-= 1.0`, at `<= 0`: 0, `_Becsukas = 0`, `Mode_SinglePlayerSelected = 1`, `Mode_Scenario_Valasztas = 0`, `Cel = 1.0`.
3. Race choice (`Mode_Fajvalasztas`): `MozgatKorYadderCel` (0xbbb3e4) = 0, or -50.0 (f32) when `LastKijelolt != -1` (`FrissitFajInfoSzoveg()` = exe 0x4f7c60 runs then) and the race is not confirmed; `MozgatKorYadder` (0xbbb3e8) moves toward it by **0.3 in f64** (`movsd 0.3` at 0x4f6776), clamped to it. If `FajVKINYITVA` (0xbbb162): `MainKorMozgatoValtozo` (0xbbb174) `+= 0.001` (**f64**, 0x4f67d5); `FajVSzetnyilas` (0xbbb16c) toward `Cel` (0xbbb170): going down `-= 0.6` (**f64**, 0x4f67f9); on reaching it with `Mode_Fajvalasztas_Becsukas`: `Mode_Fajvalasztas = 0; _Becsukas = 0; FUN_00504380(); MaxMainKorok = 3; Cel = 1.0` (the exe calls 0x504380 here, not read; the ELF calls `VisszaAllitFajvalasztasKezdoPoziciok`); going up `+= 1.0` (f32) to `Cel`: `FajValasztasFajKivalsztvaBecsukas` (0xbbb3f5) -> 0 with `MaxMainKorok = 3`; if not `StartGame` (0xbbb17c): `FajValasztasSelectedRace` (0xbbb178) = -1. `FajVFrame` (0xbbb168) `+= 0.3` (**f64**, 0x4f684d) while `< 90.0` (f32 compare); at `>= 90`: 90.0, `FajVKINYITVA = 1; LastKijelolt = 0; InitKiirInfoSzoveg()` (0x4f8ea0)`; InfoBelerakSzoveg(0)` (0x4f8910).
4. Screen 0x10 / 0xe fades: `0xbbb3ec -= 0.005` (**f64**, 0x4f6926) while `> 0` and `AktKep == 0x10`, clamp 0; `0xbb6f2c += 0.02` (**f64**, 0x4f695e) to 1.0 if `AktKep == 0x10` and byte 0x913990 != 0; `0xbbb3f0 -= 0.005` (f64) while `> 0` and `AktKep == 0xe` and byte 0xf21485 != 0; `0xf21480 += 0.02` (f64) to 1.0 if `AktKep == 0xe` and byte 0xf21484 != 0. (Names: `IrcMainAtlatszosaga` / `IrcKilepesValtozo` / `MainMainAtlatszosaga` / `MainKilepesValtozo`; the exe has the flag conditions.)
5. `MainUrhajoTimer += 1; MainAszteroidaTimer += 1` (ints); `M = (float)((double)M + 0.1)` (**f64** `addsd 0.1` at 0x4f6a3e, f32 store at 0x4f6a4c). Then the inlined `GetKameraPos` (matrix from `elforgatas` 0xde0158.., inverted; the result is unused in this function).
6. If `(!Atlepes && !Belep) || Mode_Kilepes`: meteors, then ships (5.3). Each loop reads its timer once before the loop (the snapshot).
7. If `AktKep == 0xf && !Mode_Fajvalasztas`: mouse equal to the last tick and not pressed -> if `MainInactivityTimer > 3000` (signed int; never incremented, 5.4) and `Cel != 6.0`: `Cel = 4.0`; otherwise (moved or pressed): `MainInactivityTimer = 0` and if `Cel < 5.0`: `Cel = 1.0`. Then the mouse position is saved (every `AktKep`).
8. `Tav` toward `Cel` unless `Mode_Fajvalasztas_Becsukas`: down `Tav = (float)((double)Tav - 0.05)`, up `+ 0.05` (**f64**, `movsd 0.05` at 0x4f70f1), clamped to `Cel` when it reaches or passes it. When `Tav` reaches `Cel == 5.0` going up: `MaxMainKorok = 3`, or 6 if neither `Mode_SinglePlayerSelected` nor `Mode_MultiPlayerSelected`; `Cel = 1.0`. So `Tav == 1.0` exactly at rest.
### 5.2 Per tick with `AktKep == 0xf`
- Not leaving: `GT += Novelo` (f32 0.003) while `< 1.0`, clamp 1.0 (about 333 ticks = 3.3 s). Leaving (`Mode_Kilepes`): `K -= 0.5` (**f64**, `subsd 0.5` at 0x4f71a5) while `> 0`, clamp 0; if `K < 25.0` (f32 compare): `GT = (float)((double)K / 25.0)` (**f64** division, 0x4f71df).
- `ScenarioKepNyitottsag` (0xbbb048) toward its `Cel` (0xbbb04c) by 2.0 (f32): going down to `Cel`: `= 0; Cel = 100.0; SelectedScenarioOld = SelectedScenario; LoadScenarioKep()` (the inlined `SetKepName(0x3e, entry+8); UnLoadKep(0x3e); LoadKepToMemory(0x3e)` when the list is non-empty and the index is in range); going up only when not leaving and not `_Becsukas`, to 100.0.
### 5.3 Meteors and ships, per tick (exe arrays)
Meteor i = 0..2 (record at 0xbbabe4 + 0x10*i: x, y, z; speed 0xbbac44 + 4*i; `MeteorVoltHangjamar` bytes 0xbbac50 + i), k = 1.0, or while leaving `(1 - K/100)*10 + 1` (f32):
1. if `z < camz + 1000.0` (`kamerapont.z` 0xde0150): `z += speed*k`.
2. x: if `0 <= x < 500`: `x += 10.0`; then if `-500 < x < 0`: `x -= 10.0`. The same for y (f32).
3. if `-3000 < z < 3000` and `-150 < y < 1500` (all strict): if `0 < x < 500`: `x += 1.0`; then if `-500 < x < 0`: `x -= 1.0`.
4. if `z > camz + 1000.0` and (timer snapshot `> 50` or `MINDENEGYSZERRE`): respawn with three `rand()` in this order: `z = METMAX(-50000.0, 0x9140e4) - (float)(rand() % 2000)`; `x = (float)(rand() % 3500 - 2500)`; `y = (float)(rand() % 4500 - 2500)`; `MainAszteroidaTimer = 0` and the local snapshot = 0 (so one meteor respawns per tick, unless `MINDENEGYSZERRE`); `MeteorVoltHangjamar[i] = 0`.
Ship i = 0..7 (pos 0xbbac60 + 0x10*i, start 0xbbace0, target 0xbbad60, speed 0xbbade0; `UrhajoAtlatszosagok` 0xbbae70, `UrhajoElforgatasok` e 0xbbae90, `UrhajoVoltHangjamar` 0xbbaeb0; k = 1.0, leaving `(1 - K/100)*5 + 1`):
1. if `-3000 < z < 3000` and `-150 < y < 1500`: if `0 < x < 500`: `x += 1.0`, `e -= 0.005` (**f64** op, 0x4f6b47); then if `-500 < x < 0`: `x -= 1.0`, `e += 0.005`.
2. `if e > 0: e *= 0.99; if e < 0: e *= 0.99` (**f64** 0.99, 0x4f6f16); `|e| < 0.001` (f64 compare, 0x4f6e6c) -> 0; `e > 2.0` -> 2.0; `e < -2.0` -> -2.0 (f32 compares).
3. if `|z - target.z| >= 50.0`: `pos += speed*k` on all three axes (z from the value read at the loop top); else if (snapshot `MainUrhajoTimer > 200` or `MINDENEGYSZERRE`): `GenerateUrhajoPosok(i)` (5.5), `MainUrhajoTimer = 0`, snapshot = 0.
4. `alpha`: if != 0: `alpha -= 0.02` (**f64**, 0x4f6fc6), `< 0` -> 0 (1.0 -> 0 in 50 ticks; used as transparency, i.e. a fade-in).
### 5.4 The inactivity timer and the demo
`MainInactivityTimer` (0xbbb0c4) is **never incremented**. A byte scan of the exe for the absolute address finds only the compare `cmp [0xbbb0c4], 0xbb8; jle` at 0x4f7044 (signed 3000), the zero store at 0x4f7092 and zero stores at 0x4fe5f2 (`MainScr_Belep`), 0x4fe7e1 and 0x4fe8db. So the idle camera pull-back (`Cel = 4.0`) and any idle demo never start; the demo (`SelfPlaying`) comes only from the `demomode` command-line argument (parsed around 0x4dfa80..0x4dfac7).
### 5.5 `GenerateUrhajoPosok(i)` (exe 0x4f8380; ELF 0x58c1d0; **all f32**: operands 3000.0, 8000.0, 10.0, 2.0)
1. `rand() % 6 == 0` -> `UnhideObj(ship i)`, else `HideObj` (a ship is shown with 1/6 chance per respawn).
2. Camera position `P` = inverse(`CreateMatrix(elforgatas)`) (nine floats m0..m8) applied to `kamerapont` (c = 0xde0148 / 4c / 50) plus `forgaspont` (0xde0130 / 34 / 38): `P.x = m0*cx + m1*cy + m2*cz + fp.x`, `P.y = m3*cx + m4*cy + m5*cz + fp.y`, `P.z = m6*cx + m7*cy + m8*cz + fp.z` [reading order of the decompile; rows vs columns not verified].
3. `UrhajoElforgatasok[i] = 0`; draws in order: start.x = `rand() % 3000 - 1000`; start.y = `rand() % 3000 - 1500`; start.z = `rand() % 2000 - 9000`; target = (P.x, P.y, P.z + 3000.0); alpha = 1.0; pos = start; `s = (float)(rand() % 50)/10.0 + 2.0` (2.0..6.9); speed = `(target - start)*s/8000.0` per axis; `UrhajoVoltHangjamar[i] = 0`.
(The ELF takes the target x / y from `GetKameraPos()`'s two return registers; same value.)

## 6. Input

### 6.1 Ring hit-testing and boxes (inside `Update3D`; only while `Tav == 1.0` exactly, else nothing is active)
Ring centre = projected centre of mesh B (ELF `MainGetMeshCenter`, not decompiled; exe inline: `GetObjPos(mesh)`, each coordinate x 1024, `TransformPoint`, then x*639/639 and y*479/479), virtual pixels.
A ring is hit when `cx - 40 <= Mouse_x < cx + 40` and `cy - 40 <= Mouse_y < cy + 40` (f32 compare). The ring boxes are `Mouse_Szoveg_Set(id, 0, cx - 40, cy - 40, 80, 80, text, 1, 1)` + `Mouse_Szoveg_On(id)`
(AktIkon2 == id sets `AktKijelolt`). A direct ring acts on `Mouse_Lenyom && Mouse_Bal` in `Update3D`; a box ring acts through `MainScr_Main` on the release. Hover (both kinds): `AktKijelolt = i`, `Mouse_Tipus = 1` (hand).
Ring i, mesh B = exe 0xd84378 + 0x1c*i. Tooltip text = `GetString(15, 1, line)`.

| Mode (and condition) | ring | action | label texture | box id (text line) -> `AktIkon` |
|---|---|---|---|---|
| `Mode_Main` | 0 | direct: credits (`IdoPauseMertAtlepesAlattVan = 1; OpenBink("misc/credits.bik",1,0,0,1); PlayBink(1,0,0); CloseBink(); IdoPause... = 0`) | `0_credits` | none |
| | 1 | box: quit | `0_exitus!` | 8 (0x19) -> 0x26 |
| | 2 | box: load game | `0_loadgam` | 7 (0x18) -> 0x25 |
| | 3 | direct: multiplayer (6.5) | `0_multipl` | none |
| | 4 | direct: `Mode_Main = 0; Mode_SinglePlayerSelected = 1; Cel = 5.0` | `0_singlep` | none |
| | 5 | box: tutorial menu | `0_tutoria` | 0xc (2) -> 0x2a |
| `Mode_SinglePlayerSelected`, no race choice | 0 | box: campaign | set 2 `0__camp` | 0xd (3) -> 0x2b |
| | 1 | box: scenario / skirmish list; a second box 1 (0x1b) on the same ring is created and switched off at once (-> 0x1f, the debug quick start) | `0__scenar` | 2 (0x1a) -> 0x20 |
| | 2 | direct: back: `Mode_Main = 1; Single = 0; Multi = 0; Cel = 5.0` | `0__back` | none |
| `Mode_MultiPlayerSelected` | 0 | box -> 0x23 (6.5) | set 3 `0___local` | 5 (0x1c) -> 0x23 |
| | 1 | box -> 0x22 (6.5) | `0___inet` | 4 (0x1d) -> 0x22 |
| | 2 | box 6 (0x1e) -> 0x24 (no handler) and direct back (`Mode_Main = 1; Single = 0; Multi = 0; Cel = 5.0`) | `0__back` | 6 -> 0x24 |
| `Mode_Tutorial_Valasztas` | 0..4 | direct: `Mode_Tutorial_Valasztas = 0; Cel = 6.0; AktKep = STARTSCREEN; Atlepes = 1; Mode_Tutorial_Start = 1; TutorialStartMelyik = i + 1` | set 5 `tutorial/part1..5` | none |
| | 5 | direct: back: `Mode_Main = 1; Mode_Tutorial_Valasztas = 0; Cel = 5.0` | `0__back` | none |
| single + `Mode_Fajvalasztas`, `Tav <= 1.333` (race confirmed, `MaxMainKorok` 3) | 0, 1, 2 | box: difficulty index 0, 1, 2; box 0x14 is switched off | set 4 easy, hard, normal | 0x11 (0x1f), 0x12 (0x20), 0x13 (0x21) -> 0x2f, 0x30, 0x31 |

Ring labels versus actions in the multiplayer menu: ring 0 shows the "local" texture but its box leads to 0x23 (server path), ring 1 shows "inet" and leads to 0x22 (matchmaking): check against a capture (the remaster rewired them).
`AktIkon` 0x21 (LAN: `EN = 1; Cel = 6.0; ProgramMode = 2; LogonStartScreen(true, false); AktKep = 0xe; Atlepes = 1`) needs box id 3, which nothing creates here (dead).
No `PlaySoundGlobal` in these paths.

### 6.2 Static boxes (`MainScr_MouseInit`, exe 0x4ff980): `MouseSzovegSzam = 0x27`, `MouseHanyasIkonKep = 0`
`Mouse_Szoveg_Set(id, 0, x, y, w, h, GetString(15, 1, line), releaseFlag, 1)`:

| id | x, y, w x h | line | rel | use |
|---|---|---|---|---|
| 9, 10, 11 | 123 / 263 / 403, 240, 120 x 12 | 1 | 0 | the three IP list entries (`AktIkon` 0x27..0x29; hover colours in `Gfx2D_2`) |
| 0xc | 146, 168, 347 x 91 | 2 | 0 | default (re-set by `Update3D` as the tutorial ring box) |
| 0xd | 0, 0, 640 x 480 | 3 | 1 | default (re-set as the campaign ring box) |
| 0x15 | 430, 438, 87 x 34 | 4 | 1 | scenario list: start |
| 0x16 | 523, 438, 87 x 34 | 5 | 1 | scenario list: cancel |
| 0x17 | 435, 65, 168 x 355 | 6 | 1 | list rows |
| 0x18 / 0x19 | 39 / 76, 408, 33 x 15 | 7 / 8 | 1 | skirmish race `<-` / `->` |
| 0x1a / 0x1b | 39 / 76, 446, 33 x 15 | 9 / 10 | 1 | computer players |
| 0x1c | 615, 64, 14 x 364 | 11 | 0 | scroll bar |
| 0x1d / 0x1e | 114 / 151, 446, 33 x 15 | 12 / 13 | 1 | difficulty |
| 0x1f / 0x20 | 189 / 226, 446, 33 x 15 | 14 / 15 | 1 | planets |
| 0x21 / 0x22 | 264 / 301, 446, 33 x 15 | 16 / 17 | 1 | technology |
| 0x23 / 0x24 | 339 / 376, 446, 33 x 15 | 18 / 19 | 1 | money |
| 0x25 / 0x26 | 114 / 151, 390, 33 x 15 | 20 / 21 | 1 | population |
| 0x27 | 300, 446, 110 x 19 | 22 | 1 | scenario info / high score toggle |

Ids 1..8 and 0xe..0x14 are set at run time (ring boxes above; 0xe..0x10 are the three race spheres, set by `ForgatFajvalasztasDolgai`, not decompiled).
`MainScr_MouseInit2` (exe 0x4ff7b0), at entry: ids 9..11 stay on (per non-empty IP entry) only while IP entry / name / high-score name / IRC / disconnected is active, else 9..0xb off; 0x15..0x17 on only if `Mode_Scenario_Valasztas` and `ScenarioValasztasNyitottsag > 99`;
then off: 1..8, 0xc, 0xd, 0xe..0x14; 0x18..0x1b and 0x1d..0x26 on and 0x27 off only if `Mode_Scenario_Valasztas`, the list is non-empty, entry `SelectedScenarioOld` (0xbbb354) is a skirmish (kind > 0) and `ScenarioKepNyitottsag == 100`, otherwise they are off and 0x27 on;
0x1c on iff `Mode_Scenario_Valasztas`, list non-empty and `ScenarioKepNyitottsag == 100`; with `EnterHighScoreNameMode`: 0x15..0x27 all off.

### 6.3 `MainScr_Main` (exe 0x4fefc0), every frame; `AktIkon` = once on release, `AktIkon2` = held
| value | action |
|---|---|
| `AktIkon` 0x2b | `Cel = 6.0; Mode_Fajvalasztas = 1;` race-choice init (ELF `InitFajvalasztas()`; exe calls 0x4f89b0); `VanFajVLight1 = 0`. Then (same frame) `EnterIPAddressMode = 0; MultiCloseInfoSocket(true)` |
| 0x1f | `Cel = 6.0; SetCampaignGameOptions()` (exe 0x503d40, debug quick start); `AktKep = STARTSCREEN; Atlepes = 1` |
| 0x2a | `Mode_Tutorial_Valasztas = 1; Mode_Main = 0; Cel = 5.0` |
| 0x20 | `ScenarioGombraRanyomtak("")` (exe 0x503bf0: opens the scenario / skirmish list) |
| 0x21 | LAN (dead, 6.1) |
| 0x23 | `ig2server_init(true)` (exe 0x55a7c0); on failure `activateScreen(hash("matchmaking"), 0, 0xfa)`; else `IdoPauseMertAtlepesAlattVan = 0; EN = 1; Cel = 6.0; ProgramMode = 1; LogonStartScreen(true, true); MultiConnectServer(); AktKep = 0xe; Atlepes = 1` |
| 0x22 | `ig2server_init(true)`; `activateScreen(hash("matchmaking"), 0, 0xfa)` |
| 0x25 | load game: `SingleStartGame(); Cel = 6.0; SetIdoSpeed(1, false); OldIdo = OldIdo2 = (SDL_GetTicks()/0x32)*0x32; ProgramNowStarting = 1; OptionsCsakLoadLehet = 1; AktKep = 0xd; Atlepes = 1; Mouse_FelengedesMajdMegvar = 1` |
| 0x26 | quit: `MouseIkonKep = 0; ig2_running = 0` |
| 0x27..0x29, `EnterIPAddressMode` | IP list entry n - 0x27 (`MainScr_GetIPByOrder`, exe 0x4fe8f0): if non-empty: `Mouse_FelengedesMajdMegvar = 1`, copy into `IOString`, `MultiOpenInfoSocket()` (ELF `MainScr_IPEntered()`) |
| 0x2c..0x2e, `Mouse_Lenyom` | race n = id - 0x2c: if `LastKijelolt == n`: `FajValasztasFajKivalasztva = 1; FajVSzetnyilasCel = 0; MaxMainKorok = 3; Cel = 1.3; FajValasztasSelectedRace = n`; else `LastKijelolt = n; InitKiirInfoSzoveg(); InfoBelerakSzoveg(n)` |
| 0x2f..0x32, `Mouse_Lenyom` | `FajValasztasStartGame = 1; FajValasztasFajKivalasztva = 0; FajValasztasNehezsegiSzint = id - 0x2f; Cel = 5.0; FajVSzetnyilasCel = 100; IdoPauseMertAtlepesAlattVan = 0` (then `Update3D` leaves the screen at `FajVSzetnyilas == 100`) |
| `AktIkon2` 0x15, `Mouse_Lenyom`, list non-empty | start: `IdoPause... = 0; Cel = 6.0; AktKep = 2; Atlepes = 1; Mode_Scenario_Valasztas_Becsukas = 1; Mode_Scenario_Vagy_Scirmish_Start = 1; ScenarioKepNyitottsagCel = 0` |
| 0x16, `Mouse_Lenyom` | cancel: ELF `CancelScenarioValasztas()`; exe inline: `Mode_Scenario_Valasztas_Becsukas = 1; ScenarioKepNyitottsagCel = 0; MaxMainKorok = 3` |
| 0x17, `Mouse_Lenyom` | row r = clamp((`Mouse_y` - 0x41)/0xf, 0, 0x16) + `ScenarioStart`; if r < count and r != `SelectedScenario`: `SelectedScenario = r`; if the entry is a skirmish (kind > 0) `ScenarioHighScoreMode = 0`; `InitScirmishValtozok()` (exe 0x4f8f80); if `ScenarioKepNyitottsag != 0`: `ScenarioKepNyitottsagCel = 0` |
| 0x18..0x26, `Mouse_Lenyom` | `PlaySoundGlobal(0)`, then +-1 (the first id of each pair 0x18/0x19, 0x1a/0x1b, 0x1d/0x1e, 0x1f/0x20, 0x21/0x22, 0x23/0x24, 0x25/0x26 decrements, the second increments): race 1..8, computers 1..7, difficulty 1..4, planets 0..4, technology 0..3, money 0..5, population 0..3 |
| 0x1c, `Mouse_Lenyom` or `Mouse_Megfogva` | `ScenarioStart = (int)(((float)Mouse_y - 64.0)/364.0*(float)count - 11.5)`, then clamp: < 0 -> 0; `count < ScenarioStart + 23` -> `count - 23`; < 0 -> 0; `Mouse_Megfogva = 1; DesignKeretAzonnalMorph (0xad0ef5) = 1` |
| 0x27 | `ScenarioHighScoreMode = !ScenarioHighScoreMode` (every frame the box is held, no edge test) |

### 6.4 Keyboard (`MainscrKeyboardHandler`, exe 0x5000c0)
Only when `DEBUGLEHET` is set (command line): key 13 (Enter) pressed and not `YouAreDisconnectedVolt`: `HanyGepiJatekos (0xf21488) = 7; Main_NemKellZoom = 1; AktIkon = 0x1f; MainScr_Main(); AktIkon` restored (debug quick start).
Nothing else here; text entry goes through the generic `IOMode` code, Esc through `MainScr_CancelIPEnter` (exe 0x4fe6f0) and `MainScr_ESCPressedWhenMultiplayerselect` (0x4fe780), decompiled in pass 2:
Esc: `MainScr_ESCPressedWhenMultiplayerselect` (exe 0x4fe780; ELF 0x59ecc0), only if byte 0xdb9439 == 0; the first matching case wins:
1. `Mode_Scenario_Valasztas && !_Becsukas`: `_Becsukas = 1; ScenarioKepNyitottsagCel (0xbbb04c) = 0; MaxMainKorok = 3`.
2. `YouAreDisconnectedVolt` (0xbbd144): cleared.
3. `EnterIPAddressMode` (0xbbb365) or `EnterIRCAddressMode` (0xbbb367): `Cel = 1.0; MainInactivityTimer = 0` (the text entry itself is cancelled by the generic IO code / `MainScr_CancelIPEnter`).
4. not `Mode_Fajvalasztas`: if `Tav == 1.0` (exact) or `EnterNameMode` (0xbbb364): `0xbbb3f4 = 0` (probably `Mode_Tutorial_Valasztas` [guess]), `Mode_MultiPlayerSelected = 0`, `Mode_SinglePlayerSelected = 0`, `Mode_Main = 1`, `Cel = 5.0`, `MainInactivityTimer = 0` (back to the main ring set).
5. race choice: only if not `_Becsukas`, not `FajValasztasFajKivalsztvaBecsukas`, `FajVKINYITVA`, not `StartGame`, and `FajVSzetnyilas == its Cel` (0xbbb16c == 0xbbb170): if the race is confirmed (0xbbb163): `confirmed = 0; FajValasztasFajKivalsztvaBecsukas = 1; Cel = 6.0; FajVSzetnyilasCel = 100.0` (back to the three spheres); else `FajVSzetnyilasCel = 0.0; Cel = 5.0; Mode_Fajvalasztas_Becsukas = 1` (close the race choice).
`MainScr_CancelIPEnter` (exe 0x4fe6f0; ELF 0x59ec00), by `IOAzonosito` (0xbae2c4): 0x97 (IP entry): `IOMode (0xbae2c3) = 0; EnterIPAddressMode = 0; MultiCloseInfoSocket(1)`; 0x98 (name entry): `IOMode = 0; EnterNameMode = 0;` and if `Mode_MultiPlayerSelected`: the Esc handler above; 0x9a (IRC entry): `IOMode = 0; EnterIRCAddressMode = 0;` and the saved text (0xbbb368) is copied back into `IRCAddr` (0xbbd148).

### 6.5 Multiplayer entry
Direct ring 3 in `Mode_Main`: `IdoPauseMertAtlepesAlattVan = 0; Mode_Main = 0; Mode_MultiPlayerSelected = 1; Cel = 5.0; EnterNameMode = 1; IOMode = 1; IOAzonosito = 0x98; IOString = MultiplayerNev (exe 0x914800); IOMaxLen = 9;
IOX = 0x11c; IOY = 0xe6; IOCharSize = 3; IOColor = 2; IOKurzor = strlen; IOGombnyomasraTorles = 0`; the name box is drawn by `Gfx2D_1` / `_2` (section 7). The three rings come after the name is accepted (generic IO code). `matchmaking` is a remaster screen
(`game::activateScreen(string_hash, 0, 0xfa)`; the exe computes the hash inline with different constants at the two sites: `(h*0x1003f + 0x6e)*0x7e0f81 + 0x671959` for 0x23 and `h*0x2e86d0bf + 0x3691c2c7` for 0x22, h = `FUN_004350a0("matchmaking")`).

## 7. 2D passes (sub-states only; the main menu itself draws no 2D)
`GFX_MainScr` only calls `Update3D`; `MainScrGfx2D_1/_2/_3` are the 2D passes (exe addresses of `_1`/`_3` not given). Brushes: `MainScrBrushok` (exe 0x913de8, ELF 0x1184740; 11 records x 0x38 bytes;
words `[x0, y0, dx, dy, w, h, stepx, stepy, cols, kep, ...]`): 1 = kep 92, dst (146, 168), 347 x 91 (the "disconnected" box); 2 = kep 87, 640 x 480; 3 = kep 62 (0x3e, the scenario picture loaded by `SetKepName(0x3e, ..)`), 380 x 238;
4..10 = keps 65..71, dst (42, 340), 64 x 64 (race pictures for races 1..7). Brush 0 is empty.

### 7.1 `MainScrGfx2D_1` (ELF; faders; `FaderHanyadikMenet = 2`)
`KirakFader(n, mainf, x, y, w, h, 0, mainf.f[n]*54, mainf.f[n]*90, T1, 0, mainf.f[n]*90, mainf.f[n]*130, T2, false, 0.005, 0.005)` with `T1 = 100`, `T2 = 30` for the dialogs. Fader index n counts up as boxes are added in this order:
- `YouAreDisconnectedVolt`: (111, 163, 420 x 98), (270, 220, 100 x 30).
- `EnterIPAddressMode`: (111, 163, 420 x 98). `EnterNameMode`: (111, 163, 420 x 98). `EnterHighScoreNameMode`: (60, 134, 330 x 130).
- `EnterIRCAddressMode`: (111, 163, 420 x 120) and three buttons (141, 233), (271, 233), (401, 233), each 100 x 30.
- `Mode_Scenario_Valasztas`: `t = 1 - ScenarioValasztasNyitottsag/100`, `T1 = t*155 + 100`, `T2 = t*225 + 30`: (430, 30, 180 x 400), (435, 35, 170 x 21), (435, 62, 170 x 362), (430, 438, 87 x 34), (523, 438, 87 x 34), (35, 30, 380 x 27),
  (35, 340, 380 x 130), (39, 345, 70 x 60) unless `ScenarioHighScoreMode`; then for a scenario entry (kind <= 0) the box (300, 446, 110 x 19); for a skirmish entry (`t = 1 - (Nyitottsag/100)*KepNyitottsag/100`):
  (39, 408, 33 x 15) if race > 1, (76, 408) if race < 8, (39, 427, 70 x 15), (39, 446) if computers > 1, (76, 446) if computers < 7, (114, 345, 70 x 40), (114, 410, 70 x 32), (189, 410), (264, 410), (339, 410) (each 70 x 32),
  then `<-` / `->` boxes (33 x 15) at y 446: difficulty (114 if > 1, 151 if < 4), planets (189 if > 0, 226 if < 4), technology (264 if > 0, 301 if < 3), money (339 if > 0, 376 if < 5); population at y 390 (114 if > 0, 151 if < 3).
  Then the track (613, 62, 18 x 368) and the knob (615, 64 + off, 14 x h), `h = min(364, 23.00023/count*364)` (364 if the list is empty), `off = max(0, ScenarioStart/(count*1.00001 - 23)*(364 - h))`.
- `FaderHanyadikMenet = 1`; in the list with a non-empty list: `BrushClipToScreenMelyikXY(3, 1, (int)(35 - sin((1 - ScenarioKepNyitottsag/100)*pi/2)*600), 0x50)` (the scenario picture sliding in from the left).

### 7.2 `MainScrGfx2D_2` (exe 0x4fc8d0)
`KiirStr(text, x, y, len, size, colour, style)`. `KiirStrSzinSzorzo` (0x903510) = `(int)ScenarioValasztasNyitottsag` as the fade, or `(Nyitottsag*KepNyitottsag)/100` for the texts that belong to the picture. Texts `GetString(15, 2, line)` (lines from the exe).
Scenario list (`Mode_Scenario_Valasztas`):
- picture (unless `ScenarioHighScoreMode`): `Brush3(MainScrBrushok[race + 3].kep, a, a, a + 42, a + 340, c*64, c*64)` with `a = (1 - c)*32`, `c = picture fade/100` (race = entry race, or `SelectedScirmishRace` for a skirmish) [argument names a guess].
- line 1 + ":" at (0x1b8, 0x28, 0x1e, 3, 4, 1); start = line 2 at (0x1d9, 0x1c1, 7, 3, 7, style 2 on hover of 0x15 else 1); cancel = line 3 at (0x236, 0x1c1, 7, 3, 7) (hover 0x16).
- rows k = `ScenarioStart` .. +22: name at (0x1b9, 0x46 + 0xf*(k - start), len, 2, 4, style 0x22; 0x25 for skirmish entries; 2 for the selected row); line 4 + ":" at (0x28, 0x28, 0x1e, 2, 4, 0x25); selected name at (100, 0x28, 0x1e, 2, 4, 0x22);
  race name `Faj_Nev[race]` (exe table 0xbb6a04) at (0x2c, 0x18c, 10, 1, 5, 0x22) unless high-score mode.
- scenario entry (kind <= 0): line 0xd (info) or 0xe (high scores) at (0x163, 0x1c4, 0x1e, 2, 7), style 2 on hover of 0x27 else 0x25 (the toggle).
- skirmish entry: `<-` (exe string 0x856a88) / `->` (0x856460) at (0x29 / 0x4e, 0x19c) race, (0x29 / 0x4e, 0x1c2) computers, (0x74 / 0x99, 0x1c2) difficulty, (0xbf / 0xe4, 0x1c2) planets, (0x10a / 0x12f, 0x1c2) technology,
  (0x155 / 0x17a, 0x1c2) money, (0x74 / 0x99, 0x18b) population; size 2, colour 5, len 5, style 2 on hover else 1, drawn only while the value can move that way. Values at y 0x1b1 (size 1, colour 5, len 10, style 0x22),
  labels at y 0x1a1 (style 0x25): computers `sprintf(line 5, computers + 1)` at x 0x2c; difficulty name (table exe 0xbbb248, stride 0x14) at 0x77 with label line 6 at (0x77, 0x1a1); planets `sprintf(line 7, ScenarioKezdoBolygoSzamok[n])` at 0xc2 with label line 8 + ":";
  technology `sprintf(line 9, name table exe 0xbbb2b0 + 0x14*n)` at 0x10d with label line 10 + ":"; money `"$ %d"` (ScenarioKezdoPenzek) at 0x158 with label line 0xb + ":"; population name (exe 0xbbb300 + 0x14*n) at (0x77, 0x170) with label line 0xc + ":" at (0x77, 0x160).
- right header: line 6 + ":" at (0xfa, 0x28, 0x1e, 2, 4, 0x25) and the entry difficulty name (exe 0xbbb248 + 0x14*entry[+0x34]) at (0x143, 0x28, 0x1e, 2, 4, 0x22).
- description lines (10 strings of the entry) at (0x2a, +0x48 for rows 5..9 of a scenario; 0x15a + 0xd*k; len, 2, 4, 0x22).
- high-score table (high-score mode, or a skirmish entry): line 0xf formatted with the scenario name at (0x2d, 0x15a, 0x32, 3, 4, 1); rows k = 0..9: `"%02d."` at (0x2d, or 0xe1 for k >= 5; 0x173 + 0xd*(k mod 5)); the name (entry strings) at (x + 0x16, y + 0x15a - 0x173..., 0x14, 2, 4, 2)
  or line 0x11 (empty slot, style 1); the score = line 0x10 formatted with `score/0x32` at (x + 0x5c, ..., 0xc, 2, 4, 2). Selected row frame: `DesignKeret1(0x1b5, 0x43 + 0xf*(SelectedScenario - ScenarioStart), 0xa5, 0xf, 0x22)`.
Dialogs by `IOMode` / `IOAzonosito`:
- 0x97 IP entry: `GetString(15, 3, 1)` at (0xa6, 0xb2, IOMaxLen, 4, 4, 1); `IOString` at (IOX, IOY - 9, IOMaxLen, IOCharSize, 5, IOColor); list entries k = 0..2 at (IOX - 0x25 + 0x8c*k, 0xf0, 0xf, 3, 4): "000.000.000.000" in style 1 when empty, else style 2 / 3 (hover of id 9 + k) or 7 / 6 for a flagged entry.
- 0x98 name: `GetString(15, 3, 2)` at (0x140, 0xbc, 0x28, 4, 7, 1); text at (IOX, IOY - 9).
- 0x99 high-score name: `GetString(15, 3, 3)` formatted with `scenario_score/0x32` at (0xe1, 0xa1, 0x28, 3, 7, 1); `GetString(15, 3, 2)` at (0xe1, 0xbf, 0x28, 4, 7, 1); text at (IOX, IOY).
- 0x9a IRC: line 4 at (0x140, 0xb6, 0x28, 3, 7, 1); button lines 5 / 6 / 7 at (0xbf / 0x141 / 0x1c3, 0xf3, 10, 3, 7), style 2 on hover; hover rects x 0x8d..0xf1 / 0x10f..0x173 / 0x191..0x1f5, y 0xe9..0x107 (exe unsigned tests `x - 0x8d < 0x65`, `y - 0xe9 < 0x1f`).
- The text cursor blinks with `AktIdo200`: `FillToScreen(x, y, CharXMeret[IOCharSize], CharYMeret[IOCharSize], 1, false)` after the last character (centred strings: `IOX + (IOMaxLen - len)*cw/2 + cw*IOKurzor`).
- IRC button actions (press): 1 connect (`IOMode = 0; EnterIRCAddressMode = 0;` call on `IRCAddr` 0xbbd148; failure `YouAreDisconnected = 1`, reason 0xe; success `Cel = 6.0`, clear IRC vars, `MultiOpenInfoSocket(2)` / `SendQuery(2)`, `AktKepKell = 0x10`;
  then `SaveGlobalRegistrySettings()`); 2 default server (first line of file `ircsrv`, else "irc.imperiumgalactica2.com:7000", into `IRCAddr`, `IOKurzor = strlen`); 3 cancel = `MainScr_CancelIPEnter()`.

### 7.3 `MainScrGfx2D_3` (ELF)
Only with `YouAreDisconnectedVolt`: OK hover rect 0x10d < x < 0x173, 0xdb < y < 0xfb -> style 2 (else 1); `Mouse_Gomb && Mouse_Lenyom` there clears `YouAreDisconnectedVolt`; texts `GetString(0x13, 6)` at (0x140, 0xb4, 0x28, 3, 7, 2) and
`GetString(0x12, 4)` at (0x140, 200, 0x28, 3, 7, 2); "OK" at (0x140, 0xe5, 0x28, 3, 7, style). The box is brush 1 (kep 92).

## 8. Sounds and music
- `MainInitSounds` (ELF 0x593b60 decompiled; exe 0x4fa9f0 not; only if `SOUND` (exe 0x919930) and not `mainsoundinited`): `MainLoadSound` of `sounds/screens/start/Asteroid1.wav`, `Asteroid2.wav`, `Asteroid3.wav` -> `MeteorHangHandlek[0..2]` (exe 0xbbb0dc; a row of 3 handles per meteor, stride 0xc); `SoundEngine::Duplicate` copies row 0 into rows 1 and 2 (3 meteors x 3 variants). `Shipout1.wav`, `Shipout2.wav`, `Shipout3.wav` -> `UrhajoHangHandlek` (exe 0xbbb100, rows of 3, stride 0xc), duplicated into rows 1..7 (8 ships). `MainDeInitSounds` (exe 0x4fa970) releases them (leaving to a screen other than 0xe / 0x10 / 0xd).
- `MainPlaySound(isMeteor, idx, variant)` (ELF 0x593860): wave status 1 (idle): `SetVolume(ConvertVolume(70.0))`, `Play(0)`; status 2 (playing): `Stop`, then the same. Callers: section 4 steps 6 and 7 (variants by speed). The variant is chosen once per pass (`MeteorVoltHangjamar` / `UrhajoVoltHangjamar`; reset by the respawns of 5.3 / 5.5).
- `MainSetSoundPan(isMeteor, idx, x, y, z)` (ELF 0x5939b0; exe 0x500040 `(char isMeteor, int idx, float x)`, only x is passed): if `SOUND`: for k = 0..2: `h = (isMeteor ? MeteorHangHandlek : UrhajoHangHandlek)[idx*3 + k]`; `SoundEngine::GetStatus(h)`; if status == 2 (playing): `SetPan(h, ...)` (exe `FUN_0055e590(&h, (int)x)`, ELF arguments dropped in the decompile). **`SetPan` is a no-op** (`re/screens/audio.md`: `SE_Wave::SetPan` is empty in the ELF; the exe function 0x55e590 is the 0x10 bytes before `SetVolume` 0x55e5a0, probably `ret 8`, not decompiled): the port needs no panning.
- No `PlaySound` / `PlaySoundGlobal` in `HozzaadKorok`, `HandleLoadingMesh`, `MainScrInitNullBoxes`, `MainScr_Kilep`, the timers or the ESC handlers: the main menu's rings click silently in the code read. Skirmish arrows: `PlaySoundGlobal(0)` (6.3). Music: `PlayMusicMenu(false, true)` on a fresh entry only (music id 0xc). Video audio: `title.bik`, `credits.bik`.

## 9. Other sub-states (for the next session)

### 9.1 Campaign start (`CampaignGameStart`, exe 0x4f5e90; ELF 0x58a8a0)
Condition: `FajValasztasStartGame && FajVSzetnyilas == 100.0`. `ProgramMode (0xbae544) = 0; SingleStartGame(); SetIdoSpeed(1, 0); ProgramMachines (0xbae550) = 1; ProgramPlayers (0x912acc) = 8; ProgramPlayers2 (0x912ad0) = 0x12`;
players i = 1..18: `PlayerKi[i]` (0xbae560) = 2, `PlayerFaj[i]` (0xbae574) = i, bytes 0xbae583 = 1 (i = 0xf), 0xbae584 = 3 (0x10), 0xbae585 = 1 (0x11), 0xbae586 = 1 (0x12), name = `Faj_Nev[PlayerFaj[i]]` (table 0xbb6a04) copied to `PlayerNev + 0xb*i` (0xbae587);
`ReadDebugData()`; `Nehezseg` (0x92827c) = 1 for `FajValasztasNehezsegiSzint` 0, 2 for 2, 4 for 1 (textures easy, hard, normal: easy 1, hard 4, normal 2);
`InitNewGame(seed = (0xb7725c ? that : SDL_GetTicks()), 0, FajValasztasSelectedRace + 1, Nehezseg)`; `OldIdo (0xbae274) = OldIdo2 (0xbae278) = (SDL_GetTicks()/0x32)*0x32`; `ProgramNowStarting (0xbad107) = 1`;
then `st_ShowAnim` by `EN` (0x912a3c, set inside `InitNewGame`): 1 -> `intro_human.bik`, 3 -> `intro_politikus.bik`, 5 -> `intro_brutal.bik` (how race index 0..2 becomes 1 / 3 / 5: read `InitNewGame`).
`SetCampaignGameOptions` (ELF 0x59bcd0; the debug Enter path): the same setup with `EN = 1; PlayerKi[1] = 1`, `InitNewGame(seed, AktIkon == 0x2a ? 4 : 0, 0, 0)`, `StartStory(DAT_0144c4b0)` if set.

### 9.2 Race choice (`Mode_Fajvalasztas`)
`InitFajvalasztas` (ELF 0x58bef0): `LastKijelolt = -1; FajVFrame = 0; FajVKINYITVA = 0; FajVSzetnyilas = 0; FajVSzetnyilasCel = 100; MainKorMozgatoValtozo = 2.5; FajValasztasSelectedRace = -1;
FajValasztasFajKivalasztva = FajValasztasFajKivalsztvaBecsukas = FajValasztasStartGame = 0; UnhideObj(FajVMainMesh); UnhideObj(submeshes 3..8); FajValasztasNehezsegiSzint = 0; VisszaAllitFajvalasztasKezdoPoziciok()`.
The race-choice scene (`FajVMainMesh`, `LoadFajValasztasVektor`, texture dirs `textures/main/fejek`, `fejek2`, `fejek3`, race videos `misc/race_0N_loop|wait.bik` per `re/screens/README.md`, the race boxes 0xe..0x10, the info text `LoadFajInfoText` / `InfoBelerakSzoveg` / `KiirFajInfoSzoveg`)
is in the functions listed as not decompiled. Flow: 6.3 (0x2b opens it with `Cel = 6.0`, the rings fly out; `FajVFrame` runs to 90; a click on a race highlights it, a second click on the same race confirms it: rings come back as difficulty,
`Cel = 1.3`); a difficulty click closes the spheres (`FajVSzetnyilasCel = 100`), `Update3D` then sets `AktKep = STARTSCREEN`, `Kilep` plays the exit animation and calls 9.1.

Functions read in the second pass (brief; the next session does the screen):
- `VisszaAllitFajvalasztasKezdoPoziciok` (exe 0x4f89b0; contains the ELF `InitFajvalasztas` part): `LastKijelolt (0xbbb3e0) = -1; FajVFrame (0xbbb168) = 0; FajVKINYITVA (0xbbb162) = 0; FajVSzetnyilas (0xbbb16c) = 0; Cel (0xbbb170) = 100.0; MainKorMozgatoValtozo (0xbbb174) = 2.5; FajValasztasSelectedRace (0xbbb178) = -1; confirmed (0xbbb163) = 0; FajValasztasFajKivalsztvaBecsukas (0xbbb3f5) = 0; StartGame (0xbbb17c) = 0`; `UnhideObj(FajVMainMesh 0xcc1440)`; unhide sub-meshes 3..8; `0xbbb180 = 0`; then for the sub-meshes 2, 9, 10, 0, 0xb, 0xc, 1, 0xd, 0xe in this order: `MoveObj(sub, saved x, y, z)` (saved triples at 0xbbb198, 1a8, 1b8, 1c8, 1d8, 1e8, 1f8, 208, 218), `SetObjTransparency(0)`, `RotateObj(0, 0, 0)`.
- `LoadFajValasztasVektor` (exe 0x4f9410; ELF 0x58bc90): dirs `textures/main`, `vector/main`; `LoadBox("kezdo_.xvb", false)` -> `FajVMainMesh` (0xcc1440, id slot also `0xbbb164`), scale 0.611 (0x3f1c28f6), `MoveObj(0, 0, -800)`, `SetRelative(false)`, `PropertiesOff(2)`; parallel light colour 0xdcc8c8 direction (0, 0, -1000) (handle 0xcc1464); byte 0xbbb161 = 1; `HideObj`; ambient lights colour 0x646464 on sub-meshes 2, 0, 1 (handles 0xd842fc, 0xd84308, 0xd84314) each with `PropertiesOn(0x40000)`; `HideObj`; `ElkerFajvalasztasKezdoPoziciok()` (saves the sub-mesh start positions; not read).
- `ForgatFajvalasztasDolgai` (exe 0x4f7390; ELF 0x58af10), only `AktKep == 0xf`: `Mouse_Szoveg_Off_Range(0xe, 0x10)`; not `Mode_Fajvalasztas`: `HideObj(FajVMainMesh)`; else `UnhideObj`, `FejFenyekBeallitasa()`; until `FajVKINYITVA`: `SetFrame(mesh, FajVFrame)` (the opening animation 0..90); then `PropertiesOff(2)`, hide sub-meshes 3..8, `RotateObjr(sub, 0, 0, +-0.02)` per tick on sub-meshes 9, 11, 14 (+0.02) and 10, 12, 13 (-0.02) (f32 constants 0x3ca3d70a), then by state: (a) neither confirmed nor closing and not `StartGame`, not `_Becsukas`: three spheres k = 0..2 at angle `k/3*2pi + MainKorMozgatoValtozo`, radius `230*t` (t = FajVSzetnyilas/100), blend between the start position (table at exe 0x91408c, 4 floats per sphere) and the circle via `MozgatKor(...)` (exe sin / cos are `libm_sse2_*_precise`, doubles); when `t > 0.999`: race box `0xe + k`, rect (x - 80, y - 80, 160, 160) from `TransformPoint`, text `GetString(0xf, 1, 0x17)`, `Mouse_Szoveg_On`; (b) `_Becsukas`: the three spheres fly back (radius factor `1.5`, `sin((1 - t)*pi)*800 + 900` for z); (c) `StartGame`: the chosen head only (`MozgatKor(.., 0, 0, (1 - t)*700 + t*1400, t)`); (d) confirmed or confirm-closing: the picked sphere (index `FajValasztasSelectedRace`) goes to z `sin((1 - t)*pi)*800 + 700` with scale 0.001, the others scale away (factor 5). (rough reading of the decompile; exact `MozgatKor` argument order: read the asm.)

### 9.3 Scenario / skirmish choice
Opened by `ScenarioGombraRanyomtak("")` (ELF 0x59af90, exe 0x503bf0, not decompiled; sets `Mode_Scenario_Valasztas`, `SelectedScenario`, ...); also from `Belep` after a finished scenario (name entry for the high score). State: 6.2 boxes, 6.3 rows and buttons, 7.1 / 7.2 draw, 5.1 / 5.2 animation.
Entry record (exe, 32-bit): `+0` name ptr, `+4` file ptr, `+0xc..+0x34` description lines (10 x 4), `+0x34` difficulty index, `+0x38` race, `+0x3c` registry name, `+0x40` scores (10 x 4), `+0x68` names (10 x 4), `+0x90` skirmish kind (> 0 = skirmish).
ELF (64-bit): `+0` name, `+8` file, `+0x18` lines (8 each), `+0x68` difficulty, `+0x6c` race, `+0x70` registry name, `+0x78` scores, `+0xa0` names, `+0xf0` skirmish kind. Start = 2.2 (scenario file or `SetScirmishGameOptions`).

### 9.4 Tutorial
6.1 (five parts + back), then 2.2 (`TutorialSetCampaignGameOptions` 0x504250, `TutorialStart("main", n)`).

### 9.5 Demo (`SelfPlaying`, head of exe 0x4fe3c0; ELF `StartDemoMode` 0x59a870)
After the title video, scenario list and `LoadFajInfoText`: `ProgramMode = 0; SingleStartGame(); SetIdoSpeed(1, 0);` the 18-player setup of 9.1 with `EN = 1`, `PlayerKi[1] = 1`; `ReadDebugData()`; `InitNewGame(seed, 4, 0, 0)`; `StartStory(0xb77260)` if set;
`OldIdo = OldIdo2 = (ticks/0x32)*0x32`; `TutorialStart("demo", 0)`; return (no menu state). `DemoMode_Init` (exe 0x60b7f0) is the other screens demo hook.

### 9.6 Multiplayer entry and exit
6.5. IP entry (`IOAzonosito` 0x97, 3 list entries, box ids 9..11), IRC entry (0x9a), "disconnected" box: 7.2 / 7.3. Returning from `Start` (0xe) / `Irc` (0x10) re-enters in `Mode_MultiPlayerSelected` without rebuilding the scene (2.1 step 5). Quit: ring 1, immediate (`ig2_running = 0`), no dialog, no fade.

## 10. Exe vs ELF (differences seen)
1. The exe `MainScr_Belep` (0x4fe3c0) contains `StartDemoMode` as a branch; `LoadScenarioKep` and `UnLoadScenarioList` are inline / the exe name `ElinditTutorial` (0x553c20) is `UnLoadScenarioList`; the ELF tutorial start `ElinditTutorial(n)` is two calls in the exe.
2. `Main3DInit`: exe `rand() % n` (15-bit CRT) where the ELF has `random(n)`; the exe has `WaitForDrawing()` at the end and no `reloadtextures()` / `forgadd` write order differences; it calls `VisszaAllitFajvalasztasKezdoPoziciok()` where the ELF calls `InitFajvalasztas()`; texture tables are loops over pointer tables.
3. `Update3D`: exe computes the background-galaxy colour inline; uses `GetObjPos` + `TransformPoint` (x 639/639, y 479/479) instead of `MainGetMeshCenter`; the sound code is inlined (volume 70.0 = 0x4051800000000000); `MainSetSoundPan` for meteors gets x only; `CheckCampaignGameStart` is inline;
   no `ControlConfig[0]` (touch) test visible in the exe texture loops; the exe math is SSE (`libm_sse2_*_precise`, `ucomiss` / `comiss` with f32 constants, see 5.4 and 11), the ELF decompile shows doubles.
4. `MainScr_Main`: exe 0x2b calls 0x4f89b0 (ELF `InitFajvalasztas`), the IP path calls `MultiOpenInfoSocket()` (ELF `MainScr_IPEntered()`), the cancel (0x16) is inline (`MaxMainKorok = 3`, not `CancelScenarioValasztas()`), `STARTSCREEN` is read from 0x927a88.
5. `MainScr_Kilep`: tutorial path `TutorialSetCampaignGameOptions` + `TutorialStart` instead of `ElinditTutorial`; scenario records are 32-bit.
6. `MainScr_MouseInit2`: `MainScr_GetIPByOrder` inlined (list at 0xdb9448, count at +0xc, entries at +0x10, texts from +0xc + 4k, type flags at +0x19c).
7. `Gfx2D_2`: IRC buttons use unsigned range tests in the exe; the connect call is `IMG_InvertAlpha`-named (ported name probably wrong; the ELF is `Irc_Connect`); the exe `GetString` calls carry the line numbers the ELF decompile dropped (used above).

8. `FRISSIT_MainScr2` (second pass, exe 0x4f65f0 against ELF 0x59f360): the exe's literals are f64 for `M += 0.1`, `Tav +- 0.05`, `FajVFrame += 0.3`, `MozgatKorYadder` 0.3, `MainKorMozgatoValtozo += 0.001`, `FajVSzetnyilas -= 0.6`, the fades 0.005 / 0.02, `UrhajoElforgatasok` 0.005 / 0.99 / 0.001, `UrhajoAtlatszosagok -= 0.02`, `K -= 0.5`, `K/25`; f32 for the rest (the table in 5). The exe adds flag conditions on the 0xe / 0x10 fades (0x913990, 0xf21484, 0xf21485). Its meteor / ship tests are strict (`0 < x < 500`), the respawn uses `rand()`.
9. `ChangeGalaxyColors`: exe 0x4f6040 all f32 (`mulss` 255.0, 100000.0, 0.2) and the alpha through an `ftol` helper (0x819460); same value as the ELF.
10. `HandleLoadingMesh`: the exe computes `T = K/50` and the rotation `T*pi/2` in f64 (`divsd` 0x4f8654, `mulsd` 0x4f86ed), the ELF in f32.
11. `MainSetSoundPan`: exe passes `x` only; `SetPan` is a no-op in the ELF. `GenerateUrhajoPosok`: the exe inlines `GetKameraPos` (with the `forgaspont` terms) where the ELF reads two return registers. `HozzaadKorok`, `MainScrInitNullBoxes`, `VisszaAllit..`/`LoadFajValasztasVektor`: same calls and constants in both. The `random()` call at the end of the exe's `FRISSIT_MainScr2` loops and `GenerateUrhajoPosok` is the "index > 2 / > 7" artefact, not an RNG draw. In the exe `VisszaAllitFajvalasztasKezdoPoziciok` (0x4f89b0) also holds the ELF `InitFajvalasztas` assignments; the race-close path of `FRISSIT_MainScr2` calls `FUN_00504380` instead.

## 11. Check in the asm (exe address, what)
1. **0x4f65f0 (`FRISSIT_MainScr2`) is decompiled now**; the f32/f64 reading in 5 comes from a byte scan of the operands (prefix f2 = sd, f3 = ss) and must still be confirmed in the listing at: 0x4f6a3e (`addsd [0x852208] = 0.1`, preceded by `cvtps2pd`, followed by `cvtpd2ps` + `movss [0xbbabe0]`), 0x4f6776 / 0x4f684d (0.3), 0x4f67d5 (0.001), 0x4f67f9 (0.6), 0x4f6926 (0.005), 0x4f695e (0.02), 0x4f6b47 (0.005), 0x4f6ad7 / 0x4f6f16 (0.99), 0x4f6e6c (0.001 compare), 0x4f6fc6 (0.02), 0x4f70f1 (0.05), 0x4f71a5 (0.5), 0x4f71df (`divsd 25.0`). `MainScrMozgato` accumulates for minutes: the f64-add-then-round-to-f32 each tick is what the port must repeat.
2. 0x4f70b6..0x4f7184: the `Tav` step and clamp (the decompile of the compare chain is garbled): confirm that the new value is `Cel` when it reaches or passes it, and that the `Cel == 5.0` branch (`MaxMainKorok`, `Cel = 1.0`) runs only on reaching it going up. Also the compare `cmp [0xbbb0c4], 0xbb8` at 0x4f7044.
3. 0x4f6bf0..0x4f6d60 (meteor loop) and 0x4f6c9e..0x4f6fd0 (ship loop): the snapshot-and-zero of the timers (`iVar3`) and the order of the three `rand()` calls; the `do { } while (< 3)` loop end 0x4f6d9f; the `(1 - K/100)*10 + 1` (f32) factor (`divss 100.0` 0x4f6bec, `movss 10.0` 0x4f6ba2 is the +-10 step).
4. 0x4f6040 (`ChangeGalaxyColors`): the `ftol` helper call (0x819460) near 0x4f607x: confirm `A = (int)(GT*pa)` goes to bits 24..31 and the channel order (pb << 16, pg << 8, pr).
5. 0x4f8380 (`GenerateUrhajoPosok`): the matrix element order of the camera position (movss of ebp-relative slots 0x4f83be..0x4f84ae with `forgaspont` at 0x4f8457 / 8487 / 84ae), `rand() % 6` hide/show, divisor 8000.0 (0x4f8584) and 10.0 (0x4f859b).
6. 0x4f8620 (`HandleLoadingMesh`): the dropped `MoveObj` / `RotateObj` arguments (x of `MoveObj` is the constant 0, z is 0xc4af0000 = -1400.0); f64 at 0x4f8654, 0x4f86ed, 0x4f86fa.
7. 0x500040 (`MainSetSoundPan`): how `x` is passed to 0x55e590 (the decompile casts it to `int`) and the body of 0x55e590 (empty?).
8. 0x4f8240, ~0x4f8170 and the exe `GenerateKod2` (call sites in `Main3DInit`), 0x4f7e60 (`GenerateGalaxy`): not decompiled; confirm the ELF reading of 3.2 (draw order, the per-frame `srand(400)`, the colour byte order). Bytes `push 0x190` seen at 0x4f8185 and 0x4f8254.
9. 0x4f8780 (`HozzaadKorok`): nothing open except the meaning of the property bits (2, 4, 8, 0x20) in `sg3d` (`PropertiesOn` 0x456a50).
10. Items 3..11 of the first pass still stand: 0x500270 meteor block (labels 0x500a7b, 0x500b33, 0x500bc3: the dropped `SetObjTransparency` argument and `METKOZ`), the ring loop (`RotateObj` third argument), the ship block (loop ending at `< 0xd84360`, labels 0x500f25, 0x500fd5, 0x50105c: transparency, matrix operand order), the texture loops from 0x503271 (table choice, touch-mode test, loop bound), the hit-test (`* 639.0 / 639.0`), 0x4fefc0 (the two `activateScreen` hashes; `Cel = 5.0` joins), 0x4f96a0 (`random()` at 0x4fa969 is a bounds artefact; `(int)-METMAX`; `hanyhattergalaxis` store at 0xbbd13c; `sdl_width` / `sdl_height` 0xb7721c / 0xb772a0), 0x4f5e90 (`Nehezseg` stores, `InitNewGame` args), 0x4fecc0 (`mcd` mapping, the `comiss` loop test).

## 12. Unknowns (where I looked)
- Whether the ten `szrke{i}.xvb` files differ in geometry or are the same quad; the sub-mesh count of `korke1.xvb` (asset-finder on `vector/main`).
- The property bits 2, 4, 8, 0x20 (searched `re/screens/render.md`, `re/formats`): semantic unknown.
- The vertex-colour byte order of particles (which byte is red): not found in the decompiles; check by capture.
- Exe code not decompiled: `GenerateGalaxy` 0x4f7e60, `GenerateKod` 0x4f8240, `DeGenerateKod` (~0x4f8170), `GenerateKod2` (symbol address not read), `mainOldalsoFenyekBe` / `Ki`, `ElkerFajvalasztasKezdoPoziciok`, `FejFenyekBeallitasa`, `MozgatKor`, `FUN_00504380` (the exe call in the race-close path of the timer), exe `MainInitSounds` 0x4fa9f0, `MainScrGfx2D_1` 0x4fab30 / `_3` 0x4fe190, `ScenarioGombraRanyomtak` 0x503bf0, `FrissitFajInfoSzoveg` 0x4f7c60, `KiirFajInfoSzoveg` 0x4f9040, `InitKiirInfoSzoveg` 0x4f8ea0, `InfoBelerakSzoveg` 0x4f8910, `GetKameraPos`. `GenerateKod`, `DeGenerateKod`, `GenerateKod2` have **no decompile anywhere under `_local/decomp`**; I read them from the ELF disassembly (`_local/decomp/placement/ig2.asm`, lines 333426..333700).
- The RNG hypothesis of 3.2 (per-frame `srand(400)` making the respawns near-deterministic): not observed at run time.
- Click sounds of the rings: none in the code read; the generic hot-spot code (`Mouse_Darabol`, `Mouse_Szoveg` tooltip drawing, `Mouse_Tipus`) was not read; tooltip texts are `GetString(15, 1, n)` (asset-finder job).
- The scene-light mesh at ELF 0x11abab4 (is it `mainmeshid`?) and whether the exe creates the same two lights in the same function.

## 13. Runtime observations (`tools/trace/probes/mainscr.js`, `run_screen.py --boot`)
`run_screen.py <install> <save> <out> --kep 15 --run --boot --script tools/trace/scripts/mainscr.json --probe
tools/trace/probes/mainscr.js`: the menu as the exe boots (no scenario, no save; `--boot` in `screen.js`), one UI tick a
drawn frame. The probe logs `Mainscreen_Update3D`'s state (first 12 calls, then every 50th) and the centres
`TransformPoint` (exe 0x52cc90) returns.
- **Ring hit centres** = `ring_pos · 600/3000 + (320, 240)`: `GetObjPos × 1024` hands `TransformPoint` the `MoveObj`
  position itself, so the rings (and meteors, ships: `SetRelative(false)`) are camera-fixed; the camera (`kamerapont`,
  `elforgatas`) only moves the relative objects (galaxy mesh, clouds, pictures, dust): `e = M·p − kamerapont`,
  `M = CreateMatrix(elforgatas, 0, 0)`. At M 25.1: ring 0 (384.94, 701.30, −3000) → (396.99, 380.26) … (test
  `ring_centres_match_the_exe`).
- **Start**: `Update3D`'s first call comes 2 UI ticks after the script's t 0 with M 0, `Tav` 10, GT 0; from there
  every tick M +0.1, `Tav` −0.05 (1.0 at call 181), GT +0.003. `kamerapont` (200, −100, 2100) at M 0.
- **The libc stream is deterministic at boot** (two boots alike): the four galaxy clouds start 60 draws into the CRT's
  default stream (seed 1, no `srand`), in `Main3DInit` order (test `clouds_match_the_exe`: the first three particles
  of each cloud); the meteors and ships come from `srand(400)` + 7000 draws (after the last `GenerateKod2`), as §3.2
  computed: meteors (−4339, −956, −10050), (−1254, −3312, −18238), (−2493, −474, −8061), ships 0..7 at (977, −843,
  −8837) … (1623, −506, −7206), ship 0's speed (−0.6, 0.4, 11.8) (test `meteors_and_ships_match_the_exe`). The first
  meteor respawn (meteor 2, timer 449 at UI 500): (−660, −1709, −51219).
- **Input latency**: a script press at t 350 starts the fly-out as the client's press at t 352 does (`Mouse_Lenyom`
  reaches `Update3D` two ticks after the SDL event).
- **Sounds** (`--sounds`): no `PlaySound`/`PlaySoundGlobal`/`PlayWav`/`PlayMp2` for the hover, the ring presses or the
  fly-out (the rings are silent). The meteors' and ships' `MainPlaySound` (§8) goes through `SoundEngine` directly, not
  through the hooked functions: the action → sound table is "none" for every ring; the scene's sounds are the meteor
  (`Asteroid1..3.wav`, volume 70) when a meteor first passes z −9000 and the ship (`Shipout1..3.wav`) while a ship is
  within z ±3000 (§4 steps 6–7; for the audio module).
- **Lighting** [obs]: the meteors draw near black (a dim light only); the ships show the side lights' blue and warm
  highlights.

### 13.1 Capture comparison (`run_screendiff.py … main --script tools/trace/scripts/mainscr.json`)
Best-offset mean per region (pass ≤ 6), 2026-10-22, 800 × 600: frame 2.6 / 4.0 / 5.5 at t 5 / 60 / 120 (pass), 6.6–8.0
from t 180 on, 10.0 at t 360 (the fly-out). The rings, labels and hover sit on the exe's (grey on the heat map); what is
left is in `re/deviations.md`: the top-left picture's tint (greener), the galaxy mesh's arms (sharper, ~4 % bright),
the dust points, the small bottom galaxy, the exe's own mouse pointer (the client draws none yet), the fly-out's shape.

## 14. Port status (`ig2-client::mainscr`, `mainscr_scene`)
Ported: the 3D scene (rings + labels with the hover texture, galaxy mesh, the four clouds exact, the pictures, the dust,
meteors, ships), the 10 ms tick (§5.1 items 5–8, §5.2 fade-in, §5.3, §5.5), the ring hit-tests and the menu changes
(main ↔ single player / multiplayer / tutorial; back), load game (starts the game from `--save`'s save: the Options
screen's load slots are the next task) and quit. The window starts on the menu unless `--save` is given;
`ig2-client --snapshot OUT --screen main [--ui-frames N] [--mouse X,Y] [--script S]`.
Not yet: the title and credits videos (U30), the ring boxes' tooltips (the box texts are §6.1; whether the generic
tooltip draws on this screen is unverified), the exit animation and loading plates (§2.2, `HandleLoadingMesh`), the
race choice and difficulty (§9.2), the scenario / skirmish list (§9.3, §7), the tutorial start (§9.4), the multiplayer
name / IP / IRC entry (§6.5, §9.6), the high-score name entry, the meteor and ship sounds (`MainPlaySound`; the menu music `PlayMusicMenu` plays through `ig2-client::audio`), new-game
setup (`InitNewGame` exe 0x4e1130 is not in `ig2-sim`: §9.1).
