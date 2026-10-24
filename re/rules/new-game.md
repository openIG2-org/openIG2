# New game: `InitNewGame` (exe 0x4e1130) and the campaign start

Status: **modes 0, 1, 2 and 3 are ported and verified** (`engine/crates/ig2-sim/src/game/new_game.rs`:
`Game::new_campaign` / `Game::before_start` + `Game::start_campaign`, `Game::start_scenario` / `new_scenario`,
`Game::start_mp_galaxy` / `NewMpGalaxy`, `Game::start_skirmish` / `NewSkirmish`): 35 starts recorded in ig2.exe (5 campaigns,
15 scenarios, 5 MP galaxies, 10 skirmishes) give the same save at tick 0 chunk for chunk (`tests/new_game.rs`, tracer
`tools/trace/run_newgame.py` + `newgame.js`, seeds `tools/trace/scenario_seeds.py`, trace report
`traces/2026-10-23-new-game.md`). **Not ported:** the load path (mode 5) and mode 0 with config campaign=0 (falls to
skirmish defaults). The galaxy generation of §3 is verified through modes 2 and 3; §7's mode-0 keywords through the scenarios.
Section 13 has what the scenario and MP runs added.

Sources: decompiles `_local/decomp/newgame/{elf,exe}/`, `_local/decomp/campaign2/`. ELF = unstripped x64 `ig2`
(symbol oracle), exe = `ig2.exe` (normative). VAs are exe image VAs.

## What the runtime comparison corrected or added (normative over the sections below)
- **The starting state is the exe image.** A save taken right after the start equals the image's initial bytes for
  every saved global the start doesn't write (a pre-start dump of `.data` matched the image for all of them but
  `RandSeed` and `HanyBolygo2`, which the start overwrites). The port starts from the image (`ig2_rules::PeImage::
  read_loaded`, BSS zero) and applies the start.
- **Static constructors at process start** (in the pre-start dump, not in the image): every battlefield `WF[i]`
  has `+0x18` = 1, `+0x184` = 1 (byte), `+0x190` = 1 (byte), `+0x334` = −1, `+0x148` = −1, `+0x7fc..` 18 pairs
  (200, 300) and the camera `+0x1a8..+0x1b8` = (0xC4016383, 0x43F9EBEB, 0xC4898000, 0xBD4CCCCD, 0x40C4A3D7)
  (step 53 overwrites the camera of planets' battlefields with its own values).
- **`StringReadAll`** (exe 0x531e40) writes the **Hungarian** `txt17` names into the exe's tables at start-up,
  whatever the game language: `Tervek0[k − 1]` name = `strncpy(GetString(17, 7, k), 0x14)` (k = 1..47);
  `Talalmanyok0[t]` name = `strncpy(GetString(17, 8 + (t − 1) / 48, (t − 1) % 48 + 1), 0x10)` (t = 1..192);
  `Faj_Nev[r]` = (17, 1, r + 5); `AlapFlottaNev` from (17, 5, 46) of the same file (the AI conquerors are named
  "Flotta 1" on an English install).
- **The exe's `NEWGAME_Research`** does reset `ResearchTerulet` = 3, `ResearchOldForgato` = 3 and `ResearchTalalmany`
  = 0x21 (seen in every save), contrary to §1 step 39's reading.
- **`LoadScenario(map, 1)`** (the campaign map) loads neither `RandSeed` nor the relations: the streams after the
  start are `RandomInit`'s plus the start's own draws, and the relations' 5000s come from `INIT_MI`.
- **`INIT_MI`** (exe 0x5ade70, ELF decompile `_local/decomp/ai_elf/INIT_MI.c`): per empire 1..=ProgramPlayers2
  `VanUresBolygo` 0, `BirodalomEl` 1, countdown 100, main fleet/planet 0, `MiVeszelyAllapot` 1, the timers and
  centre 0, the scouting grid and the design slots 1..=24 cleared, `FRISSIT_MI_DESIGN(e, true)` if `BirodalomMIzik`;
  then for a, b = 1..=ProgramPlayers2 the relation record reset (words +0x22, +0x02..+0x06, +0x0c..+0x10, +0x1c,
  +0x1e = 5000, the others 0, byte +0x25 = 3, +0x24 = 0) and `DiplomaciaMikorHivtaUtoljara[a·9 + b]` = 0, which
  **runs past the table** (0xDAF520, 0x144 B) into `MiTervMutato` (0xDAF668): its rows 0..=7 are zero after the start
  (kept); then `FRISSIT_Viszony`, empires with `BirodalomHanyBolygo` 0 dead, the categories cleared.
- **`FRISSIT_Planet(false, 0)`** in the map tail is the display refresh with the `only == 0` prologue: the per-empire
  and per-planet resets (spy capacity 1, fire brigade 1, labs), each owned planet's building pass and production,
  and the totals (`ColonyWorld::display_refresh`).
- **Heap records** (never zeroed: fleet and ship slot 0, a new fleet's fields `FlottaNew` doesn't write, design
  records past the templates, building blocks `OldReallocate` grows by 20, including those `LoadScenario` grows)
  hold the heap's bytes; the port takes them from `Game::uninit` (zero without it), the test feeds the exe's.
- **The AI's design names** (`DesignPoenosanElnevez`, §14): ported; the test no longer copies the exe's names
  (a key off by one makes 7 chunk groups differ on every campaign and skirmish start).
- `NEWGAME_Diplomacy` reseeds libc with `SDL_GetTicks()`: [`NewCampaign::ticks`]; the port leaves `Game::libc` at
  that seed after the 90 draws.

## 0. Modes of `InitNewGame(seed, mode, campaign, difficulty)`

| mode | caller (exe) | what it is | what InitNewGame does for it |
|---|---|---|---|
| 0 | `CampaignGameStart` 0x4f5e90; `SetCampaignGameOptions` 0x503d40 (debug Enter) | campaign (race menu) | `CampaignType` = campaign (1..3), `EN` = {1,3,5}[CampaignType-1], PlayerKi[1]=2, PlayerKi[EN]=1, `HangLejatszasKell`=CampaignType. No stars (HanyBolygo := 0, the story's `LoadScenario` loads the map). Conqueror fleets + traders. `StartStory(debug id or 400)`. 2 extra `AddPlanet` for the other two main races. `Nehezseg==1` human bonus. |
| 0 with campaign 0 | `SetCampaignGameOptions` (config campaign=0) | falls to skirmish defaults | `mode := 3`, HanyGepiJatekos=7, pop=0, tech=0, planets=0x50, money=50000 (see section 1 step 1) |
| 1 | `Betolt_Scenario` 0x4f5ba0, `TutorialSetCampaignGameOptions` 0x504250 | scenario / tutorial; caller then `LoadScenario(file, 0)` | no stars, no fleets, no story start. `CampaignType` := 0. |
| 2 | multiplayer lobby (`FRISSIT_Start`, `ReceiveStartData`; §13; `BirodalomEl` for mode 2 only turns ON the humans) | MP galaxy generated by InitNewGame | like 3 but `BirodalomEl`, `HanyBolygo` come from the lobby; keeps `HanyBolygo` as set |
| 3 | `SetScirmishGameOptions` 0x503e80 | skirmish | generates the galaxy (Generate_Stars), skirmish money/tech/planets, `InitScirmishBolygok`, conqueror fleets + traders |
| 4 | `SetCampaignGameOptions` when `AktIkon==0x2a`; StartDemoMode (ELF decompile tutorial/elf3/StartDemoMode.c); exe 0x4fe3c0 | debug/demo | like 1 for InitNewGame (no stars/fleets); caller does `StartStory(debug id)` |
| 5 | `LoadSave` (`InitNewGame(0,5,0,0)` before reading the slot) | before loading a save | no stars, no fleets, no story start (INIT_MI and all NEWGAME_* still run); the single-human assertion is skipped; step 50 (Nehezseg==1 extras) still runs if the difficulty is 1 |

Derived flags: `genStars = mode in {2,3}`; `fleets = mode in {0,2,3}` (exe `local_38`).
`CampaignType`: `ct = campaign; if ct<1||ct>3 {ct=1}; if mode!=0 {ct=0}` (exe 0x928278). `Nehezseg` (0x92827C): `if difficulty==0 {difficulty=atoi(GetParam("Difficulty","2"))}; if difficulty>0 {Nehezseg=difficulty}; if Nehezseg<1||Nehezseg>4 {Nehezseg=2}`
so the previous `Nehezseg` survives when difficulty<=0 after the config returns 0 or negative. Callers: CampaignGameStart maps menu level 0->1, 2->2, 1->4 (else keeps old Nehezseg); skirmish passes 1..4 where 3 -> 4 (SetScirmishGameOptions stores `Nehezseg = (d==3)?4:d` before the call).

## 1. InitNewGame step by step (exe order = ELF order unless noted; Poll(1) = SoundEngine::Poll, no sim state)

Notation: E[x] = exe VA array indexed by empire e (1..18), P[p] = planet record.

| # | step | writes (exe VA) |
|---|---|---|
| 0 | free MainScenarioDescriptor; clear scenario info | 0xBBD140 = free,0; scenario_registry_name[0] 0xBAD0E8 = 0; scenario_kezdesi_ido 0xBAD108 = 0; NemSzabadEpiteni 0x996474 = 0; KepernyokLetiltva[0..20) 0xF66F2C = 0 (bytes) |
| 1 | mode fix-up (only mode==0 and campaign==0): ct = atoi(ConfigFile.GetParam("campaign","0")) (exe FUN_0053e4c0: case-insensitive lookup in the parsed config, default string). If ct==0: mode=3; HanyGepiJatekos (0xF21488)=7; ScrirmishPopulation (0xBAD0E0)=0; ScrirmishTechicalLevel (0xBAD0D8)=0; ScrirmishPlanetNumber (0xBAD0DC)=0x50; ScrirmishStartingMoney (0xBAD0E4)=50000 | |
| 2 | CampaignType / Nehezseg (section 0) | 0x928278, 0x92827C |
| 3 | Desync (0xB771E8)=0; scenario_nevbevitelkell (0xBAD10C)=0; BattlesWon[0..18) (0xB79FB8, i32) = 0; VoltHarc (0x99646A)=0 | |
| 4 | InitializeStarLog (exe 0x4eb1c0, ELF 0x55e540) | StarLog (0xB82990, 200 entries stride 0x2c): +0 (i32)=0, +8 (char text)=""; StarlogHanyszoveg 0xB84BF0=0; StarlogHanysorfeltoltve 0xB84BF4=0 |
| 5 | InitializeInfoCharts (exe 0x4eb130, ELF 0x55e740) | InfoCharts (0xB7A000, 0x44C0 B = 4400 f32) := -1.0f (InfoChartsSmoothed 0xB7E4C0 is NOT touched); InfoOldNap 0xB84C1C=-1; InfoOldHonap 0xB84C78=-1; InfoOldNapCounter 0xB84C70=0; InfoOldNapCounter2 0xB84C74=0; InfoTriX 0xB84C00=0x135; InfoTriY 0xB84C04 = (int)(((s+50)-s)+(s*2)/5) with s=(f32)sqrt(2700.0) = 70 (exe: sqrt in f64 then f32 sums; ELF sqrtf) |
| 6 | mode==0 only | EN (0x912A3C) = {1,3,5}[CampaignType-1]; PlayerKi[1] (0xBAE561)=2; PlayerKi[EN] (0xBAE560+EN)=1; HangLejatszasKell (0xBAE26C)=CampaignType |
| 7 | RandomInit(seed) (exe 0x540960, ELF 0x6220e0) | srand(seed) (MSVC LCG h=h*214013+2531011, value (h>>16)&0x7fff); RandSeed[0..21) (0xBAF2D0, u32) = 21 consecutive rand() values (masked 0x80007fff = plain 15-bit). The libc LCG is left after 21 draws. ELF build differs (random(-0x80000000)) |
| 8 | texts | Szovegek[0..8) := "" ; SzovegSzin[k] (0xF16EB8+4k)=0 |
| 9 | ground-war arrays | WF::Free on all 200 Warfare records (exe base 0x9C3B30 stride 0x8EC; ELF stride 0xA20); for i=0..199 (value i+1): WF[i]+0x14 (exe; ELF +0x20) = i+1; WF[i]+0x18c (ELF +0x278) = i+1; WF[i]+0x1d0 (ELF +0x2e0) = i+1; WF[i]+0x148 (ELF +0x1f8) = -1; WF[i]+0x8d0 (ELF +0xa00) = 0; LandingZones[i] (exe stride 0xC0, word at 0xA350CC+0xC0 i; ELF +0x134 stride 0x138) = i+1 |
| 10 | zero per-planet arrays (planets 1..200) | 200 dwords each at 0xB99CBC (BolygoMennyiErodEro = FORTRESS+4), 0xB99994 (BolygoMennyiFoldiEro = GROUND+4), 0xB99FE4 (IonGun), 0xB9A30C (Tuzoltosag), 0xB9A634 (NeededMoral), 0xBA951C (HanyEpulet); FullGDP (0xBAE060+4e) and FullTax (0xBAE0B0+4e), e=1..18 := 0 |
| 11 | OldReallocate (exe 0x540400: p = realloc(p,(n+2)*size); *cap = n) | Flottak (0xBB40A4): size 0x80, n=100, MAXHANYFLOTTA 0xBB40AC=100 (uninitialised heap); Hajok (0xBB4100): size 6, MAXHANYHAJO 0xBB40FC=100; for p=1..200: Epuletek[p] (0xBA9E90+4p) size 0x20, n=0 -> 0x40 bytes uninit, HanyEpuletBlokk[p] (0xBA9B68+4p)=0; HanyEpuletLefoglalva[p] (0xBA9840+4p)=0 |
| 12 | NEWGAME_Story (exe 0x5c3040, ELF 0x7308a0) | st_vOzonvizMut 0xF265D8=0; memset(st_vOzonviz 0xF265E0,0,0xE10); sHanyBolygo 0xF23A08=0; sHanySzamlalo 0xF23A0C=0; sHanyLogikai 0xF23A10=0; sHanyValtozo 0xF23A14=0; 0xF26528=0 (sHanyDuma); memset(StoryAdat 0xF273F8,0,0x128); StoryDesignElsoTerv 0xF419D4=10000; StoryDesignDataMut 0xF419D0=0; StoryDesignColorDataMut 0xF419DC=0; StoryFlottaDataMut 0xF2F870=0; StoryBuildingDataMut 0xF23A00=0; StoryValueDataMut 0xF26204=0; then ReadFlottaData, ReadValues, ReadBuildingData, ReadDesignData (story data files, ig2_formats::story_data). ig2-sim: story.rs StoryState::new_game |
| 13 | empires loop e = 1..18 | Money[e] (f64, 0xBAD168+8e) = 50000.0; if Nehezseg==2 and PlayerKi[e]==1: 80000.0; else if Nehezseg==1 and PlayerKi[e]==1: 125000.0; if mode==3: Money[e] = (f64)ScrirmishStartingMoney (for ALL empires). BirodalomEl[e] (0xBAE6AC+e): mode 2: set to 1 only if PlayerKi[e]==1 (otherwise left as is); other modes: e <= ProgramPlayers2 (0x912AD0) ? 1 : 0; then mode 3 and PlayerKi[e]!=1 and e>8 -> 0. BirodalomIsmeri[e][1..18] (0xBAEBB8+19e+k, u8)=0. BirodalomMiotaIsmeri[e][k] (0xBAED28+0x4c e+4k, i32), k=1..18: if PlayerKi[k]==1 or Nehezseg!=4 then T[Nehezseg] with T=[_,0x708,0x708,0x12c,0] (index 1..4) else 600. Then BirodalomIsmeri[e][e]=1, [e][10]=1. TradeIncomeLastYear 0xBAE100+4e, TradeIncomeThisYear 0xBAE150, TaxIncomeLastYear 0xBAE1A0, TaxIncomeThisYear 0xBAE1F0 = 0. NOTE the money bonus tests PlayerKi[e]==1, the human: easy gives the human more |
| 14 | mode==3: delete AI empires | n = 7 - HanyGepiJatekos times (exe guard: HanyGepiJatekos != 7 and <= 7; ELF asserts 0 < HanyGepiJatekos < 8): repeat x = rndlog(1,1,8) while BirodalomEl[x]==0 or PlayerKi[x]==1; BirodalomEl[x]=0. Stream 1, 1..8, a variable number of draws |
| 15 | planet count | not mode 2/3: HanyBolygo (0x912AC8) = HanyBolygo2 (0xBB6B58) = 0. Else mode 3: HanyBolygo = ScrirmishPlanetNumber; mode 2 keeps HanyBolygo; HanyBolygo2 = HanyBolygo; Generate_Stars() (section 3); then if mode in {2,3} and BirodalomEl[3]: for p=1..HanyBolygo with owner (+0x20)==3: EpuletNewSearchLocation(p,0x30,true,0,-1,-1), (p,0x32,...), (p,0x34,...). The 3 is literal (ELF BirodalomEl._3_1_, exe 0xBAE6AF): only empire 3 |
| 16 | switches | BirodalomEl[9] = BirodalomEl[10] = 1 (exe _DAT_00BAE6B5 = 0x101); DiplomaciaEngedelyezve 0xBAE3AD = TradeEngedelyezve 0xBAE3AE = SpyEngedelyezve 0xBAE3AF = 1 |
| 17 | MoneyInit(true,0) (exe 0x540060, ELF 0x622530) | section 2 |
| 18 | calendar | Ev 0xBAE040 = 0x917 (2327); Honap 0xBAE044 = 3; Nap 0xBAE048 = 12; OldNap 0xBAE058 = 12; Ido 0xBAE04C = DatumToIdo(2327,3,12,0) (exe 0x53c2b0); Ido2 0xBAE050 = 0; IdoMultiPlayerBattle 0xBAE054 = 0; Counter 0xBAE05C = 0; TutorialCounter 0xF6919C = 0; Multiplayer_speed 0xBAE0AC = 1; SpWar_MultiCsata_folyamatban 0xBAD114 = 0; GrWar_MultiCsata_folyamatban 0xBAD13C = 0 |
| 19 | spies | OsszHanyKem[1..8] (0xBB1F84+4i) = HanyKemLehet (0xBB2388+4i) = HanyKemLehetOsszesen (0xBB23AC) = HanyKemVan (0xBB23D0) = 0; HanyKemKozpontBolygonkent[i][1..200] (0xBB2418+0x324 i+4p) = 0. For i=1..8, k=0..12: Kemek[i][k] (0xBAFCC8+0x3DC i+0x4C k) fields +0x10 (u16), +0x16 (u16), +0x18 (u8), +0x1a (u16), +0x38 (i32) := 0. For i=1..8 with PlayerFaj[i] != 5: memcpy(Kemek[i], Kemek0, 0x3DC) where Kemek0 = 0xBB1FA8 (lies in .data's BSS tail: zero in the image, no code writes it that I found), then Spy_New(i) once (ELF loop for j=1; j<2). KemKuldetesek[14..26] byte +0 (0x912F48+0x20 i) = 0 |
| 20 | counts | OsszHanyWaypoint 0xBB60F0 = 0; OsszHanyFlotta 0xBB40A8 = 0; OsszHanyHajo 0xBB4104 = 0; OsszHanyTank 0xBB4154 = 0; OsszHanyTervLefoglalva[e] (0xB9C078+4e) = 0; OsszHanyTerv[e] (0xB9C028+4e) = 0x2E (46) |
| 21 | TervekReallocate (exe 0x542790, ELF 0x619cf0) | for e=1..18 where Lefoglalva==0 or Lefoglalva<OsszHanyTerv or OsszHanyTerv+10<Lefoglalva (always true here): if e==EN also realloc 3 UI int arrays (0xF1738C, 0xF17390, 0xA3F0AC) to (n*4+0x30) bytes; Tervek[e] (0xB9AA18+4e) = realloc((OsszHanyTerv+12)*0x84) (uninit), Lefoglalva = OsszHanyTerv+10 = 56. ELF: OldReallocate size 0x84, n=OsszHanyTerv+10 |
| 22 | tech table | memcpy(Talalmanyok 0xB9C0C8, Talalmanyok0 0x8DE048, 0xD014) (193 x 0x114); for t=1..192, e=1..ProgramPlayers2: status[t][e] (0xB9C10D+0x114 t+e) = Talalmanyok0 status[t][PlayerFaj[e]] (0x8DE08D+0x114 t+race). Then TalalmanyEroHP[t] (0xBA9210+4t, t=1..192) = (int)sqrt((f64)((f32)TalalmanyokrEro[t] (0x8EB730+4t) * (f32)Talalmanyok[t].+0x20)) (f32 product, f64 sqrt, truncation toward zero) |
| 23 | designs | for e=1..18, i=0..0x28: memcpy(Tervek[e][i] (+0x84 i), Tervek0 0x90BF80+0x84 i, 0x84); Tervek[e][i].+0x40 (u16) = PlayerFaj[e]; if i>0: TervAutoDesign(e,i,0) then TervUjraSzamol(e,i,false,true). Then e=1..18, i=0x29..OsszHanyTerv[e] (=0x2e): copy template + race word only |
| 24 | InitAdvancedTalalmanyok(mode) (exe 0x4e0f70, ELF 0x581c00) | section 2 |
| 25 | mode==3 | InitScirmishBolygok() (exe 0x4e2a10, ELF 0x581f60), section 3 |
| 26 | for e=1..18, i=0x29..OsszHanyTerv[e] | TervAutoDesign(e,i,0); TervUjraSzamol(e,i,false,true) |
| 27 | TradeHanyTradeCuccVan 0xF439CC = 0 | (ELF loop over it never runs; exe dropped it) |
| 28 | per-empire | OsszHanyKolonizalo[e] 0xBB4108+4e, OsszHanyTerraformalo[e] 0xBB4158+4e, OsszHanySajatFlotta[e] 0xBB40B0+4e = 0; KiMelyikBolygonVan[e] (0x9ABAE8+4e) = -1; LaborSzint[e] (0x912728+0x10 e) +4,+8,+0xc = 0; LaborSzintOsszes[e] (0xBA90E0+0x10 e) +4,+8,+0xc = 0 |
| 29 | planets p=1..HanyBolygo2 (EMPTY LOOP for modes 0,1,4,5: HanyBolygo2 is 0 here) | P[p]+0xda+2k (u16, e=k+1, k=0..17: visibility +0xd8+2e) = 0; P[p]+0xc4+e (u8, e=1..18) = 0; +0x114 = 0; +0x11c..+0x164 (19 dwords) = 0; +0x174 = +0x178 = 0; +0x17c (u32) = (r1<<16)|(r2<<8)|r3 with r = rndlog(1,0x80,0xff) three times in order r1,r2,r3; +0x180 = rndlog(1,0,99); +0x181 = rndlog(1,5,9); +0x182 = rndlog(1,8,15); +0x183 = rndlog(1,0,99); +0x184 = rndlog(1,20,99); +0x185 = rndlog(1,50,99) (all stream 1, 9 draws per planet in this order); BolygoUtolsoKozeliFlotta[p] (0xB99340+4p) = 0; BolygoMennyiIdejeLazad[p] (0xB99668+4p) = 0. NOTE this runs AFTER Generate_Planet etc. and zeroes the visibility Generate_Planet0 set (+0xd8+2*owner = 1000) |
| 30 | GlobalisMoralCSokkentoBarkacs 0xBAEAC4 = 0; GlobalisMoralCSokkentoBarkacsSebesseg 0xBAEB14 = 0; ResearchAktFejlesztett[e] (0xB9A9B8+4e) = 0 | |
| 31 | starmap view | StarmapMode 0xB9998C = 1; StarmapTransferEgyesMode 0xB9A306 = 0; AktPlanet 0xB9933C = 1 then the LAST p<=HanyBolygo with owner==EN; AktFleet 0xB99664 = 1; StarmapPlanetKering 0xB9A97C = 0; StarmapVoltInit 0xB99339 = 0 (Generate_Stars had set it to 1); OsszHanyWormhole 0xBB61A0 = 0; WormHoles list emptied (head 0xF18B68: each element destroyed through its vtable slot 0 with arg 1), StarmapWormHoleParticles freed (exe 0x4a87a0: delete each pointer, free array, count=0) |
| 32 | radar | for e=1..18, k=0..7: RadarHatotav2[e][k] (0xBB61A8+0x20 e+4k) = RadarHatotav2Init[k] (0x9138B0, f32); RadarHatotav2Negyzet (0xBB6408+0x20 e+4k) = f32 square of it; RadarHatotavSzorzo[e] (0xBB6668+4e) = 1.0f. ig2-sim radar.rs RadarTables::new_game |
| 33 | UI defaults | AktEpulet 0xB9C074 = 0; AktEpuletTipus 0xB9C0C4 = 1; ResearchTerulet 0xB9A9B4 = 1; ResearchMezo 0xB9AA04 = 5; ResearchTalalmany 0xB9AA08 = 5; KutatasSurgos[e] (0xB9C014+e, u8) = 0; AktTervMutato 0xB9AAEC = 1; AktTerv 0xB9AA64 = 3; DesignTervSorszam 0xB9AA10 = 1 |
| 34 | NEWGAME_Design (exe 0x4b6060, ELF 0x508210) | desKINYITVA 0xA3EF8C = 0; for the 55 PodTipusok (0x8F8918, stride 0x54) with +0 > 0: +4 (14 chars) = localized GetString(table 0x13, id if id<0x33 else (2, id-0x32)) - text written into the exe's static table |
| 35 | NEWGAME_Colony (exe 0x48bf80, ELF 0x4d8e40) | reads text/maffia.txt, 64 lines, each cut at the first double quote -> MaffiaNev[64] (0xA3EA40..0xA3EB40, CString); Colony_elozo_foldrenges 0x9ABAE4 = 0; Colony_foldrenges 0x9ABB70 = 0; Colony_foldrenges_ido 0x9ABB74 = 0 |
| 36 | AktKem 0xBB2384 = 1; KemHanyBeszolas 0xBAF334 = 0; CsaladtagElrabolvaKi[e] (0xBB405C+4e, e=1..8) = 0 | |
| 37 | NEWGAME_Messages (exe 0x508f30, ELF 0x5a8ff0) | 0xDABDD4 (AktUzenetPointer) = 0; 0xD90DC8 = -1; 0xDABDB8 = -1; 0xDABDB4 = -1; 0xD909BA (u8) = 1; 0xDABDD8 = -1; 0xDABDA0, 0xDABDA4, 0xDABDA8, 0xDABDAC, 0xDABDB0 = -1; 0xDA65E7 (u8) = 1; 0xDABDC0 = -1; 0xD909AE, 0xD909AF (u8), 0xD909A8 = 0; LoadMessagesFile() (reads the message-kinds archive resource 0xd5189e: HanyUzenetFajta 0xDABE38 and the 1000-entry UzenetFajtak table); ReadUzenetAnimLista() (exe 0x509280); mesKINYITVA 0xDA65E5 = 0 |
| 38 | NEWGAME_Diplomacy (exe 0x4c80c0, ELF 0x529e20) | Diplomacy_ReadSpecials() (exe 0x4bfce0; SpecialLista 0xA42DE8, SpecialListaMut 0xA41DC4; parser in ig2-formats); MaxPlayerek 0xAB64DC = 0; InitBirodalom(i) i=0..19 (exe 0x4c6560, ELF 0x51c9d0): AktPlayerek (0xA41E98 + 0xC4 i) +0 = 0, +8 = 0, +0xc (u8) = 0, +0x10 = 0, +0x14 (u8) = 1, +0x16 (u16) = 0, +0x1c/+0x20/+0x24 = -1 (the exe loop covers i = 0..19 inline); DiplomacyStarted 0xAB69B5 = 1; then srand(SDL_GetTicks()) and, for e=1..18, FIVE libc rand() draws: BirodalmiMacskak[e] (0xA41CD8+4e) = rand()%10; BirodalmiPapagajok[e] (0xA41D28) = rand()%10; BirodalmiEvfordulok[e] (0xA41D78) = rand()%12+1; BirodalmiGyengesegek[e] (0xA41DC8) = rand()%10; HanyevesAzUralkodo[e] (0xA41E48) = rand()%10+30; VoltmarUdvozloUzenetEbbenAzEvben[e] (0xA41CC4+e, u8) = 0; DipOldNap 0xAB6FFC = -1; dipKINYITVA 0xAB6504 = 0. NOT deterministic (wall clock) and the five arrays are SAVED; Game.libc ends in the state after 90 draws from that seed |
| 39 | NEWGAME_Research (ELF 0x5e8c40) vs exe | ELF: Terulet_Akt[0..13) (0x919200) = Terulet_Akt_Def; ResMelyikGombot/2/Old = -1; ResGombNew/2 = 0; ResearchTerulet = 3; ResearchOldForgato (0x919598) = 3; ResearchAktTeruletSorszam = Terulet_Melyik[3]; ResearchTalalmany = 0x21; ProjectAllapotok[i] = status of EN for tech i+1; InfoSorok text rows; Double_Lenyomva = 0; resKINYITVA = 0; then LoadTalalmanyInfok(). EXE: only LoadTalalmanyInfok() (exe 0x51f050, ELF 0x5e7df0: reads the tech info text resource id 0xd55539 into ResTalalmanyInfok rows of 0x21c bytes, 189 entries). The exe image already holds Terulet_Akt (0x919200) = [0,1,1,1,1,1,1,1,1,1,1,1,1] and ResearchOldForgato (0x919598) = 3 in .data, so a fresh process has what the ELF writes (the exe does NOT reset them for a second game in the same process) |
| 40 | Research_TalalmanyUjraSzamol (exe 0x5245c0, ELF 0x5eb450) | ported in research.rs (status 1->2->3 from prerequisites +0x2d/+0x2e/+0x2f, DebugAllTechs 0xB77254) |
| 41 | NEWGAME_Trade (exe 0x6074b0, ELF 0x7acc20) | TozsdeScrollLoad (exe 0x607fe0, ELF 0x7a7420): ScrollQueueLength 0xF439C8 = 0; ScrollSorok +0x50 (0xF43528) = 0; ScrollSorok text (0xF434D8) = GetString(8,4); OldEtaHonap 0x928DB0 = -1; TradeOldNap 0xF66E2C = -1; TradeHanySzallitas[e] (0xF66DE0+4e, e=1..18) = 0; traKINYITVA 0xF43433 = 0 |
| 42 | UzenetekDb 0xE413D4 = 0; UzenetekDbHistory 0xE413D8 = 0; UzenetSzoveg[0] 0xE41230 = 0; HibaUzenetSzoveg[0] 0xE41308 = 0; UzenetekSzama 0xDABDD0 = 0; StoryTerroristaCselValoszinusege 0xF23A18 = 0x32 | overwritten by Story_Generate (step 43) |
| 43 | InfoString_Read (exe 0x53ec80, ELF 0x620a30): only if InfoStringMut == 0 (once per process; DAT 0xBAD0D0): reads infostr.txt into InfoString[1000] (0x18 B each exe/ELF). Story_Generate (exe 0x5c8d30, ELF 0x73f0a0) | StoryEsemeny[0..17) (0xF2F8AC, u8) = 0; StoryInterregnumIdo 0xF22738 = 0; StoryTerroristaCselValoszinusege 0xF23A18 = 0x1e (30) FINAL |
| 44 | InitStory(mode) (ported, campaign.rs) | mode 0 loads the campaign map here: LoadScenario("scenario/<n>.scn", 1) (section 6) |
| 45 | LoadGameInfo (exe 0x53f2a0, ELF 0x6218b0); ClearGameInfo (exe 0x53c180, ELF 0x621b80) | GameInfoList (0xE3E398; 100 entries x 8 B exe) allocated if null, filled from gameinfo.txt (lines "<id> <text>", GameInfoListMut 0xE3E39C = count); GameInfo (0xE3E3A0) freed and re-allocated with 100 default entries (12 B each exe); GameInfoMut 0xE3E3A4 = 0; GameInfoAlloc 0xE3E3A8 = 100 |
| 46 | if fleets (mode in {0,2,3}) | for e=1..8 with BirodalomEl[e]: h = st_bSearch(-e, 2) (random planet of e with visibility >= 0 for EN: rndlog(2,1,count), STREAM 2); name = PlayerKi[e]==1 ? "Conqueror" : AlapFlottaNev; AktFleet = FlottaNew(name, e, 0, P[h].x, P[h].y - 15.0f, 0.0); HajoNew(AktFleet,0x2b) twice; a third HajoNew(0x2b) unless (MultiPlayerMode and e==5); if !MP and Nehezseg==1 and human: HajoNew(0x2b), (0x2c), (0x2d); if !MP and Nehezseg==2 and human: HajoNew(0x2b), (0x2c); TankNew(AktFleet,0x2a,8); TankNew(AktFleet,0x29,2); if e==EN: AktPlanet = h. Then for e=1..8: r1 = rndlog(1,-250,250); r2 = rndlog(1,-250,250); AktFleet = FlottaNew(TraderNev[e-1], 9, 10000+e, (f32)r1, (f32)r2, 5.0); fleet flags: ELF (u16 at +0x52) &= 0xf7ff, exe (u8 +0x53) &= 0xf7; HajoNew(AktFleet, 8). TraderNev (exe pointer table 0x90B638, 8 names)|
| 47 | INIT_MI (ported) | Poll(1) before and after |
| 48 | voltmargeneralva 0xF16F19 = 0; ColonyBuildingPlace 0x99643A = 0; mode 0: StartStory(DebugStory (0xB77260) != 0 ? that : 400, false) | |
| 49 | SpwC::EndSpaceWar (exe 0x566c30, ELF 0x68c230) on the 10 instances (exe base 0xE417C0 stride 0x2D9C; ELF stride 0x3450) | if the instance's active byte (+0x1f2e exe, +0x24ba ELF) is set: frees its buffers and clears +0x1f2e/+0x1f2c; else nothing. Fresh process: all no-ops. Then spacewarbanvanbarkacs 0xF176EE = 0; infoKINYITVA 0xB79F89 = 0; hlpKINYITVA 0xB49E7B = 0; spyKINYITVA 0xE75436 = 0 |
| 50 | if mode not in {1,2} and Nehezseg==1 | h = first planet p <= HanyBolygo with owner==EN (else HanyBolygo+1); n = AddPlanet(EN) (section 3, ALWAYS called, even when h is invalid); if h <= HanyBolygo and n != 0: P[h].+0x24 = 0x96; P[n].+0x24 = 0x7d; EpuletNewSearchLocation(h, t, true, 0, -1, -1) for t in [0xe, 7, 0x34, 4, 6]; EpuletNewSearchLocation(n, t, ...) for t in [0x15, 0xf, 0x10, 4, 6, 7, 3, 0x34, 0x34, 0x34, 10]; P[h].+0x28 = 0x3E2C (15916); P[n].+0x28 = 0x3B24 (15140). Runs in modes 0,3,4,5 (mode 5/4/0 have HanyBolygo from the loaded map; mode 5: 0 planets so h = 1 > 0) |
| 51 | mode 0 | EN==1: AddPlanet(3); AddPlanet(5). EN==3: AddPlanet(1); AddPlanet(5). EN==5: AddPlanet(1); AddPlanet(3). (AddPlanet(e): section 3) |
| 52 | StarmapSetSideView (exe 0x5b0c60, ELF 0x6fde80) section 4; then StarmapXStart 0xB9A968 = TargetXStart 0xF16F1C; StarmapYStart 0xB9A96C = TargetYStart 0xF16F20; StarmapZoom 0xB9A978 = TargetZoom 0xF16F24; StarmapPitch 0xB9A974 = TargetPitch 0xF16F08; StarmapRotate 0xB9A970 = TargetRotate 0xF16F04 | camera (UI) |
| 53 | for i = 0..HanyBolygo2-1 | WF[i] (exe base 0x9C3B30 + 0x8EC i): +0x1a8 = 0xC4016383 (f32 -517.5549); +0x1ac = 0x43F9EBEB (499.8431); +0x1b0 = 0xC4898000 (-1100.0); +0x1b4 = 0xBF490FDB (-pi/4); +0x1b8 = 0; +0x148 = -1 (ELF +0x2a8.. +0x2b8, +0x1f8) |
| 54 | for p = 1..HanyBolygo2 with owner != 0 | P[p].+0x6c = 1; +0x6d = 2; +0x34 = 3 |
| 55 | techs 89, 124, 159 (Talalmanyok offsets +0x6039/+0x85F5/+0xABB1 = tech*0x114+0x45) | for e = 1..ProgramPlayers2: Talalmanyok[89/124/159].status[e] = 0; Talalmanyok[0x9EAA] = 7 (= tech 147 byte +0x2e, written every iteration); if e <= ProgramPlayers (8): seen matrix cells of those 3 techs (+0xc0+9e+o, o=1..8) := 0 |
| 56 | !MultiPlayerMode and Nehezseg==1 | Talalmanyok[tech 83].status[EN] (0xB9C0C8+0x59C1+EN) = 5 |
| 57 | StoryAdat+196 (0xF274BC) = rndlog(1,0xa0,0xf0) * 0x32 | 1 draw, stream 1 |
| 58 | tail | KepernyokLetiltva[0..20) = 0; nemlehetidotallitani 0xBAD0D4 = 0; SetIdoSpeed(1,true); SetIdoPause(false,true) (section 5); ELF asserts exactly one PlayerKi==1 unless MP/mode 5; FRISSIT_Viszony() (ported) |

Order check (RNG): stream 1 draws happen in this order for a skirmish: step 14 (delete AI), Generate_Stars (StarCoords, StarNames, StarData), EpuletNewSearchLocation (empire-3 extras), InitScirmishBolygok, planet colors (step 29), conqueror-fleet st_bSearch (stream 2), trader positions (stream 1), AddPlanet / Nehezseg-1 extras, StoryAdat+196.

## 2. MoneyInit, InitAdvancedTalalmanyok

### MoneyInit(init, e0) (exe 0x540060, ELF 0x622530), called as MoneyInit(true, 0)
1. if init, for e in 1..18 where e0==e or e0==0: MoneyTeruletCsuszka[e] (0xBAD920 + 0x30 e) f64[0..5] = {0, 40, 40, 20, 10, 10} (bit patterns 0, 0x4044.., 0x4044.., 0x4034.., 0x4024.., 0x4024..).
2. MoneyTeruletSzorzoCalculate (exe 0x540280, ELF `MoneyTeruletSzorzoCalculate`; ported: game/info_orders.rs, ai/mod.rs): MoneyTeruletSzorzo[e][1..5] (0xBADCB0 + 0x30 e + 8k) from the sliders (shares in 1/100 steps: `(f64)(int)(slider*100/(c1+0.0+c2+c3))/100`, then 0.01 fix-ups so the first three sum to 1).
3. for e in 1..18 (same filter): if init: MoneyHatar[e][0] (0xBAD590 + 0x30 e) = 0; for k=1..5: MoneyTeruletenkent[e][k] (0xBAD200 + 0x30 e + 8k) = Money[e] * MoneyTeruletSzorzo[e][k] (f64); MoneyHatar[e][k] = 0.
Check asm: product operand order is irrelevant (commutative f64); the exe keeps Money[e] in a register (0xBAD168+8e).

### InitAdvancedTalalmanyok(mode) (exe 0x4e0f70, ELF 0x581c00)
- mode 0 (campaign) only: for e = 1..ProgramPlayers2 with PlayerKi[e] != 1 (the AIs; includes the 3 non-playable-race slots): `TalalmanyKifejlesztve(e, t)` in this order: if Nehezseg>2: t=6, 0x12, 0x16; if Nehezseg>1: 0x99; if Nehezseg>3: 0x92, 0xa2. (Nehezseg is not changed by the calls.)
- mode 3 (skirmish): for e = 1..8 with BirodalomEl[e]: level = ScrirmishTechicalLevel (0xBAD0D8), +1 if PlayerKi[e]==2 and Nehezseg>3; for t = 1..192: if status[t][e] is neither 0 nor 5 and ScirmishTalalmanyFejlesztes[t] (0x8EC218 + 4t, ig2-rules `SKIRMISH_TECH_START`) <= level: TalalmanyKifejlesztve(e, t). Then Research_TalalmanyUjraSzamol.
- all modes: for t = 1..192: for a=1..8: if status[t][a]==5: for b = 1..8: if status[t][b]==5: seen[t][a][b] = seen[t][b][a] = 1, where seen is the byte matrix at record offset +0xc0 + 9a + b.
- TalalmanyKifejlesztve(e, t) (exe 0x524e50, ELF 0x5ebcb0): ported (research.rs).

## 3. Galaxy generation (modes 2/3) and planet helpers

### Generate_Stars (exe 0x59bf40, ELF 0x7081b0)
1. for p = 1..HanyBolygo: P[p] := P[0] (0x62 dwords = 0x188 bytes copied from the template record 0xB85EA8 to 0xB85EA8+0x188 p). P[0] is BSS (all zero in a fresh process; nothing here writes it, but AddPlanet with no candidate would call Generate_Planet0(0,...) and write it).
2. Poll; Generate_StarCoords (exe 0x59b120, ELF 0x705f00); Poll; Generate_StarNames (exe 0x59b860, ELF 0x706430); Poll; Generate_StarData (exe 0x59b520, ELF 0x707cb0); Poll; StarmapVoltInit (0xB99339) = 1 (reset to 0 in step 31).

### Generate_StarCoords (HanyBolygo2 = N; stream 1, starmap.cpp lines 0x148b/0x148c)
- radius R = 100 (N<=30), 160 (N<=50), 230 (N<=70), else 300; half-range H = (R*0x104)/300 (unsigned int division: 86, 138, 199, 260).
- for p = 1..N: repeat { x = rndlog(1,-H,H); y = rndlog(1,-H,H) (x first; stored as f32 in P[p].+0x18 / +0x1c) } until (f32)sqrt((f64)(x*x+y*y)) <= R (f32 square sum; the exe casts the sqrt result to f32 and rejects when (f32)R < that; the ELF compares in f64) AND every earlier planet q<p has (f32)sqrt((P[q].x-x)^2+(P[q].y-y)^2) >= 20.0f (ELF StarDist 0x6e18e0 returns false if any is < 20.0f; the exe inlines it with the same strict < 20.0 and, on failure, goes back to redraw BOTH coordinates).
- Then a nearest-neighbour re-ordering: chain[1] = planet 1; for c = 2..N: from the planet picked last, take the unvisited planet with the smallest (int)((f32)dx*dx + (f32)dy*dy) (strict <, scanning p ascending, best starts at 10,000,000); coordinates are stored TRUNCATED to int ((int)(f32)x) and finally written back as P[i].x = (f32)xs[i], P[i].y = (f32)ys[i] in chain order (only x/y are permuted, planet records stay in place).
- Check asm: exe 0x59b120 sqrt/compare in f32 vs ELF f64; the (int) truncations; the exe auStack_3fc[] indices are shifted by one against the ELF.

### Generate_StarNames (exe 0x59b860, ELF 0x706430) (story.rs only has a StoryWorld hook generate_star_names, not the algorithm)
- K = N/7 (N<51), N/9 (N<101), N/11 (N<151), else N/13, capped at 15.
- constellation names: _s(0x1d, k) for k = 1..15 (localised string table 29); star suffixes are the 20 literals "1".."20" (exe .rdata 0x837BB0 "1", 0x848614 "2", 0x854A44 "3", 0x85D8F8.. "4".."9", 0x837BB4 "10", 0x85D910.. "11".."20").
- seeds: first = rndlog(1,1,N); label[first] = 1; further centres c = 2..K: the unlabelled planet whose min over centres of (int)((f32)dx^2+(f32)dy^2) is largest (strict >, start 0); label it c. cnt[c] = 1 for all K.
- then N-K rounds: for each unlabelled planet p and constellation c: sum of (int)(f32 dx^2+dy^2) to all members (integer accumulation); for constellations with fewer than 20 members score(c,p) = ((min over c2 != c of sum(c2,p), start 10,000,000) / (sum(c,p)+1)) * 100 / cnt[c] (integer divisions); take the max score (strict >, scan p ascending then c ascending, best starts at 0), add p to constellation c.
- names: every planet name (P[p]+0, 0x14 bytes) first "ERROR!!!"; then for each c, members in join order j = 1.. get strncpy(name, constellation name, 0x14); strncat(" "); strncat(suffix[j-1]); then m = rndlog(1,1,cnt[c]) picks a member and st_bAddNewAzonosito(member m, 499+c) (adds story planet id 499+c to sBolygok unless present); finally st_vSet(500, K).
- RNG: 1 + K draws on stream 1 in total.

### Generate_StarData (exe 0x59b520, ELF 0x707cb0)
1. for p = 1..HanyBolygo2: StarData_Init(p, 0) (exe 0x5a7be0, ELF 0x7078d0): +0x22 = rndlog(1,1,8); +0x23 = rndlog(1,1,6) (a value of 60 would become 0x3f, never happens); +0x24 = TermelesSzazalekModosito[rndlog(1,1,5)] (0x90B8E8 + 4k); +0xb8 (u16) = 0; +0xba = 0; +0xbb = 0; +0xc0 = -1; +0xbc = 0; +0x64 = 0; +0x70 = +0x74 = 0; +0x78..+0xb7 = pGyartas0 (0x90B8A8, 64 B); +0xc4+e = 0 (e=1..18); owner +0x20 = 0 and +0xd8+2e = 0 (e=1..18); +0x114 = 0; +0x16a..+0x172 (5 u16) = 0; +0x186 = +0x187 = 0. (3 draws per planet, stream 1)
2. BolygoBirodalom[1..HanyBolygo] (0xF173B8+4p) = 0. BirodalomBolygo[1] (0xF17394+4) = rndlog(1,1,HanyBolygo); BolygoBirodalom[that] = 1.
3. for c = 2..ProgramPlayers (0x912ACC = 8): over p = 1..HanyBolygo with BolygoBirodalom[p]==0: score(p) = min over earlier empires j=1..c-1 of (int)(((f32)dx*dx + dy*dy) * (f32)rndlog(1,10,0x23)) (a NEW rndlog(1,10,35) for every (p,j) pair, p ascending outer, j ascending inner; the product is f32, truncated; min starts at 10,000,000); pick the p with the largest score (strict >, best starts 0; if none exceeds 0 the previous pick stays: the ELF reads an uninitialised local); BirodalomBolygo[c] = pick; BolygoBirodalom[pick] = c.
4. for c = 1..ProgramPlayers with BirodalomEl[c]: m = max(0, FajBolygoViszony[PlayerFaj[c]][1..8]) (0x90B9D8 + 4(k + 9 race)); RndInitValasztas; type = 0; for k=1..8: if FajBolygoViszony[race][k]==m and RndValasztas(1,1) then type = k (stream 1; one rndlog(1,1,total) per tying k, total counting 1,2,..); planet b = BirodalomBolygo[c]: P[b].+0x24 = TermelesSzazalekModosito[3] (= 100, exe 0x90B8F4); P[b].+0x22 = type; P[b].+0x23 = rndlog(1,1,6).
5. SetPlanetSurfaces (exe 0x49c160, ELF 0x4bcc80): lastsurf[0..8) (0x996420, u8, va MAP_CURSOR) = 0; for p = 1..HanyBolygo2: P[p].+0x23 = NextColonyMap(P[p].+0x22 - 1) (colony_side::next_colony_map: next terrain of that kind from lastsurf[kind], wraps; index 60 -> 63; needs the colony maps from the install).
6. InitGrWar (exe 0x482c90, ELF 0x49e6b0): for p in 0..HanyBolygo2: WF[p].+0x18 (exe) = 1 (SetGroupMaxSize(1)); no tanks exist yet so the tank loop is empty; then GroundWarSetBattleStatus (exe 0x47caa0, not read).
7. for c = 1..ProgramPlayers with BirodalomEl[c]: Generate_Planet(BirodalomBolygo[c], owner=c, race=c, true, extra=true, 0, 0).

### Generate_Planet0(p, owner, race, initial) (exe 0x59adf0, ELF 0x7075a0) and Generate_Planet(p, owner, race, a, extra, lo, hi) (exe 0x59af50, ELF 0x707100)
Generate_Planet0: P[p].+0x20 = owner; +0x21 = race; +0x28 (pop) = initial ? rndlog(1,0x22c4,0x238c)+2000 : rndlog(1,0xb54,0xc1c); +0x2c = 0; +0x30 = 500 (morale); +0x34 = 3 (tax); +0x38,+0x3c,+0x40,+0x44 = 0; +0x58,+0x5c = 0; +0x54,+0x55 = 0; +0x68 = 0; +0x6c = 1; +0x6d = 2; +0x70 = +0x74 = 0; +0x78..+0xb7 = pGyartas0; visibility +0xd8 + 2*owner (u16) = 1000. colony_side::colonise implements the initial=false variant (pop 2900..3100); the new-game variant initial=true is used only through Generate_Planet(.., extra=true) in Generate_StarData step 7 (the 4th arg of Generate_Planet0 is the 5th of Generate_Planet).
Generate_Planet: Generate_Planet0(p, owner, race, extra); r = rndlog(1,lo,hi); EpuletNewSearchLocation(p, 1, a, r, -1, -1); if extra: for type in [2, 2, 4, 6, 0x13, 0x34, 0x34, 0xe, 10] in this order: r = rndlog(1,lo,hi); EpuletNewSearchLocation(p, type, a, r, -1, -1). With lo=hi=0 every rndlog is a pure stream-1 advance returning 0.

### AddPlanet(e) (exe 0x5911f0, ELF 0x706fa0)
p = 0; if e == EN: p = st_bSearch(0x2711,0); if p==0 or P[p].owner != 0: p = st_bSearch(0x2712,0); if again: p = st_bSearch(0x2713,0) (story planet ids from sBolygok; no RNG). If still no free planet: h = st_bSearch(-e, 0) (random planet owned by e and visible to EN; rndlog(5,1,count), STREAM 5); p = st_bGetNearest((int)P[h].x, (int)P[h].y, 0) (nearest planet with owner 0 and visibility[EN] >= 0). Then Generate_Planet0(p, e, PlayerFaj[e], false) [pop 2900..3100]; r = rndlog(1,0,0); EpuletNewSearchLocation(p, 1, true, r, -1, -1). Returns p (0 if none: Generate_Planet0(0,..) then writes P[0]).
st_bSearch(id, mode) (exe 0x603d30, ELF 0x72f590): id > 0: linear search of sBolygok (0xF23A20, 8-byte entries: id i32, planet u8 at +4) up to sHanyBolygo (0xF23A08), 0 if absent; id <= 0: count planets with owner == -id and visibility[EN] >= 0 (i16 at +0xd8+2*EN), n = rndlog(mode==2 ? 2 : 5, 1, count) (stream 2 when mode==2, else stream 5; ONE draw even if count == 1; count == 0 gives rndlog(s,1,0)); return the n-th such planet (0 if the index is past HanyBolygo). ELF only: id==0 with mode != 1 calls st_UnknownPlanetReq(1) / throws CStoryWinException when no planet.
st_bGetNearest(x, y, owner) (exe 0x603980, ELF 0x732f50): planets with visibility[EN] >= 0 and (owner == o, or o == -1 and owner != 0xff); metric (int)(((f32)y - P.y)^2 + ((f32)x - P.x)^2) kept truncated; replace only when strictly smaller (the exe compares the f32 sum with (f32)best).

### InitScirmishBolygok (exe 0x4e2a10, ELF 0x581f60) (mode 3)
tbl = [12000, 3500 (0xdac), 8500 (0x2134)], tbl2 = [15000, 23000, 18000], count = 1.
- ScrirmishPopulation (0xBAD0E0) == 1: count = 1, +1 if HanyBolygo > 30, +1 more if > 60.
- == 2: tbl = [24000, 14000, 21000], count = HanyBolygo / 15.
- == 3: tbl = [51000, 14000, 21000], count = HanyBolygo / (HanyGepiJatekos + 3).
- else count stays 1.
for e = 1..8 with BirodalomEl[e]: h = st_bSearch(-e, 2) (stream 2, one draw); P[h].+0x28 = tbl[0] + rndlog(1,0,600) - 300; if status[tech 93][e] != 5 and +0x28 > 32000: +0x28 = rndlog(1,0,600) + 0x7bd4; GenerateCivilBuildings(h); GenerateEnergyBuildings(h); for k = 1..count-1: q = st_bGetNearest((int)P[h].x, (int)P[h].y, 0); Generate_Planet(q, e, e, true, false, 0, 0); pop = (count < 3) ? tbl[count] + rndlog(1,0,600) - 300 : (tbl2[k%3] * rndlog(1,80,120)) / 100; P[q].+0x28 = pop; same tech-93 / 32000 clamp (rndlog(1,0,600)+0x7bd4); GenerateCivilBuildings(q); GenerateEnergyBuildings(q). (tbl[count] with count==2 is tbl[2].) Status of tech 93 = Talalmanyok + 0x6489 + e.

### GenerateCivilBuildings(p) (exe 0x5c20c0, ELF 0x737fb0) and GenerateEnergyBuildings(p) (exe 0x5c23f0, ELF 0x738450)
Civil: V = P[p].+0x28 (pop), o = owner; n[t] = existing buildings of type t (1..14) in slots 1..HanyEpuletLefoglalva[p]. While n[2]*7000 + 5000 + n[3]*12000 < V: t = (status[tech 83][o]==5) ? rndlog(5,2,3) : 2; n[t]++; EpuletNewSearchLocation(p, t, true, 0, -1, -1). While n[4]*9000 + 5000 + n[5]*18000 < V: t = (status[tech 85][o]==5) ? rndlog(5,4,5) : 4; same. While n[6]*0x1d4c + 0x1d4c < V: type 6, n[6]++. While n[7]*10000 + 10000 < V: type 7. If V > 20000 and n[8]==0: type 8 once. If status[EpuletekLeiro[13].tech][o]==5: while n[13]*30000 + 25000 < V: type 13. While n[10]*4000 + 3000 + n[11]*8000 + n[12]*14000 + n[13]*30000 < V: t = 10; RndClass(5): for k=10..13: if status[EpuletekLeiro[k].tech][o]==5 and Next(k-9) then t = k (reservoir on stream 5: rndlog(5,1,cum) <= weight, cum accumulates the weights 1,2,3,4 of the ELIGIBLE k only); EpuletNewSearchLocation(p, t, true, 0, -1, -1); n[t]++. (The exe keeps n[12], n[13] in the locals past the 12-element array; same arithmetic.) EpuletekLeiro: exe 0x8EBA38, stride 0x24 (+0 required tech id, +0xc cost, +0x14 energy).
Energy: E = sum over the planet buildings: type in 0x34..0x37 or == 1: +0x14 * 100; else -(+0xc). While E < 0: t = 0x34; RndClass(5): for k = 0x34..0x37: if status[EpuletekLeiro[k].tech][o]==5 and Next(k-0x33) then t = k; EpuletNewSearchLocation(p, t, true, 0, -1, -1); E += EpuletekLeiro[t].+0x14 * 100.
EpuletNewSearchLocation (exe 0x53cca0): ported (placement.rs) and draws its own RNG (stream 1 for damage args already passed in).

## 4. StarmapSetSideView (exe 0x5b0c60, ELF 0x6fde80): only camera targets (UI, not in the save)
TargetZoom (0xF16F24) = StarmapZoom (0xB9A978); TargetRotate (0xF16F04) = 0; TargetPitch (0xF16F08) = 0xBF733333 (-0.95f). Bounds in f32: maxX = maxY = -300, minX = minY = 300. For p = 1..HanyBolygo with (i16 at P[p]+0xd8+2*EN) > 0 or owner == EN: widen the bounds with P[p].+0x18/+0x1c; count++. If count > 0: TargetXStart (0xF16F1C) = (maxX+minX)*0.5; TargetYStart (0xF16F20) = (minY+maxY)*-0.5; zx = |(|maxX-minX| * 0.5 * 600.0) / 320.0 - 600.0|; zy = |(|(-maxY)-(-minY)| * 0.5 * 600.0) / 240.0 - 600.0|; StarmapMaxKiZoom (0x92798C) = min(zx, zy) / 1.5; TargetZoom = StarmapMaxKiZoom. All f32 (exe SSE, constants single). InitNewGame then copies Target* to Starmap* (step 52).

## 5. SetIdoSpeed / SetIdoPause / SingleStartGame
SetIdoSpeed(s, f) single player (MultiPlayerMode 0xBAE2C0 == 0): returns early if nemlehetidotallitani (0xBAD0D4) != 0 (InitNewGame clears it just before); IdoSpeed (0xBAE258) = s; s==1: IdoSpeedSzorzo (0xBAE25C) = 1, IdoSpeedSzorzoBattle (0xBAE260) = 1; s==2: 4 and 2; s==3: 0x10 and 4. (InitNewGame: SetIdoSpeed(1,true) -> 1/1/1.) Multiplayer: IdoSpeed = ProgramSebesseg (0xBAE54C) (1: x4/b1, 2: x8/b2, 3: x16/b2) and the 4 (= host controlled) case honours f; exe code differs in layout from the ELF but is the same.
SetIdoPause(p, f): single player: IdoPause (0xBAE264) = p (0 here); MP: forced 0 unless ProgramSebesseg==4 and f.
SingleStartGame (exe 0x516390, ELF 0x5c46b0): new_data+4 (u16 0x916034) = EN; MultiGameComposer::Clear (0x91603C = [0x916038]); Multi_ido (0xDB949C) = 10; Multiplayer_speed (0xBAE0AC) = 1; datarecv_ido (0xDB954C) = 0; for i=1..8: datarecv_lasts[i] = 0 (0xDB96E4+4i region) and datarecv_sents[i] = Multi_ido (10) (0xDB9708+4i); datarecv_list[0..100) (0xDB9550..): delete pointee and set 0; ServerBirodalom (0xDB94A0) = 1; SuperVisor (0x912A3A) = 1; MultiPlayerMode (0xBAE2C0) = 0; GameStarted (0xBAE297) = 1; KapcsolatEl (0xBAE2C1) = 1; SocketBlock (0xDB94A4) = 0; MultiPlayer_JoinLock (0xDB94A8) = 0; MultiPlayer_JoinLockUnlocked (0xDB94B0) = 0; MultiPlayer_ClientJoinNo (0xDB94AC) = 0; MultiPlayerJoinIdo (0xDB94B4) = -1; MultiPlayerJoinKi (0xDB94B8) = 0. (Name/VA pairing of the last ten is by position in the two decompiles; check before porting.)

## 6. Callers (all single player)
Common prologue (CampaignGameStart exe 0x4f5e90 / ELF 0x58a8a0, SetCampaignGameOptions 0x503d40 / 0x59bcd0, SetScirmishGameOptions 0x503e80 / 0x59bab0 (8 args), Betolt_Scenario 0x4f5ba0 / 0x59b510, TutorialSetCampaignGameOptions 0x504250 / 0x59c5f0): ProgramMode (0xBAE544) = 0; SingleStartGame(); SetIdoSpeed(1,false); ProgramMachines (0xBAE550) = 1; ProgramPlayers (0x912ACC) = 8; ProgramPlayers2 (0x912AD0) = 0x12; for i = 1..18: PlayerKi[i] (0xBAE560+i) = 2; PlayerFaj[i] (0xBAE574+i) = i; overrides PlayerFaj[15] = 1 (0xBAE583), [16] = 3 (0xBAE584), [17] = 1 (0xBAE585; Betolt_Scenario: 2), [18] = 1 (0xBAE586); PlayerNev[i] (0xBAE588 + 11 i, 11 B strcpy) = Faj_Nev[PlayerFaj[i]] (CString table at 0xBB6A04, localised race names; the copy uses the overridden race).
- CampaignGameStart: only runs when FajValasztasStartGame (0xBBB17C) != 0 and FajVSzetnyilas (0xBBB16C) == 100.0f. ReadDebugData; Nehezseg (0x92827C) = menu level 0 -> 1, 2 -> 2, 1 -> 4 (other values keep the old Nehezseg); seed = DebugRnd (0xB7725C) != 0 ? that : SDL_GetTicks(); InitNewGame(seed, 0, FajValasztasSelectedRace (0xBBB178) + 1, Nehezseg); OldIdo (0xBAE274) = OldIdo2 (0xBAE278) = (SDL_GetTicks()/50)*50; ProgramNowStarting (0xBAD107) = 1; then st_ShowAnim by EN (1 intro_human.bik, 3 intro_politikus.bik, 5 intro_brutal.bik). EN and PlayerKi are set INSIDE InitNewGame.
- SetCampaignGameOptions (debug Enter): EN = 1 (0x912A3C); PlayerKi[1] = 1; ReadDebugData; seed as above; InitNewGame(seed, AktIkon (0xBAE23C) == 0x2a ? 4 : 0, 0, 0) (campaign from config); if DebugStory (0xB77260) != 0: StartStory(it, false); OldIdo/OldIdo2/ProgramNowStarting as above.
- SetScirmishGameOptions(en, humans, randomId, techLevel, planets, money, difficulty, population): EN = en; PlayerKi[en] = 1; ReadDebugData; NewgameRandomID (0xB7740C) = randomId; HanyGepiJatekos (0xF21488) = number of computer opponents (1..7; InitNewGame removes 7 - this many AI empires at random); ScrirmishTechicalLevel (0xBAD0D8) = techLevel; ScrirmishPlanetNumber (0xBAD0DC) = planets; ScrirmishStartingMoney (0xBAD0E4) = money; Nehezseg = (difficulty == 3) ? 4 : difficulty; ScrirmishPopulation (0xBAD0E0) = population; seed = SDL_GetTicks() (the debug rnd override is NOT used); InitNewGame(seed, 3, 0, Nehezseg); OldIdo/OldIdo2 = (SDL_GetTicks()/50)*50; ProgramNowStarting = 1. No StartStory, no FelfedezBirodalom.
- Betolt_Scenario(path): EN = 1; PlayerKi[1] = 1; ReadDebugData; InitNewGame(SDL_GetTicks(), 1, 0, 0); LoadScenario(path, 0) (on failure builds and drops a CString "Scenario load failed:"); for i=1..8: if BirodalomIsmeri[EN][i] (0xBAEBB8 + 19 EN + i): FelfedezBirodalom(i,false); for i = 0..HanyBolygo2-1: WF[i] camera defaults + WF[i]+0x148 = -1 (same as step 53); SetIdoSpeed(1,false); OldIdo = OldIdo2 = (SDL_GetTicks()/50)*50; ProgramNowStarting = 1; then the tail of step 55 (techs 89/124/159 status 0 for all e<=ProgramPlayers2, tech 147 +0x2e = 7, three seen matrices cleared for e,o <= ProgramPlayers).
- FelfedezBirodalom(e, msg) (exe 0x4c1d60, ELF 0x50f090): ported (diplomacy.rs discover_by).
- LoadSave (mode 5): InitNewGame(0, 5, 0, 0) then the slot is read.
- ReadDebugData (exe 0x503980, ELF 0x58ab30): FILE READ of init\init (relative to the cwd): lines key=value; keys (case-insensitive): story -> 0xB77260; rnd -> 0xB7725C; money -> 0x90B4E0; speed -> 0xB77240; ai -> 0xB77244 = (atoi(v) == 0); savescenario -> 0xB77264; measure -> 0xB77268; scenedit -> 0xB7727C. Missing file = nothing. For a deterministic new_game ignore it (defaults 0 in a fresh process; 0x90B4E0 start_money is in .data: read its initial value).

## 7. LoadScenario(path, mode) (exe 0x545b40, 54 KB / ELF 0x62b880, 118 KB; scn.md has the grammar)
The exe decompile hides every keyword body (Ghidra lost the stores); the ELF decompile (needed a 500 MB payload: _local/decomp/newgame/elf/LoadScenario.c, 9862 lines) shows them; exe asm confirms the shape (e.g. the Money store at exe 0x54609a: CVTPS2PD of the parsed f32 then MOVSD to 0xBAD170+8(e-1) for e = 1..18; spy opener at 0x54848e; planet opener at 0x54bae5).
Callers: mode 0 = a full scenario/tutorial (Betolt_Scenario, TutorialLoadScenario call LoadScenario(file, 0)); mode 1 = the campaign map (InitStory: LoadScenario("scenario/<n>.scn", 1)).
Lines: leading whitespace/commas skipped, trailing control chars trimmed, ";" lines match no keyword. A keyword matches by strncasecmp(line, kw, strlen) == 0 and the next char being NUL or whitespace; keywords are tried in the fixed order of scn-keywords.txt. Unknown lines are ignored. Value readers: GetInstantParam (int, consumes one token), GetInstantParamF (f32), GetParam(line, &v, optional) / GetParamF / GetParamShort (return false on a missing token: then LoadErrorString is built and LoadScenario returns false; for pTank/fShip/fTank the count token is optional, default 1).

### Preamble (before the first line)
sHanyValtozo 0xF23A14 = 0; HanyBolygo 0x912AC8 = 0; HanyBolygo2 0xBB6B58 = 0. If mode == 0: OsszHanyKem[1..8] (0xBB1F84) = 0; OsszHanyTerv[1..18] (0xB9C028+4e) = 0x28 (40, NOT 0x2e); Viszony[a][b] for a,b = 1..18 (0xDABF38 + 0x2d2 a + 0x26 b; the exe loop covers 18 rows, bound CMP EDX,0xdaf222 / JLE at 0x545d0a): +0x00 = 0, +0x02 = 5000, +0x04 = 5000, +0x06 = 5000, +0x08 = 0, +0x0a = 0, +0x1c = 5000, +0x1e = 5000, +0x20 = 0, +0x22 = 0x9c4 (2500) (all u16), +0x24 (u8) = 0, +0x25 (u8) = 3.

### Keywords that act in mode 0 only (the whole saved state; every other mode ignores the line)
- EN -> EN; "Money v1..v18" -> Money[e] (f64) = (double)(float)GetInstantParamF; Ev/Honap/Nap/Ido -> calendar (0xBAE040/44/48/4C); ProgramMode/Difficulty/Sebesseg/Players/Players2/Machines/GalaxyShape -> 0xBAE544/548/54C/912ACC/912AD0/BAE550/BAE554; PlayerKi e (1..18 u8) 0xBAE560; PlayerFaj e (1..18 u8) 0xBAE574; "BirodalomIsmeri row v1..v18" -> byte (0xBAEBB8 + 19 row + k) = (v != 0); "RandSeed v1..v20" -> RandSeed[1..20] (0xBAF2D0 + 4k; RandSeed[0] stays RandomInit value); CsaladtagElrabolvaKi / Altal v1..v8 (0xBB405C / 0xBB4080); AktPlanet / AktFleet / StarmapMode (0xB9933C / B99664 / B9998C); StarmapXStart/YStart/Rotate/Pitch/Zoom/PlanetKering (f32); KutatasSurgos v1..18 (u8 = v != 0); ResearchAktFejlesztett v1..18 (i32, 0xB9A9B8); ResearchTerulet/Mezo/Talalmany; "LaborSzint row v1..v3" (0x912728 + 0x10 row + 4k); DesignTervSorszam, AktTervMutato; Nehezseg v (-> 0x92827C); Diplomaciatiltas / tradetiltas (the ELF writes DiplomaciaEngedelyezve = 0 for BOTH; spytiltas -> SpyEngedelyezve = 0: check which byte the exe clears for tradetiltas); ViszonyNemValtozhat -> StoryAdat+176 = 1; tervredesign -> sets the flag that makes the tail recompute every design (local_258 = 1).
- SPY e: OsszHanyKem[e]++, k = OsszHanyKem[e]; memset(Kemek[e][k], 0, 0x4c); the s* lines then write Kemek[e][k] (0xBAFCC8 + 0x3DC e + 0x4C k): sNev name +0 (my_strncpy), sStatusz +0x10 u16, sFaj +0x12 u8, sKep +0x14 u16, sUjonnanJelentkezett +0x16 u16, sKinekKettosKem +0x18 u8, sKuldetes +0x1a u16, sCelFaj +0x1c u8, sCel +0x1e u16, sCel2 +0x20 u16, sIdo +0x24 i32, sIdoHatra +0x28 i32, sIdoElkapasEllenorzes +0x2c i32, sSzetoszthato +0x38 i32, sXP +0x3c i32, sRang +0x40 u8; sTudas k=1..4 -> bytes at record +0x30+k (skills; espionage.md), sIsmerik k=1..8 -> bytes at record +0x41+k (known-by flags, (v != 0)).
- TALALMANY: tech counter t++ (starts at 0, so the first TALALMANY is tech 1); rProjectAllapot v1..v18 -> status[t][e] (record+0x45+e); rKutatasAktIdo v1..18 -> record+0x58+4k (ELF 0x14339b8+4k); rNeededNullazva v1..18 -> record+0xa4+k (ELF 0x1433a04+k, bool; the key-facts name this field the given flag); "rKemkedeshezIsmeri row v1..v8" -> seen matrix record+0xc0+9 row+k. rNeeded, rKutatasSzint, rKutatasMoney, rKutatasIdo are matched in every mode and ignored.
- TERV e: OsszHanyTerv[e]++, i = OsszHanyTerv[e] (the first is 0x29); TervekReallocate(); the record is NOT zeroed (uninit heap; only the t* fields below are written): tNev name +0 (my_strncpy 0x13), tTipus +0x16 u16, tEredetiTulaj +0x40 u16, tLathato +0x18, tNemLehetModositani +0x19, tEltuntetniHaNincs +0x1a, tUpgradeMire +0x1c i32, tGyartasIdo +0x20, tAr +0x24, tMaxTank +0x28 u8, tEro +0x2c, tHP +0x30, tKolonizalo +0x34, tTerraformalo +0x35, tVadasz +0x36, tUrbazis +0x37, tUrbazisMozgathato +0x38, tSatellite +0x39 (bools), tSebesseg +0x3c f32, tAlap +0x44, tAlapTalalmany +0x48, tHanyPod +0x4c, tPodTalalmany k=1..12 +0x50+k u8, tPodDarab k=1..12 +0x5d+k u8, tRadar +0x6a u8, tFleetCloaking +0x6b bool, tKamuDarab +0x6c, tKamuKills +0x70 u8, tKamuBeadasiEv/Honap/Nap +0x74/0x78/0x7c.
- Fleet x, y owner (mode 0): the previous fleet must be empty-or-special, else an error aborts the load; OsszHanyFlotta++ (grow by 100 via OldReallocate when full), OsszHanySajatFlotta[owner]++, the 0x80-byte record is zeroed, +0x15 = owner, +0x30 = x, +0x34 = y (f32), name (+0) = AlapFlottaNev. DUMMYFLEET: OsszHanyFlotta++ and record +0x15 = 0xff only (the rest uninit). The f* lines then write the newest record: fAzon / fAzonosito +0x16 (u16), fNev name +0, fHidden bit0 / fEltunes bit1 of +0x18, fLathato k=1..18 bytes at +0x19+k, fHovaVonulVissza +0x2c u16, fSebesseg +0x38 f32, fAbszolutSebesseg +0x3c f32, fMozgasTipus +0x40 u16, fCelTipus +0x42 u16, fCelX +0x44 f32, fCelY +0x48 f32, flag bits of the u16 at +0x52: fSpecVirus 2, fSpecKolonizalo 3, fSpecTerraformalo 4, fSpecAtiranyitott 5, fKellSzolniFelfedezeskor 6, fVillogas 7, fMindigLathato 8, fBarat 9, fWormhole 10, fNeveLatszik 11, fCloaking 12, fSpecWormholeDeploy 13, fMeteor 14; fRadar +0x7c f32; "fTank type [n]" -> TankNew(fleet, type, n); "fShip type [n]" -> n x HajoNew(fleet, type) (type 0x2b in an owner-9 fleet becomes 8).
- "VISZONY a b" selects [a][b]; vMegbizhatatlan +0x0a, vLebukottKemek +0x00, vRegiHaboruk +0x02, vStoryViszony +0x04, vDiplomViszony +0x06, vDiplomTulSokszorZavartal +0x08, vEroviszony +0x1c, vSzovetsegiRendszer +0x1e, vTavolsag +0x20, vViszony +0x22 (all u16), vTradeAgreement +0x24 (bool), vSzovetsegiAllapot +0x25 (u8).

### Keywords that act in EVERY mode (mode 1 = campaign map load)
BirodalomEro v1..18 (f32 at 0xBAEA78), BirodalomHanyBolygo (i32 0xBAEB18), BirodalomEl (u8 0xBAE6AC + k = v != 0), BirodalomVisszaSzamlalas (i32 0xBAEB68); "vSet id value" -> st_vSet(id, value); "szStart a b" -> st_szStart(a, b); "story id" -> StartStory(id, false); Title / Colony* / rNeeded / rKutatasSzint / rKutatasMoney / rKutatasIdo: consumed, no effect. Planets and buildings:
- planet: HanyBolygo++, HanyBolygo2++, HanyEpulet[p] (0xBA9518+4p) = 0, HanyEpuletLefoglalva[p] (0xBA9840+4p) = 0, memset(P[p], 0, 0x188). "pazon id" -> st_bAddNewAzonosito(p, id); pnev -> strncpy(P[p]+0, text, 0x14); then px +0x18 f32, py +0x1c f32, pTulaj +0x20, pFaj +0x21, pFelszin +0x22, pMap +0x23 (u8), pGazdagsag +0x24 i32, pLakossag +0x28, pLakossagUtolsoValtozas +0x2c, pHangulat +0x30, pAdo +0x34 u8, pAktGDP +0x38, pOldGDP +0x3c, pAktTax +0x40, pOldTax +0x44, pAramTermeles +0x48, pAramIgeny +0x4c, pEmberIgeny +0x50 (i32), pTechLevel +0x54 u8, pEconomyType +0x55 u8, pAutoBuildOsszegyultPrioritas +0x68 i32, pAutoBuild +0x6c u8, pPrioritas +0x6d u8, pRadarRange +0x6e u8, pTermelesKapacitas0/1 +0x70/+0x74 i32, pVirusIdo +0xb8 u16, pVillogas +0xba u8, pKellSzolniFelfedezeskor +0xbb (bool), pBarat +0xbc (bool), pMuholdak k=1..18 u8 at +0xc4+k, pFelderites k=1..18 u16 at +0xd8+2k, pSpBase k=1..10 u16 at +0xfe+2k, pSpBaseHany +0x114 i32, pProblemak k=1..19 i32 at +0x118+4k, pSpecials k=1..5 u16 at +0x168+2k, pMoralCSokkentoBarkacs +0x174, ...Sebesseg +0x178, szin +0x17c i32, bolygomeret +0x180, csillagmeret +0x181, SpinSpeed +0x182, Yaw +0x183, Dist +0x184, Speed +0x185 (u8); "pTank type [n]" -> TankNew(-p, type, n) (a negative id means a planet).
- BUILDING: requires HanyBolygo != 0; if HanyEpuletBlokk[p] <= HanyEpuletLefoglalva[p]: OldReallocate(Epuletek[p], 0x20, &Blokk[p], Blokk[p] + 20); HanyEpuletLefoglalva[p]++; HanyEpulet[p]++; record index = HanyEpulet[p]; the 0x20-byte record is zeroed and +10 = p (u8); then eTipus +0 u8, eTulaj +1 u8, eDamage +4 i32, eEpIdo +8 u16, ex/ey/ez +0xc/+0x10/+0x14 f32, eModosito +0x18 u8, flags at +0x19: eAktiv b0, eRepair b1, eTooDamaged b2, eNincsAram b3, eMaffia b4; eMennyiEnergy +0x1a u16, eMennyiEmber +0x1c u16, eMennyireMukodik +0x1e u16.

### Tail after the last line (every mode; exe 0x552bcc.., ELF lines 836-930)
1. for p = 1..200: planet +0x98..+0x9f = {1,2,3,4,1,2,3,4} (ELF 0x141ee48+4k+j; exe loop from 0xB860C8 stride 0x188).
2. for p = 1..HanyBolygo with owner != 0, for each building slot j = 1..HanyEpuletLefoglalva[p] with type != 0 and EpIdo (+8) == 0: WF[p-1].AddBuildingTurrets(type, 100, owner, j, p); WF[p-1].BuildingBuilt(j) (exe 0x45af60 / 0x45faf0; ported in ground/realtime.rs).
3. for f = 1..OsszHanyFlotta: if bit 10 (fWormhole) of the u16 at +0x52: fleet +0x15 = 0xff (the fleet is deleted).
4. ENGRAFIKA (0xBB6798) = EN; MoneyInit(true, 0).
5. spies (exe 0x552d43..0x552d8d): for e = 1..8: k = 0, 1, ... while k <= OsszHanyKem[k] (BUG kept by both binaries: the bound reads OsszHanyKem indexed by the spy index k, not by the empire e; k = 0 is always processed): Kemek[e][k].+0x12: 0x0b -> 4, 0x05 -> 7.
6. EpuleteketJoHelyreTesz() (exe 0x53cd00, ported: Game::refit_buildings).
7. for e = 1..ProgramPlayers2, t = 1..192: if Talalmanyok0.status[t][e] (0x8DE048 + 0x114 t + 0x45 + e; NOTE indexed by the EMPIRE e, not by its race) == 0: status[t][e] = 0 and, if e < 9, the seen-row +0xc0+9e+o (o = 1..8) = 0.
8. Research_TalalmanyUjraSzamol (0x5245c0); FRISSIT_Planet(false, 0) (exe 0x474930; the display refresh, ported in colony.rs near line 503).
9. if the tervredesign flag is set: for e = 1..8 with BirodalomEl[e], i = 0x29..OsszHanyTerv[e] with design +0x16 >= 0: TervAutoDesign(e,i,0); TervUjraSzamol(e,i,false,true); unless Lang (0x912724) == 1: strncpy(design name, Talalmanyok[design.+0x48].name, 0x13); then FlottakUjraSzamol() (ELF 0x618b40: zeroes the fleet totals +0x4c..+0x78, planet +0x58..+0x60, OsszHanyKolonizalo/Terraformalo, fixes orphan ships/tanks to 0x7fff and recounts them).
10. always: for e = 1..8 with BirodalomEl[e], i = 0x29..OsszHanyTerv[e] with +0x16 >= 0 and Lang != 1: the same strncpy of the tech name into the design name.
Betolt_Scenario then repeats step 55 of section 1 (techs 89/124/159 and tech 147) and the Warfare camera defaults.

## 10. Spots to check in the asm (exe addresses)
| addr | why |
|---|---|
| 0x4e24b0..0x4e2512 (trader FlottaNew) | the two rndlog(1,-250,250) results are stored into the outgoing arg slots by slot reuse: the FIRST draw ends up in the 5th FlottaNew argument and the SECOND draw in the 4th. So exe trader x = second draw, y = first draw (z = 5.0f, the third float slot A). The ELF assigns x = first draw. Verify and keep the exe order |
| 0x4e1a80..0x4e1aaf (TalalmanyEroHP) | MULSS of two CVTDQ2PS ints (f32 product), CVTPS2PD, sqrt (0x819d9c), CVTSD2SS (the sqrt is ROUNDED TO f32), CVTTSS2SI: (int)(f32)sqrt; the ELF truncates the f64. Differs only near integers but must follow the exe |
| 0x59b1b0..0x59b2b3 (Generate_StarCoords) | draw order (x then y), f32 sum of squares, sqrt -> CVTSD2SS -> COMISS against (f32)R, JA = retry; min-distance test COMISS 20.0 > dist (0x8524e0) |
| 0x59b600..0x59b673 (Generate_StarData step 3) | f32 products MULSS of (float)rnd and the f32 sum, CVTTSS2SI, CMOVG (running min); the rndlog is called inside the inner loop (p outer, earlier empire inner) |
| 0x59b7a7 and 0x59b520..0x59b860 | step 4 RndValasztas loop and the BirodalomEl test; P[b].+0x24 = [0x90b8f4] |
| 0x546060..0x5460a9 (LoadScenario Money) | f32 parse -> CVTPS2PD -> MOVSD: (double)(float) |
| 0x552d43..0x552d8d (LoadScenario spy remap) | bound CMP ECX,[ECX*4+0xbb1f84] after the INC: the OsszHanyKem[k] quirk |
| 0x545cb0..0x545d10 (LoadScenario Viszony defaults) | 18 rows, u16 stores in the exact field order; bound CMP EDX,0xdaf222 JLE |
| 0x4eb130 (InfoTriY) | f32 chain ((s+50)-s) + (s*2)/5 with s = (f32)sqrt(2700.0); result 70 either way |
| 0x4e1762 | CMP byte [0xbae6af],0 = BirodalomEl[3] (literal 3) |
| 0x4e2244 and 0x5c8d69 | StoryTerrorista... 0x32 then Story_Generate sets 0x1e |
| 0x4e16be, 0x4e170c | HanyGepiJatekos subtraction loop for the AI removal (retry loop shape) |
| 0x540960..0x5409a0 (RandomInit) | AND 0x80007fff then the negative fix-up: with rand() <= 0x7fff the value is just rand() |
| 0x4c80c0..0x4c814c (NEWGAME_Diplomacy) | rand()%10, %12+1, %10+30 (MSVC rand, SDL_GetTicks seed) |
| Generate_StarNames 0x59b860: divisions by 7/9/11/13 (compiled as magic multiplies) | signed int division of positive values: plain division |
| InitScirmishBolygok 0x4e2a10: local_20[uVar4 % 3] (unsigned modulo), (iVar3 * local_20[..]) / 100 (signed) | |
| 0x4e29a7 (StoryAdat+196) | rndlog(1,0xa0,0xf0)*0x32 stored into 0xF274BC |
| LoadScenario tradetiltas handler | the ELF clears DiplomaciaEngedelyezve for both Diplomaciatiltas and tradetiltas: confirm the exe byte (should be TradeEngedelyezve 0xBAE3AE) |

## 11. Surprises and unknowns
- Non-deterministic inputs: (a) seed = SDL_GetTicks() for scenario/skirmish/tutorial (campaign: debug init\init rnd or ticks); (b) NEWGAME_Diplomacy re-seeds the libc rand with SDL_GetTicks() and fills 5 saved arrays (cats, parrots, anniversaries, weaknesses, ruler ages) and leaves libc in the state after 90 draws; (c) ReadDebugData reads init\init from the cwd (story, rnd, money, speed, ai, savescenario, measure, scenedit); (d) ConfigFile GetParam("campaign") / GetParam("Difficulty") (and InitStory reads storymap/config/registry); (e) OldIdo/OldIdo2 = (SDL_GetTicks()/50)*50 set by every caller after InitNewGame.
- File reads inside InitNewGame: text/maffia.txt, infostr.txt (once per process), gameinfo.txt, the story data files, the message-kinds and tech-info resources, Diplomacy_ReadSpecials, LoadScenario files. Poll(1) = SoundEngine::Poll: only releases finished waves.
- Deliberate-looking oddities kept by the exe: BirodalomEl[3] literal in the skirmish extras; the LoadScenario spy-remap bound; status defaults indexed by empire (not race) in the LoadScenario tail; Money f32 round-trip; tech 147 +0x2e patched to 7 on every empire iteration; StoryTerroristaCselValoszinusege 0x32 overwritten by 0x1e; the easy-mode human bonus tests PlayerKi==1.
- Uninitialised: Flottak/Hajok/Epuletek[p]/Tervek[e] heap beyond what is written; Generate_StarData step 3 pick when no candidate scores > 0 (ELF local_18); AddPlanet may return 0 and then write Bolygok[0]; scenario TERV records and DUMMYFLEET records are not zeroed.
- ELF/exe difference worth a deviation row: exe NEWGAME_Research only loads texts; ELF resets research UI. Also exe InitNewGame has no TradeHanyTradeCuccVan loop and no debug asserts.
- ig2.exe hung (no return in 450 s) on a skirmish with 200 planets (en 8, 4 AIs, tech 2, pop 3, difficulty 1); 150 planets worked. Cause not investigated.
- Unknowns: (1) resolved, §13; (2) contents of Bolygok[0] and Kemek0 at new-game time (both BSS, assumed zero); (3) resolved, §13; (4) GroundWarSetBattleStatus (0x47caa0) effect on a fresh game: for owner==0 planets it writes P.+0x64 = 0, for owned ones FUN_0047be90 decides (never reached in InitGrWar because owners are 0 at that point); (5) FUN_00474930(false, 0) second argument meaning is "all planets"; (6) names/text: GetString tables (PlayerNev = Faj_Nev[race], AlapFlottaNev, constellation names _s(0x1d,k), pod names) are localised and part of saved or visible state: the port needs the user language's tables (Game::fleet_name, empire_names exist); (7) none left for spy offsets (sTudas +0x30+k, sIsmerik +0x41+k match espionage.md).

## 12. Tables and constants read from the exe image (VA, element size, count)
- Talalmanyok0 0x8DE048, 0x114 x 193 (0xD014); TalalmanyokrEro 0x8EB730, 4 x 193; ScirmishTalalmanyFejlesztes 0x8EC218, 4 x 193 (index t = 1..192); EpuletekLeiro 0x8EBA38, 0x24 x 56 (+0 required tech, +0xc cost, +0x14 energy); Tervek0 0x90BF80, 0x84 x 201 (only records 0..0x2e are copied); RadarHatotav2Init 0x9138B0, 4 x 8 (f32); FajBolygoViszony 0x90B9D8, i32 [15][9] (index race*9 + k); TermelesSzazalekModosito 0x90B8E8, i32 x 8 ([3] at 0x90B8F4); pGyartas0 0x90B8A8, 64 bytes (planet +0x78 default production); TraderNev pointer table 0x90B638 (8 x char*); star suffix literals "1".."20" (0x837BB0, 0x848614, 0x854A44, 0x85D8F8..0x85D934, 0x837BB4); PodTipusok 0x8F8918, 0x54 x 55 (name field +4, 14 chars, localised at new game); Terulet_Akt 0x919200 (13 x i32 initial {0,1,...,1}) and ResearchOldForgato 0x919598 (initial 3); OsszHanyTerv scenario constant 0x8597D0 (two i32 = 0x28, 0x28 as a 16-byte splat); 20.0f at 0x8524E0 (star minimum distance); DebugMoney start_money 0x90B4E0 (.data; written by ReadDebugData only); Kemek0 0xBB1FA8 (0x3DC, BSS, zero) and Bolygok[0] 0xB85EA8 (0x188, BSS, zero) are templates in BSS; InfoCharts etc. are BSS.
- Immediates: Money 50000.0 / 80000.0 / 125000.0 (f64); BirodalomMiotaIsmeri {0x708, 0x708, 0x12c, 0} and 600; start date 2327-03-12; DatumToIdo(0x917, 3, 12, 0) (time.rs).
- Localised text needed (not rules): Faj_Nev (race names, PlayerNev), AlapFlottaNev, "Conqueror" (literal), constellation names _s(29, 1..15), pod names GetString(19,..), scroll header GetString(8,4), maffia.txt, infostr.txt, gameinfo.txt, uzenet files.

Note on rows 53/54 of section 1: for mode 0 these run AFTER InitStory (row 44) loaded the campaign map, so HanyBolygo2 is the map size there; rows 29 and 31's loops run before and see 0 planets in modes 0/1/4/5 (row 31 only reads planets for AktPlanet after the map exists, so it works in mode 0).

## 13. Scenario start (mode 1) and the MP generated galaxy (mode 2): ported, 2026-10-23
Port: `Game::start_scenario` (`NewScenario`; `Betolt_Scenario` exe 0x4f5ba0: the §6 prologue with PlayerFaj[17] = 2,
EN = 1, PlayerKi[1] = 1, `InitNewGame(ticks, 1, 0, 0)` with the difficulty from config "Difficulty" (default 2),
`LoadScenario(file, 0)` with every mode-0 keyword, `FelfedezBirodalom` for known empires 1..8, the WF cameras,
`SetIdoSpeed`, the step-55 patches) and `Game::start_mp_galaxy` (`NewMpGalaxy`; `FRISSIT_Start` exe 0x5bc590 →
`InitNewGame(seed, 2, 0, 1)`: MultiPlayerMode 1, ProgramPlayers2 = ProgramPlayers = 8, the lobby's humans, AI
empires, races and planet count, Nehezseg forced 1). The galaxy is §3 (`generate_stars`, `star_coords`, `star_data`
with SetPlanetSurfaces/InitGrWar/Generate_Planet initial, the empire-3 extras), step 29 (planet colours and looks),
step 31's AktPlanet. MP guards: no 3rd destroyer for empire 5, no easy-mode bonus ships, no tech-83 bonus. Step 40
`Research_TalalmanyUjraSzamol` is explicit (before it was only reached via the map tail); `InitAdvancedTalalmanyok`'s
head start is gated to mode 0. `Story::init_story_cleared`; `ScenarioStory::{Start, SzStart}`.

Corrections to §7 / §10 / §11 (normative):
- `tradetiltas` clears `DiplomaciaEngedelyezve` 0xBAE3AD, the same byte as `Diplomaciatiltas`; the trade byte is never
  touched (resolves unknown 3 and the §10 row).
- `tNev` and `fNev` are `my_strncpy(.., 0x15)` (strncpy, cut to n−1 when the name has n bytes or more); `sNev` 0xf.
- TERV records past 0x28 reuse the template bytes; past 0x2e they are heap bytes.
- Fleet's "last fleet empty" check: `OsszHanyFlotta != 0` and prev `+0x15 != 0xFF` and i32 `+0x54 == 0` and
  (u16 `+0x52` & 0x4400) == 0.
- Only the GetParam-family readers fail a load (pazon, pTank type, Fleet x/y/owner, fAzon, fTank/fShip type,
  Nehezseg) with "Error in line N" and no tail.
- `pTank`'s `TankNew` runs inline, before the planet's BUILDING lines: the garrison is placed on an empty map, then the
  battlefield hand-off `AddOwnTankToPlanet` + rearrange.
- Mode 2's callers (unknown 1): the host's `FRISSIT_Start` and the client's `ReceiveStartData` (msgs 3 and 0x1e),
  both `InitNewGame(seed, 2, 0, 1)`; seed = host `rand() % 0x7fff` after `srand(SDL_GetTicks())`; `HanyBolygo` from
  the lobby (20/40/60/80). The lobby: `Start_Init` exe 0x5bfbf0, click handler 0x5c0120, AI pick 0x5bd6c0, status
  msg 2 layout (decompiles in `_local/decomp/newgame-mp/`).

`NewMpGalaxy` also carries shape / battle_status / server_time (0xBAE554 / 558 / 55C); the room is `re/screens/start.md` (ADR 0014).

Seeds: scenario and MP starts use wall-clock seeds that Frida-made calls don't pass through the `srand` hook;
`scenario_seeds.py` recovers them from the post dump (diplomacy ticks from the 90 saved draws; `RandomInit`'s seed =
the latest tick ≤ that whose RandSeed[0] matches).

## 13.1 Skirmish start (mode 3): ported, 2026-10-23
Port: `Game::start_skirmish(&NewSkirmish, &StartData)` = `SetScirmishGameOptions` (exe 0x503e80; 2nd arg = number of computer
opponents, `HanyGepiJatekos` 0xF21488, not humans) then `InitNewGame(ticks, 3, 0, Nehezseg)`: the §6 prologue (PlayerFaj[17] = 1),
EN/PlayerKi, `NewgameRandomID` 0xB7740C, ScrirmishTechicalLevel/PlanetNumber/StartingMoney/Population, `Nehezseg` = d==3 ? 4 : d;
step 13 skirmish money for all empires and AIs > 8 off; step 14 AI removal (`rndlog(1,1,8)` retries); step 15 `HanyBolygo` =
ScrirmishPlanetNumber + `Generate_Stars`; step 24 `InitAdvancedTalalmanyok` mode 3; step 25 `InitScirmishBolygok`;
`GenerateCivilBuildings` / `GenerateEnergyBuildings` (RndClass reservoir on stream 5).

Runtime findings (normative):
- `TalalmanyKifejlesztve` for the human in the head start raises `ResearchEvent::AutoDesign`; `TervezAutomatikus` adds the human's
  automatic designs (`OsszHanyTerv[EN]` grows past 0x2e), and step 26 (`TervAutoDesign` + `TervUjraSzamol` for i = 0x29..=`OsszHanyTerv[e]`)
  refits those too: the loop bound is the live count, not 0x2e.
- The RndClass draw in Civil/EnergyBuildings is the non-logging `rnd` (exe 0x542ea0) on stream 5, not `rndlog` (same values).
- Civil counts only the type byte (+0) of slots 1..`HanyEpuletLefoglalva`, types 1..14, with no destroyed/unfinished test. Energy
  subtracts `EpuletekLeiro[t]`+0xc for every non-power slot (type 0 included) and adds the chosen plant's +0x14*100 per loop whether or
  not it was placed. Tech-id reads of `EpuletekLeiro` are bytes (exe 0x5c22b9).
- The exe's `st_bSearch` has no assert: `InitScirmishBolygok` with no planet would write `Bolygok[0]`.
- Unknown: the 200-planet hang (§11).

## 14. `DesignPoenosanElnevez(e, hull, out, special)` (exe 0x4b19d0, ELF 0x5087c0): ported, 2026-10-23
Exe decompile (`_local/decomp/design/exe/004b19d0_DesignPoenosanElnevez.c`): `n = special + (e·100 + hull)·10`;
the name is the first non-empty of `_s(0x3b, n)`, `_s(0x3b, n − 1)` (only with `special ≠ 0`), `_s(0x3b, hull·10)`
and `Talalmanyok[Alapok[hull].tech].name` (exe 0x90bc04 + hull·0x24, 0xb9c0c8 + t·0x114), built in a 52-byte local
and `strcpy`'d to `out`. `_s` (ELF 0x623f20) returns `CString("")` (ELF 0xd4eeab) for a missing key, so the chain
is live. Table 0x3b = `dataraw/text/s_design.txt` (the GOG install ships no localised copy) (keys `e·1000 + hull·10 + special`, empires 0..8; the
longest name is 16 characters, so a table name never runs past the record's `+0x14`). `e` is the empire, not its
race. Callers: `FRISSIT_MI_DESIGN` (SP only; MP takes the hull tech's name; `ai-empire.md` §10.1) and the story
merchant's generated offers (`trade.md`, `TradeEvent::OfferNamed`). `TervezAutomatikus` doesn't call it (it copies
the tech's name, production-research.md §2.3).
Port: `ai::design::generated_name`; the table is `Game::design_texts` (`StartData::design_texts` for a new game),
read through `AiHooks::design_text` and `Market::design_name`. Verified: `tests/new_game.rs` (35 starts) and the save
replay, both byte-exact without copying names; the mutation above is caught.

## 15. The campaign stories in `Game`'s story tick (done 2026-10-24)
`game::story_chain` runs the whole `Story_Check` chain (Begin, Rnd, BRnd, Fable, Scen, Line, Elem, Multi) at every
story call site (game.rs, game/battles.rs, game/realtime.rs, game/message_orders.rs), not `ScenarioStories` only.
- **Reads**: `game::story_world::GameStory` (a `StoryWorld` over `&mut Game`, replacing `StoryView`) answers every
  read from the save state in the exe's layouts. New helpers: `st_IsInRR` (exe 0x5eb300, `GetENRadarKifejlesztve`
  first), `InfoString_GetIndex` (infostr.txt, the II flag), `CTF_value` (written by the auto-resolved space battle,
  exe 0x5a7430..0x5a7469, [1] and [2]). `GameRules.story` (`StoryRules`) holds the campaign tables, st_Rnd 138's
  recruit names (exe 0x85E0BC), info strings, special.eng ids, `Tervek0` (named from Hungarian txt17 7/k+1 as
  `StringReadAll` does), `pGyartas0` (exe 0x90B8A8) and values/fleet/building.txt (`ig2-rules`:
  `story_recruit_names`, `design_templates`, `production_template`).
- **Creators** made in the game at once: `FlottaNew`, `TervNew` (+`TervUjraSzamol`), `TervCopy2`, `st_GetTerv`
  (`trade::story_design`), `Spy_New`. Fleet names are `FleetName` (story text/bytes/sprintf/design name); ~50 call
  sites that passed "" now pass the exe's `_s(5, N)`.
- **Effects** (`game/story_effects.rs`): every world-reaching `StoryEvent` is carried out when the script raises it:
  fleets, ships, units (the AI's ports); designs (incl. 8's sale, 840's discount, 908's reset); planets
  (`SetBolygoTulaj`, `st_Colonise`/`Generate_Planet`, `st_BolygoDestroy`, `Story_VulkanKitores`, razing,
  `BolygoInfoAdd`/`Remove` (+0xba from the infostr flag; third arg = allow duplicates), `Virus_CheckAdd`, buildings,
  battlefields); techs (`st_SetTalalmanyAllapot`, all states), money, trade prices, relations, `BirodalomBeketKot`,
  `Diplomacy_AddSpecial/RemoveSpecial`, spies (records, `KemJelentesAdd`, story agent 310, 138's recruits, story
  missions); undo types 1, 2, 3, 5, 6, 9, 10, 11.
- **Statics**: the story statics the save leaves out (`save.rs va::STORY_MESSAGE..STORY_BRUTAL4`, `KemEsemeny`,
  `Story_Minden_Flotta_Latszik`) are kept at their exe addresses between checks. Fixes on the way: the `st_vOzonviz`
  chunk was emitted after its count when the loaded save had none; `MULTIGET_Spy`'s per-frame `HanyKemVan` recount
  now runs at the start of `run_tick` (a new campaign's save holds a stale count).
- **Verification**: two campaign-start save replays recorded in ig2.exe from `_local/traces/newgame/newgame-c1-d1-s5150.sav`
  (campaign 1, difficulty 1): `replay-campaign-c1-x16` (60 ticks) and `replay-campaign-opening-c1-x16`
  (`run_replay.py --warmup 685 --ticks 40`: story 401's first opening episode starts at tick 10, pirate base planet 111
  with buildings and garrison, pirate fleet "Khan kalóz" attacking), both byte-exact tick by tick (chunks and the
  compared unsaved globals, incl. the story statics and CTF). All earlier save replays stay byte-exact.
- **Not modelled** (still only forwarded as `GameEvent::Story`; deviations 230-232): the GameInfo lines
  (`st_AddGameInfo`/`RemoveGameInfo`, undo 4; the list at 0xE3E3A0 is saved and passes through as loaded);
  `SpyOrder` (0x49) and `SendStorySpecial` (0x66) lockstep messages (a driver must deliver them); `SaveStoryline`
  (registry); `HoldGroundWar`'s pause and the 0xF261F8 hold; Earthquake/Anim/notices (UI); `JoinBattle` writes
  `spFlottak` (0xBAA1C0) but the battle code doesn't read it back; undo 7 `st_RemoveTerv` (no ported script registers it).
