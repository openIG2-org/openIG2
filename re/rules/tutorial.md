# The tutorial interpreter

Status: **verified at runtime** (2026-10-22, §11, `traces/2026-10-22-tutorial.md`): all five chapters of the shipped
script, 219,000 UI ticks and 970 commands, exact against `ig2_sim::tutorial` (`tests/tutorial_trace.rs`).

Sources: ELF `ig2` `tutorial.cpp` (`TutorialGetNextCommand` 0x7b4bc0, `TutorialFrissit` 0x7bd460, …) and the exe (0x60c570, 0x60c020, …).
The script's lexing (74 keywords by case-insensitive prefix, `.mou` records) is `formats/text.md` §6 (`ig2_formats::tutorial`).
Port: `engine/crates/ig2-sim/src/tutorial.rs` (`ig2_sim::tutorial`).
Exe addresses of globals were matched by position in TutorialStart/Stop/Frissit (ELF and exe sequences agree line for line).

## 0. Summary

* The tutorial is a normal **single-player game** (`TutorialSetCampaignGameOptions` + `InitNewGame` + scenario
  `tutorial/scn/tutorial<N>.scn`) with a **script interpreter beside it**. The script (`tutorial/main/script/main.tut`) drives the UI
  by *replaying recorded mouse/keyboard samples* (`.mou`, one sample per 10 ms UI tick) while live mouse/keyboard are ignored,
  types narration into a 40x8 text window, plays `mfx/tutorial/*.mfx` speech, and sets enable/disable flags other screens read.
* Commands run from `TutorialFrissit`, once per **UI tick (100 Hz)** from `Fake_FRISSITES`; each call runs as many commands as it can
  until a wait blocks. `WAITFOREVENT` switches the tutorial from *Active* (scripted input) to *Paused* (live input, window still
  drawn); `TutorialCheckEvent(id)` (called from ~25 game functions, plus polling in `TutorialCheckEvents`) switches it back.
* Sim-visible effects exist (money, fleets/vehicles/ships, tech state, AI-off debug flag, traders' flags, autobuild, game speed/pause,
  explore flags, a diplomacy message, trade-market destroyers, save loading, spy creation off, a planet rename at event 1). They are
  marked SIM in section 3; everything else is UI.

## 1. State (ELF `ig2` addr / exe `ig2.exe` addr)

| Name | ELF | exe | Type | Start | Stop | Notes |
|---|---|---|---|---|---|---|
| TutorialActive | 0x180f47b | 0xf6915d | u8 | 1 | 0 | scripted input + interpreter run |
| TutorialPaused | 0x180f4a5 | 0xf66f2a | u8 | 0 | 0 | waiting for an event; live input |
| TutorialPuffer | 0x180f480 | 0xf6808c | char* | malloc(len+100) + file | freed (exe also NULLs) | script text, no NUL added |
| TutorialPufferLength | 0x180f488 | 0xf68090 | i32 | file length | | |
| TutorialPufferPtr | 0x180f48c | 0xf68094 | i32 | 0 | | += return of GetNextCommand |
| TutorialTargetTime | 0x180f490 | 0xf68098 | i32 | 0 | | written by TIME, **never read** |
| TutorialTargetCounter | 0x180f49c | 0xf680a0 | i32 | 0 | | COUNT n (absolute, section 2) |
| TutorialStartCounter | 0x180f498 | 0xf6809c | i32 | `Counter` (20 Hz game counter, exe 0xbae05c) | | `TutorialStartMouseSave` sets it to `TutorialCounter` instead |
| TutorialCounter | 0x180f4a0 | 0xf6919c | i32 | (InitNewGame writes) | | +1 per UI tick after TutorialFrissit **unless Paused** |
| TutorialOldCounter | 0x180f494 | 0xf69198 | i32 | -1 | | last TutorialCounter seen by Mouse_Tutorial_Kezel |
| TutorialRestarting | 0x180f4d4 | 0xf680b6 | u8 | 0 | | RESTART |
| TutorialDefaultChapter | 0x180f4b4 | 0xf680ac | i32 | arg 2 of TutorialStart | | menu's `TutorialStartMelyik` (exe 0xbbb230); 0 for `ctut`/demo |
| TutorialSzinVillogtato / Dir | 0x180f564 / 0x119d9a8 | 0xf6813c / 0x928ea9 | i32 / u8 | 0 / 1 | | blink multiplier 30..100 |
| TKActX, TKActY | 0x180e6ac, 0x180e6b0 | 0xf673b8, 0xf673bc | f32 | 398.0 (0x43c70000), 220.0 (0x435c0000) | | text window top-left (640x480 px) |
| TKTargetX, TKTargetY | 0x180e6a4, 0x180e6a8 | 0xf673b0, 0xf673b4 | i32 | 0x18e (398), 0xdc (220) | | AT |
| TKXAdder, TKYAdder | 0x180e6b4, 0x180e6b8 | 0xf673c0, 0xf673c4 | f32 | 0 | | step per move tick |
| TKQueue / TKQueueLen | 0x180e6c0 / 0x180f278 | 0xf673c8 / 0xf67f80 | char[3000] / i32 | - / 0 | | text still to type; TYPE has no bound check |
| TKTombSorPtr | 0x180e6a0 | 0xf6712c | i8 | 0 | | typing column 0..39 on row 7 |
| TKTomb | 0x180e420 | 0xf67130 | 8 x 40 x (char,colour) = 0x280 B | all (' ',1) | | window contents; row 7 = typing row |
| TKTCol / TKTVil | 0x180f27c / 0x180f280 | 0xf67f84 / 0xf6712d | i32 / u8 | (TYPE: 1 / 0) | | colour index / blink flag |
| Tutorial_Szoveg_Counter / _Max / _Novelo | 0x1810698 / 0x181069c / 0x18106a0 | 0xf69144 / 0xf69148 / 0xf6914c | f32 | 0 / 100.0 (0x42c80000) / 15.0 (0x41700000) | | typing accumulator |
| TutorialTextShown | 0x119d9a9 | 0x928ea8 | u8 | 1 (static init 1) | unchanged | key `t` toggles (MainCallBackEventHandler) |
| TutorialDisableTextWindow | 0x180f290 | 0xf6712e | u8 | 0 | unchanged | DISABLE TEXTWINDOW |
| aktct2 | 0x181076c | 0xf67114 | i32 | never reset | | /6 divider of the window move |
| TutorialVisszaSzamlalo | 0x180f4b0 | 0xf680a8 | i32 | **not reset** | | event 0x22 countdown (set to 0x640 = 1600 ticks) |
| TutorialEvent | 0x180f284 | 0xf66f40 | i32 | 0 | unchanged | awaited id (0 = none) |
| TutorialWaitForMouseMoveToEnd | 0x180f4d1 | 0xf6808b | u8 | 0 | | WAITFORMOUSE |
| TutorialWaitForTextToEnd | 0x180f4d2 | 0xf680b4 | u8 | 0 | | WAITFORTEXT |
| TutorialWaitForAtlepes | 0x180f4d3 | 0xf680b5 | u8 | 0 | | WAITFORATLEPES |
| TutorialWaitForScreen / MelyikreWait | 0x180f4aa / 0x180f4ac | 0xf6712f / 0xf680a4 | u8 / i32 | 0 / 0 | | WAITFORSCREEN n |
| TutorialWaitForBattle | 0x180f4a9 | 0xf66f48 | u8 | 0 | 0 | cleared by StartGrWar, SpaceWar_Belep |
| SpeechMiattKilepve | 0x180f4ba | 0xf6808a | u8 | reset each Frissit | | PLAYWAV busy |
| TutorialActMouseAllapot / Max | 0x180f4c8 / 0x180f4cc | 0xf66f20 / 0xf66f24 | i32 | -1 / kept | | .mou sample index / count |
| MouseKeyboardAllapotTomb | 0x180f288 | 0xf66f44 | ptr | | freed, NULL | (count+2) x 0x24 B records (recorder: 1.8 MB) |
| TutorialMouseSave | 0x180f47a | 0xf6915e | u8 | | | recorder (`savetut` console) |
| TutorialKellCounterKirakas | 0x180f4d0 | 0xf66f28 | u8 | 0 | | debug overlay, set only by console `ctut` |
| TutorialLastCommands | 0x180f380 | 0xf67f90 | char[5][0x32] | | | debug history |
| TutorialLastSzovegPuffer / Hossz | 0x180f690 / 0x1810690 | 0xf68140 / 0xf69140 | char[] / i32 | | | last TYPE text |
| Tutorial_LastMp2_Length | 0x180f2fc | 0xf67f88 | i32 | 0 | | byte length of the last PLAYWAV file |
| Tutorial_MainBolygoNeve | 0x180f310 | 0xf66f88 | char[~112] | | empty | SET MAIN BOLYGONEV |
| Tutorial_MainKepernyo_Belepes_A_Tutorialrol | 0x1810695 | 0xf66ffc | u8 | 1 | | **no reader** |
| TutorialHanyadikType | 0x180f300 | 0xf67f8c | i32 | 0 | | only the dead TutorialStartMp2 (ELF 0x7b4b20) reads it |
| TutorialStarmapElindult | 0x1810694 | 0xf66ffa | u8 | 0 | | ShowStarmap sets 1; no reader |
| Tutorial_Split_Ok | 0x18106a4 | 0xf66ffe | u8 | 0 | | set in Starmap_Main while event 5 awaited; read by TransferOff |
| TutorialFelfedezettBolygokSzama / BirtokoltBolygokSzama / TankokSzamaFlottaimban | 0x1810760 / 64 / 68 | inlined (exe ~0x611a30) | i32 | | | baselines of TutorialInitEvents |
| GlobalMusicVolumeCsokkento | 0x16e23b0 | 0xe4166c | i32 | `(int)((double)GlobalMusicVolume*0.4)` | 0 if Stop(true) | music ducking (GlobalMusicVolume exe 0x919928) |
| SelfPlaying | 0x1415d6e | 0xbae267 | u8 | | 0 | demo mode; lets Frissit run on screen 0xd |
| TutorialDebugData_AiDisabled | 0x180f684 | 0xf691c0 | u8 | copy of debug AI flag (ELF 0x144c494 / exe 0xb77244) | restored | |
| TutorialGrNoOverrun / GrSpOverrun / DebugDatagrBattle / DebugData_Battle | 0x180f674 / 78 / 7c / 80 | 0xf691a4 / cc / e4 / a8 | i32 | | restored if KellOverrunVisszaAllitas | saves of ELF 0x144c48c / 488 / 484 / 480 (exe 0xb7723c / 38 / 34 / 30) |
| TutorialKellOverrunVisszaAllitas | 0x180f670 | 0xf680b7 | u8 | 0 | 0 | |
| enable/disable flags | | | | | | section 5 |

Record layouts used by the commands. Fleet (`Flottak`, stride 0x80; exe table pointer 0xbb40a4, count `OsszHanyFlotta` 0xbb40a8):
+0 name, +0x15 owner u8, +0x16 i16, +0x18 flags (bit0 = trader hold), +0x30/+0x34 f32 x/y, +0x52 flags. Vehicle (`Tankok`, stride 8,
count exe 0xbb4154): i16 fleet, i16 type (0x7fff = empty), i32 count. Planet (`Bolygok`, stride 0x188; ELF base 0x141edb0, exe 0xb85ea8):
+0 name, +0x18/+0x1c f32 x/y, +0x20 owner u8, +0x6c autobuild u8, +0xd8 `i16 explored[empire]`, +0x186 u8 (meaning UNCERTAIN).
Tech (`Talalmanyok`, stride 0x114; ELF 0x1433960, exe 0xb9c0c8): state byte for empire e at +0x45+e (5 = researched, 1 = reset;
3 is set by event 9, meaning UNCERTAIN). Screen ids from callers: 13 (0xd) = options/ESC screen, 2 starmap, 3 colony.

## 2. Driver

**Entry** `Fake_FRISSITES` (ELF 0x585160; exe 0x4de720; the call to TutorialFrissit is at exe 0x4de8f7): inside `while (10 < AktIdo - OldIdo2)`
(`OldIdo2 += 10` per pass = **100 Hz**, independent of game speed), in order: `DoKeyboard`, `Mouse_Tutorial_Kezel`, the screens `*2` updates,
`TutorialFrissit`, then `if (!TutorialPaused) TutorialCounter++`.

**TutorialFrissit** (ELF 0x7bd460 / exe 0x60c020). Whole body only if `AktKep != 0xd || SelfPlaying`:
1. Blink: `Szin += 1` while dir=1 until >99 then =100, dir=0; then `-= 1` until <31 then =30, dir=1 (range 30..100).
2. `if (VisszaSzamlalo > 0 && --VisszaSzamlalo == 0) TutorialCheckEvent(0x22)` (runs regardless of Active/Paused).
3. If `Paused || Active`: `++aktct2 > 5` -> `aktct2 = 0` and move the window one step (every 6th tick = 60 ms), per axis in f32:
   `if (Act < Target) { Act += Adder; if (Target < Act) Act = Target }`, then `if (Target < Act) { Act -= Adder; if (Act < Target) Act = Target }`
   (`Target` = `(float)TKTargetX/Y`). `AT` sets `Adder = |Target - Act| / 40.0`, so the window glides in 40 steps (2.4 s).
4. If not Active: `if (Paused) TutorialCheckEvents()`; done (no typing while paused).
5. If Active: `DemoMode_Frissit()` (elf2; exe 0x60b740; effective only when SelfPlaying), `SpeechMiattKilepve = 0`, then the
   **command loop**: while all of these hold, `TutorialPufferPtr += TutorialGetNextCommand(TutorialPufferPtr)`, and after each call
   `if (Restarting) { Restarting = 0; PufferPtr = 0 }`:
   `PufferPtr < Length` and `TutorialTargetCounter <= Counter - TutorialStartCounter` (signed) and `Active` (re-read every pass) and
   the four checks (in this order, short-circuit; **each clears its own flag when it lets the loop pass**, a blocked earlier check leaves
   later flags untouched) and `!TutorialWaitForBattle` and `!SpeechMiattKilepve`:
   * mouse (`MozgatasBefejezesreVarakozasCheck`): pass if `!WaitForMouseMoveToEnd || ActMouseAllapot >= Max || ActMouseAllapot == -1`.
   * atlepes: pass if `!WaitForAtlepes || (!Atlepes && !Belep)` (screen-transition flags, ELF 0x1414b5d / 0x1414b5c, exe 0xbae24d / 0xbae24c).
   * screen: pass if `!WaitForScreen || AktKep == MelyikreWait` (AktKep exe 0xbae0fc).
   * text: pass if `!WaitForTextToEnd || TKQueueLen < 1`.
   * battle: blocks while `WaitForBattle` (not cleared here; `StartGrWar` / `SpaceWar_Belep` clear it).
   * COUNT: absolute since TutorialStart on the 20 Hz game counter `Counter` (it advances only when not `IdoPauseMertAtlepesAlattVan`/`Atlepes`).
   There is **no cap** on commands per call: non-blocking commands (AT, TYPE, PLAYMOUSE, flags, ...) all run in the same tick.
6. Typing (after the loop, still Active): `Szoveg_Counter += Novelo; if (Max < Szoveg_Counter) { Szoveg_Counter -= Max; ONE_STEP }` (f32).
   ONE_STEP (ELF; exe equal; `SlideQueue` = exe 0x60bd70, `TKTombSoremeles` = exe 0x60bda0). CR = byte 0x0d, HASH = 0x23, SP = 0x20:

        while (Q[0]==CR && QLen>0) { SlideQueue(); TKTombSoremeles(); }
        if (QLen>0) {
          w=0; i=0; while (Q[i]!=SP && Q[i]!=CR && i<QLen) { j=i+1; if (Q[j]!=HASH && (Q[i]!=HASH || Q[i+2]!=HASH)) w++; i=j }
          if (0x28 - SorPtr < w) TKTombSoremeles();
          while (Q[0]==SP && SorPtr==0 && QLen>0) SlideQueue();
          while (Q[0]==HASH && QLen>0) {
            if (Q[1]==C) { SlideQueue(); TKTCol=GetInt(Q,0); drop chars until Q[0]==HASH; drop that HASH; }
            if (Q[1]==V) { SlideQueue(); drop until HASH; drop HASH; TKTVil=1; }
            (HASH followed by another letter makes no progress: infinite loop, UNCERTAIN, absent from shipped scripts) }
          if (QLen>0) { TKTomb[7][SorPtr] = (Q[0], TKTCol + (TKTVil&1)*0x40); SlideQueue(); SorPtr++; }
        }
        if (SorPtr==0x28) TKTombSoremeles();

   Comments on the pseudo-code: the first loop treats each CR as a newline (rows scroll up, SorPtr=0); the second block is the word wrap
   (w = printable length of the next word); the markup is HASH C n HASH = colour n, HASH V HASH = blink on until the next TYPE.
   `TKTombSoremeles`: `SorPtr=0`, rows 0..6 <- rows 1..7, row 7 = (SP,1). `SlideQueue`: shift left by one, `QLen--`.
   Speed: no speech -> `Novelo = 15.0` -> 0.15 char/tick = 15 chars/s. After a PLAYWAV the next TYPE sets
   `Novelo = ((float)textLen/(float)Mp2FileBytes) * 50.0 * 160.0` (text spread over the file at 8000 B/s). Colour byte < 0x40 is drawn with
   that colour; >= 0x40 with colour-0x40 and `KiirStrSzinSzorzo = Szin` (blink), reset to 100 afterwards.

**Drawing** (not part of the interpreter, for the client). `TutorialKeretFrissit` (ELF 0x7b4440, called from `GFX_DateMoneyScrollFade` with
`FaderHanyadikMenet=3`; **inlined in the exe** into that function, exact site UNCERTAIN): if `AktKep != 0xd && !DisableTextWindow && TextShown &&
(Active||Paused)`: `FadeKerettelKerekit(x=(int)(TKActX-5.0), y=(int)(TKActY-5.0), w=min(CharXMeret[2]*40+10, 0x27f-x), h=(CharYMeret[2]+3)*8+10,
0, 0x36,0x5a,0x1e, 0,0x5a,0x82, 0, false, 0,0,0,0)`.
`TutorialSzovegFrissit` (ELF 0x7b45a0 / exe 0x612740, called from `Update3D` at exe 0x52d397): nothing if `AktKep==0xd`. If neither Active nor Paused
(or `KilepesElott` exe 0xbae296 / `KilepesElott2` exe 0xf1737e): only the recorder overlay when `TutorialMouseSave` ("Tutorial Counter:" at (0x118,4),
`Counter-TutorialStartCounter` at (0x1a4,4), "RECORDING" at (0x118,0x16) when `AktIdo200`). Else, if `!DisableTextWindow && TextShown`: for r<8, c<40, cell
char != SP: `KellNyelviBugKijelzesTiltas = 1` (exe 0xa3ec54), `KiirStr(cell, (int)(TKActX + (float)(c*CharXMeret[2])), (int)(TKActY + (float)(r*(CharYMeret[2]+3))),
1, 2, 4, colour)`; cursor block `FillToScreen(x of SorPtr, row 7, CharX, CharY, 8, false)` when `((QLen==0 && AktIdo200) || (QLen!=0 && TKTomb[7][SorPtr-1] != SP)) && !Paused`.
(exe: `KiirStr` = 0x4a9e60, `FillToScreen` = 0x4ab250, both unnamed in the symbol table). The KellCounterKirakas debug overlay is never on in the shipped game.

**TutorialGetNextCommand** (ELF 0x7b4bc0 / exe 0x60c570). Returns the byte offset **relative to `param_1`** of the closing `>` (the next call skips it as ordinary text).
Scanning: from `param_1`, skip to the next `<` or `/*` (no `<` before the end -> `TutorialStop(true)`, return 0); for `/*` skip to `*/`, rescan, repeat; `i++` past `<`;
then the 74 `strncasecmp` tests (prefix, no whitespace skipping: "< AT" does not match) in the order of `tutorial::COMMANDS`. **Unknown keyword**: `TutorialStop(true)`; return `i-1`.
All commands below end by skipping to the next `>` unless noted. `GetInt(p, n)` (elf2/GetInt.c): scans from `p` (the byte after `<`, so it includes the keyword) skipping non-digits, skips
`n` digit runs, copies the next digit run to the global `Getintnumber`, `atoi`. Unbounded: with too few numbers it reads past `>` into later text. The exe's `GetInt` (0x60bcc0, inlined `IsNumber`) takes runs of `0-9`, `.` and `-` (`atoi` then stops at the
first non-digit: `-5` → −5, `1.5` → 1) and aborts on a run over 29 characters.
a minus sign is UNCERTAIN (not decompiled; no negative number occurs in the shipped scripts).
Quoted-string reader (FIND*, SHOW PLANET, PLAYMOUSE, SET MAIN BOLYGONEV, WAITFOREVENT, TEXTURE): skip to the first quote, then copy bytes after it up to the next quote
(PLAYMOUSE and SET MAIN BOLYGONEV also step over a CR); in FIND*/SHOW/WAITFOREVENT/TEXTURE a CR inside the quotes **hangs** (the inner loop stops on CR without advancing and the outer loop restarts it).
Buffers are fixed (100..110 bytes), unchecked.

## 3. The 74 commands

Order = ELF/exe test order. Class: SIM = changes simulation state, UI = screens/window/sound only, CTL = control flow, FLAG = sets a gate flag read elsewhere (section 5),
wait = sets a wait flag. Every command first shifts `TutorialLastCommands` (debug only; omitted).

| # | Keyword | Args | Effect | Class |
|---|---|---|---|---|
| 0 | AT | 2 ints x,y | `TKTargetX=x; TKTargetY=y; TKXAdder = abs((float)x - TKActX)/40.0; TKYAdder = abs((float)y - TKActY)/40.0` (f32 diff, abs, **divide in f64**, narrow to f32; exe 0x60c7d2..0x60c856) | UI |
| 1 | END | - | `TutorialStop(true); AddSzoveg("Tutorial END.", 7)` (message line); return | CTL |
| 2 | RETURNTOMAINMENU | - | `TutorialStop(true); AktKepKell = 0xf` (main screen; exe 0xbae250); return | CTL |
| 3 | SEEKCHAPTER | - | `sprintf("Chapter %d", TutorialDefaultChapter)`; from just after the `<` scan forward for the first `strncmp` (case-sensitive, any position, no tag boundary) hit, then skip to `>` (the pointer sits on the `>` of the CHAPTER N tag). Not found: pointer = end of buffer, interpreter stays Active but idle (UNCERTAIN) | CTL |
| 4 | QUIT | - | `TutorialStop(true); MouseIkonKep = 0 (exe 0xbae29c); exit(0)` | CTL |
| 5 | KURVAANYAD | - | `Colony_BELEP_ABBA_A_KIBASZOTT_PLANETINFOBA()`: `ColonyPlanetInfoMod=1; AktEpulet=0; if (ColonyBuildMode) ColonyRemoveBuilding=1; ColonyBuildMode=0` (history string "BAZMEG"; used once in chapter 2) | UI |
| 6 | TYPE | 1 quoted string | skip to the quote; push CR CR onto TKQueue; copy chars to the closing quote: CR -> SP and `i+=2` (eats the LF), LF -> SP in the queue; every char also appended to TutorialLastSzovegPuffer (raw LF kept); `TutorialLastSzovegHossz=len`; `Novelo = (Mp2Len==0) ? 15.0 : ((float)len/(float)Mp2Len)*50.0*160.0` (all f32; exe 0x60cdb2..0x60cdd4); `TKTCol=1; TKTVil=0; Szoveg_Counter=0`. Does not wait: script uses WAITFORTEXT | UI |
| 7 | TIME | 2 ints | `TutorialTargetTime = a*60 + b` - **never read**: no effect | none |
| 8 | PLAYWAV | 1 quoted name | `ExistAnyTutorialMp2()` (needs `KELLBESZED`; a SoundEngine stream whose name contains "tutorial" is playing): then `SpeechMiattKilepve=1; return 0` (not consumed, retried next tick; PLAYWAV doubles as "wait for previous speech"). Else copy name (300 B), `TutorialStartMp2_2`: path `mfx/tutorial/<name>`, `Tutorial_LastMp2_Length` = file length (0 if missing), `GlobalSoundVolume=100; SetGlobalSoundVolume(100); PlayMp2(path); GlobalSoundVolume=0x3c; SetGlobalSoundVolume(0x3c)` (PlayMp2 exe 0x563df0). Script puts PLAYWAV before its TYPE | UI |
| 9 | COUNT | 1 int | `TutorialTargetCounter = n` (absolute vs `Counter - TutorialStartCounter`); not used by main.tut | CTL |
| 10 | GIVE MONEY | 1 int | `MoneyHozzaadLog(EN, 0, (double)n, "../sources_base/tutorial.cpp", 0x532)` (exe 0x53fb10). main.tut: 300000 | **SIM** |
| 11 | DISABLE HAJOELADAS | - | `TutorialDisableHajokEladasa = 1` | FLAG |
| 12 | DISABLE OVERRUN | - | save `GrNoOverrun<-0x144c48c, GrSpOverrun<-0x144c488, DebugDatagrBattle<-0x144c484, DebugData_Battle<-0x144c480`; then `0x144c480=0, 0x144c484=0, 0x144c488=1, 0x144c48c=1; KellOverrunVisszaAllitas=1` (exe 0xb77230=0, 0xb77234=0, 0xb77238=1, 0xb7723c=1). A second execution would save the modified values. Restored by Stop | **SIM** (battle debug options) |
| 13 | DISABLE STARMAP | - | `TutorialStarmapMindenLetiltvaKiveveAKurzorMozgatoBillentyuket = 1` | FLAG |
| 14 | ENABLE STARMAP | - | same flag = 0 | FLAG |
| 15 | DOUBLEGOMB ON | - | `ResearchDoubleGomb()` (exe 0x522020): `if (!ButtonPressed) { PlaySoundGlobal(0); if (!Double_Lenyomva) SetDoubleTexture(); else DoubleGombFelenged(); Double_Lenyomva ^= 1; Multi_Send(0x2b, EN, 0) }` (toggles the research double button, sends message 0x2b) | UI + net |
| 16 | DOUBLEGOMB OFF | - | **no-op** | none |
| 17 | DISABLE COLONY | - | `TutorialColonyMindenLetiltvaKiveveAPageupot = 1` | FLAG |
| 18 | ENABLE COLONY | - | same = 0 | FLAG |
| 19 | DISABLE EXITGOMB | - | `TutorialExitGombDisabled = 1` | FLAG |
| 20 | ENABLE EXITGOMB | - | `= 0` | FLAG |
| 21 | DISABLE AI | - | debug AI-off flag `0x144c494 = 1` (exe 0xb77244). Gates `FRISSIT_MI`, `MI_DIPLOMACY`, `FRISSIT_MI_DESIGN`, `SpaceWarFrissit` | **SIM** |
| 22 | ENABLE AI | - | `0x144c494 = 0` | **SIM** |
| 23 | DISABLE ROMBOLAS | - | `TutorialDisableEpuletLerombolas = 1` (exe 0xf66f49) | FLAG |
| 24 | ENABLE ROMBOLAS | - | `= 0` | FLAG |
| 25 | DISABLE SPLITFLEET | - | `TutorialSplitDisabled = 1` | FLAG |
| 26 | ENABLE SPLITFLEET | - | `= 0` | FLAG |
| 27 | DISABLE PRODUCTION | - | `TutorialProductionDisabled = 1` | FLAG |
| 28 | ENABLE PRODUCTION | - | `= 0` | FLAG |
| 29 | DISABLE SCREEN | 1 int n | `KepernyokLetiltva[n] = 1` (u8[0x14], index unchecked) | FLAG |
| 30 | ENABLE SCREEN | 1 int n | `KepernyokLetiltva[n] = 0` | FLAG |
| 31 | DISABLE ALL SCREENS | - | `KepernyokLetiltva[i] = 1` for i in 0..19 except 13 | FLAG |
| 32 | CENTERFLEET | - | first fleet with owner==EN: `AktFleet=i; StarmapMode=2; KellSelect=1; StarmapKozepreAllit(x,y)` (exe AktFleet 0xb99664, StarmapMode 0xb9998c, KellSelect 0xf1737d, centre fn 0x5af560) | UI |
| 33 | FIND FLEET | quoted name | first fleet with owner==EN and `strcmp(name)==0` (case-sensitive): same select+centre | UI |
| 34 | FIND PLANET | quoted name | first planet with owner==EN and exact name: `AktPlanet=i; StarmapMode=1; KellSelect=1; StarmapKozepreAllit(planet x,y)` | UI |
| 35 | SHOW PLANET | quoted name | first planet (any owner) with that name: `explored[EN] = 1` (planet +0xd8, i16 per empire) | **SIM** (visibility) |
| 36 | CENTERPLANET | - | first planet with owner==EN: select+centre as 34 | UI |
| 37 | CENTERUNCOLONISEDPLANET | - | first planet with `explored[EN]==1 && owner==0`: select+centre | UI |
| 38 | ENABLE INVENTION | 1 int t | tech t state byte for EN (`Talalmanyok + t*0x114 + 0x45 + EN`) = 5 (researched); index unchecked (193 techs) | **SIM** |
| 39 | DISABLE INVENTION | 1 int t | same byte = 1 | **SIM** |
| 40 | ENABLE BUILDING | 1 int b | `TutorialEpuletekLetiltva[b] = 0` (u8[0x3c]) | FLAG |
| 41 | DISABLE BUILDING | 1 int b | `= 1` | FLAG |
| 42 | DISABLE ALL BUILDINGS | - | all entries 0..0x3b = 1 | FLAG |
| 43 | ENABLE ALL BUILDINGS | - | all = 0 | FLAG |
| 44 | DISABLE AUTOBUILD | - | for planets 1..HanyBolygo (**all owners**): planet +0x6c = 0 | **SIM** |
| 45 | DISABLE TRADERS | - | for each fleet with owner==9: `flags(+0x18) = flags OR 1; (+0x16) = 0x29a` | **SIM** |
| 46 | ENABLE TRADERS | - | for owner==9 fleets: `flags(+0x18) = flags AND NOT 1` (+0x16 stays 0x29a) | **SIM** |
| 47 | PLAYMOUSE | quoted file name | reads the name (steps over CR), `TutorialPlayMouse(name)` (section 6) | UI (input) |
| 48 | SET MAIN BOLYGONEV | quoted name | `Tutorial_MainBolygoNeve = name` (unbounded copy); used by event 0x1c | UI |
| 49 | WAITFORMOUSE | - | `WaitForMouseMoveToEnd = 1` (also matches WAITFORMOUSEMOVE, prefix) | wait |
| 50 | WAITFORSCREEN | 1 int n | `WaitForScreen = 1; MelyikreWait = n` | wait |
| 51 | WAITFORBATTLE | - | `WaitForBattle = 1`; for every fleet with owner==EN and `CountMyTanksInFleet(i)==0`: `TankNew(i, 9, 2)` (exe 0x541750; args confirmed in asm 0x60fd1f..0x60fd24: push 2, 9, fleet). Cleared by `StartGrWar` / `SpaceWar_Belep` | **SIM** + wait |
| 52 | WAITFORTEXT | - | `WaitForTextToEnd = 1` | wait |
| 53 | WAITFOREVENT | quoted event name | section 4: `TutorialEvent = id` (unknown name: unchanged); for every planet owned by EN planet +0x186 = 1; `TutorialTextShown = 1`; `TutorialPause()` (`if Active { Paused=1; Active=0; ClearKeyboardBuffer(); KeyShift=0 }`); `TutorialInitEvents()` | CTL + SIM bits |
| 54 | LOADSCENARIO | quoted | **no-op**: skips to the quote then to `>`; the scenario comes from `TutorialLoadScenario(chapter)` at start (script names are ignored; chapter 5 names tutorial4.scn but loads tutorial5.scn) | none |
| 55 | LOADSAVEGAME | 1 int n | `SingleStartGame(); OldIdo=OldIdo2=(SDL_GetTicks()/50)*50; ProgramNowStarting=1; saved=Counter; LoadSave(n,"",1); Counter=saved; ReadDebugData(); SetIdoSpeed(1,false)`; skip; then `OptionsCsakLoadLehet=0; SocketBlock=0`. Not used by main.tut | **SIM** |
| 56 | GAMESPEED | 1 int n | n==0: `SetIdoPause(true,false)`; else `SetIdoSpeed(n,false); SetIdoPause(false,false)`. SP: n=1/2/3 -> `IdoSpeedSzorzo` 1/4/16, battle multiplier 1/2/4 (elf2/SetIdoSpeed.c); gated by `nemlehetidotallitani==0` | **SIM** (clock) |
| 57 | CHAPTER | quoted | **no-op** marker | none |
| 58 | WAITFORATLEPES | - | `WaitForAtlepes = 1` | wait |
| 59 | DIPLOMACY EXPLORE EMPIRE | 1 int e | `FelfedezBirodalom(e, true)` (ELF 0x50f090 / exe 0x4c1d60) then `BirodalomIsmeri[e + EN*0x13] = 1` | **SIM** |
| 60 | EXPLORE EMPIRE | 1 int e | `BirodalomIsmeri[e + EN*0x13] = 1` only (exe base 0xbaebb8) | **SIM** |
| 61 | GIVE KILLER FLEET | 1 int e | if e == EN: p = **last** planet in 1..HanyBolygo-1 (strict <) owned by EN; `FlottaNew("Terminator", EN, 0, x+20.0, y-20.0, 0.0)`; `TankNew(f,2,0x19)`; `HajoNew(f,3)` x3; `HajoNew(f,6)` x1; select+centre (AktFleet/StarmapMode=2/KellSelect); every planet with owner==6: `explored[EN]=1`. Else: `FlottaNew("Unbeatable", e, 0, (float)(120-20*e), 300.0, 0.0)` + `HajoNew(f,0x2b)` x100. main.tut uses `GIVE KILLER FLEET 6` (e != EN: the Unbeatable fleet at (0,300)) | **SIM** |
| 62 | GIVE KOLONIZALO FLEET | - | p = last planet in 1..HanyBolygo-1 owned by EN; `FlottaNew("Colonization", EN, 0, x+20, y-20, 0)`; `HajoNew(f,4)`; select+centre | **SIM** |
| 63 | PAUSE | - | `SetIdoPause(true,false)` | **SIM** (clock) |
| 64 | UNPAUSE | - | `SetIdoPause(false,false)` | **SIM** |
| 65 | IRRITATEME | 1 int (ignored) | `DiplomaciaTalkAbout(6, EN, 4, -1)` (exe 0x4bdac0): AI empire 6 opens a diplomacy exchange with EN (meaning of 4/-1 UNCERTAIN). Not used by main.tut | **SIM** |
| 66 | SPY MISSION | 1 int m | `TutorialSpyMelyikLehet = m` (only mission m selectable on the spy screen; Stop -> -1) | FLAG |
| 67 | ADD DESTROYERS TO TRADE | - | `DestoryerATradereTutorialhoz()` x4 (ELF 0x7b0850 / exe 0x6057f0): 4 destroyer offers on the trade market | **SIM** |
| 68 | DISABLE NEW SPIES | - | `TutorialUjKemekLetiltva = 1` (history string copy-pasted "MAKE AN EMPIRE IRRITATING ME") | FLAG |
| 69 | DISABLE TEXTWINDOW | - | `TutorialDisableTextWindow = 1` | FLAG |
| 70 | GOTO SCREEN | 1 int n | `AktKepKell = n` (screen switch next frame) | UI |
| 71 | SEND MESSAGE | 1 int id | `NewMessage(id, 0)` (exe 0x508fe0): inbox message | **SIM** (message list) |
| 72 | RESTART | - | `TutorialRestarting = 1` -> Frissit sets `PufferPtr = 0` (script restarts at its first SEEKCHAPTER; no state reset, `TutorialStartCounter` unchanged) | CTL |
| 73 | TEXTURE | quoted name | copies the name into `DemoMode_TexturaNev` (ELF 0x1810720, unbounded; consumed by demo mode) | UI |

Not used by shipped `main.tut`: TIME, COUNT, LOADSAVEGAME, IRRITATEME, GOTO SCREEN, SEND MESSAGE, RESTART, TEXTURE, QUIT, END. `SEEKCHAPTER` runs at script start; each chapter ends with `RETURNTOMAINMENU`.

## 4. WAITFOREVENT names -> ids -> who fires them

`strcasecmp` ladder, first hit wins; an unknown name leaves `TutorialEvent` unchanged (a stale or 0 id: nothing resumes it).
`TutorialCheckEvent(id)` (ELF 0x7b3560 / exe 0x60bdf0): `if (TutorialEvent == id) { TutorialEvent = 0; TutorialUnPause() }`.
`TutorialUnPause` (elf2): `if Paused { StarmapResetKeyboardAndMouse() (exe 0x5afc10); ClearKeyboardBuffer() (fk_from=fk_to; exe 0x4db030); ColonyInitKeyboard() (exe 0x462890); Paused=0; Active=1; KeyShift=0 }`.
`TutorialInitEvents` (elf2; exe inlined, call 0x611a30): FelfedezettBolygokSzama = number of planets with `explored[EN]==1`; BirtokoltBolygokSzama = planets owned by EN; TankokSzamaFlottaimban = `CountMyTanks()` (own fleet vehicles + ground-war units).
`TutorialCheckEvents` (ELF 0x7b39a0 / exe 0x60be40, called every tick while Paused): event 2 if `Birtokolt < owned planets now`; event 0x19 if `CountMyTanks() >= baseline + 3`; event 0x1e if the vehicles of own fleets named "Assault fleet" (case-insensitive) whose design byte +0x36 is 0 sum to more than 3 (ELF tests bit 0 only, the exe tests the whole byte: see section 8).

Event ids are hexadecimal below (as in the code).

| id | name | fired by (ELF function: condition) |
|---|---|---|
| 0x01 | exploreaplanet | `FRISSIT_Starmap`: a radar sweep explores a planet for the first time while Active/Paused and event==1; **also renames that planet** (`strcpy(planet name, GetString(2,4))`) |
| 0x02 | colonizeaplanet | `TutorialCheckEvents` polling |
| 0x03 / 0x04 | renameplanet / renamefleet | `DoKeyboard` (AktKep 2, `IOAzonosito` 0x15 / 0x16, Enter in a rename box) |
| 0x05 | splitfleet | `TransferOff` when `Tutorial_Split_Ok` (set by `Starmap_Main` while event 5 is awaited and a transfer icon 0x43..0x52 is clicked) |
| 0x06 | producedestroyer | `Starmap_Main`: production item chosen whose design kind (Tervek[EN] record +0x48) == 5 |
| 0x07 | gotocolony | `Colony_Belep` |
| 0x08 | click on tank turret 1.0 | `TalalmanyKivalasztva`: research tech index 0xa2 selected |
| 0x09 | research laser 2.0 | `FRISSIT_Research` (EN ResearchAktFejlesztett == 0x12) and a same-valued check in `FRISSIT_Spy`; the command also sets tech 0x12 state byte for EN to 3 |
| 0x0a | research heavy destroyer | same, tech == 6 |
| 0x0b | equip laser 2.0 | `Design_Main`, `KiszamolErtek`, `TervAutoDesign`: tech 0x12 placed in a slot of kind 5 |
| 0x0c | designdestroyer | `Design_Main`: design saved (AktAlap == 6) |
| 0x0d | make friends in diplomacy | `FeldolgozValasz`: reply from empire 6 with raw decoded fields (54==6, 44==2); meaning UNCERTAIN |
| 0x0e | disable empire 6 in diplomacy | **no caller** (never fires) |
| 0x0f / 0x10 | send a spy for information / counter intell | `Spy_Main` new-mission button: (SpyNewMissionKuldetes==2 and CelFaj==7) / Kuldetes==0xd |
| 0x11 | attack darkon empire | `Starmap_Main` attack click on a foreign planet or fleet (also fires 0x1f) |
| 0x12 | press i in spacewar | **no caller** |
| 0x13 | build a small house | `EpuletNew` type 2 on an EN planet |
| 0x14 | build a fusion plant | `EpuletNew` type 0x34 |
| 0x15 | movefleet | `MULTIGET_Starmap` message 0x15: own fleet ordered to a planet farther than the radar range from the last owned planet |
| 0x16 | exitfromplanetinfo | `Starmap_Main` planet-info closed (icons 8 with left click, 9, 0xb) |
| 0x17 / 0x18 | build a spaceship factory / tank factory | `EpuletNew` types 0x13 / 0x15 |
| 0x19 | build 3 tanks | `TutorialCheckEvents` polling |
| 0x1a | gotostarmap | `Starmap_Belep` |
| 0x1b | press pageupordown | `ColonyKeyboardHandler` (two sites) |
| 0x1c | gotonomaincolony | `DoKeyboard` key r, `FRISSIT_Starmap`, `MOUSE_SelectScreens`, `Starmap_Main` (x3): current planet name != `Tutorial_MainBolygoNeve` |
| 0x1d | build a police | `EpuletNew` type 7 |
| 0x1e | transfer tanks | `TutorialCheckEvents` polling |
| 0x1f | attack any empire | `Starmap_Main` (with 0x11) |
| 0x20 | win in spacebattle | `SpaceWar_Kilep` (not on screens 0xd, 1, 10) |
| 0x21 | win in groundbattle | `GroundWarFinished` winner == EN |
| 0x22 | 15masodperc | `TutorialFrissit` countdown, `VisszaSzamlalo = 0x640` UI ticks (16 s) |
| 0x23 | build a ship research | `EpuletNew` type 0xe |
| 0x24 / 0x25 | changetax / changeautobuild | `Colony_Main` tax icons 0x17/0x18 or list selection / autobuild list selection |
| 0x26 | buildtradeport | `EpuletNew` type 0x32 |
| 0x27 | buydestroyer | `Trade_Main` buy with design kind 5 |
| 0x28 | changetodiplomacy | `Diplomacy_Belep` |
| 0x29 | buildfoodfactory | `EpuletNew` type 4 |
| 0x2a | exchangedipl | `FeldolgozValasz` (raw pattern 6,1,..,0x12,3,0xf: meaning UNCERTAIN) |
| 0x2b | sendsattoplanet | `Starmap_Main` rop icons 5 / 6 (`Multi_Send(0x18, ...)`) |

`EpuletNew` fires only if the planet owner is EN. Building type numbers are the `param_2` argument of `EpuletNew`.

## 5. Gate flags and their readers (ELF xrefs, refs/elf-global-xrefs.txt; "not seen" = reader not decompiled)

| Flag (set by) | Read by | Effect |
|---|---|---|
| KepernyokLetiltva[20] (ENABLE/DISABLE [ALL] SCREEN) | `MOUSE_SelectScreens` (screen buttons; screen 2 button), `DoKeyboard` (F-keys: index Key-0x6f, [0xd], [10]), `Starmap_Main` ([3] planet -> colony), `Fake_FRISSITES` (**writes** [6],[7],[8] every tick from the diplomacy/spy/trade enable flags and [6] from `KigyujtPlayerek()==0`), `SpaceWar_Belep`/`Kilep` (saves to `backKepernyokLetiltva`, sets all 1, restores), `InitNewGame`, `Starmapvisszaallit`, `screenfrissites`, `screen_button_state_callback` (not seen) | greys / blocks the screen buttons and hot keys |
| TutorialEpuletekLetiltva[0x3c] (ENABLE/DISABLE [ALL] BUILDING) | `Colony_Main` (building list click, index = building class byte at `Talalmanyok[b]+0x27`), `ColonyGfx2D_4` (draws the entry greyed, colour 0xc) | |
| TutorialSplitDisabled | `Starmap_Main` (rop icon 0xd and the transfer/split block) | |
| TutorialProductionDisabled | `Starmap_Main` (rop icon 2 = production) | |
| TutorialDisableHajokEladasa | `Starmap_Main` (sell ship) | |
| TutorialExitGombDisabled | `Starmap_Main` (production panel exit, `AktIkon2==9`) | |
| TutorialStarmapMinden... | `StarmapKeyboardHandler`, `Starmap_MouseInit2` | cursor-move keys removed |
| TutorialColonyMinden... | `ColonyKeyboardHandler`, `Colony_MouseInit2` | PageUp/Down keys removed |
| TutorialDisableEpuletLerombolas | `Colony_MouseInit2` (`Mouse_Szoveg_Off(2)` = demolish tooltip) | |
| TutorialSpyMelyikLehet | `Spy_Main`, `SpyGfx2D_3` (`spySelected` forced -1 unless == allowed mission) | |
| TutorialUjKemekLetiltva | `FRISSIT_Spy` (blocks AI spy creation; the `rndlog(4,0,..)` draw happens first, so the RNG stream is unchanged) | SIM |
| debug AI flag 0x144c494 | `Fake_FRISSITES` (skips `FRISSIT_MI`), `MI_DIPLOMACY`, `FRISSIT_MI_DESIGN`, `SpaceWarFrissit` | SIM |
| debug battle flags 0x144c480..48c | `SpaceWarIndit`, `GroundWarIndit`, `SetOptions` | SIM |
| TutorialEvent | `DoKeyboard`, `FRISSIT_Starmap`, `MULTIGET_Starmap`, `MOUSE_SelectScreens`, `Starmap_Main`, `TutorialCheckEvent(s)` | event-specific side logic |
| TutorialActive / TutorialPaused | ~40 readers incl. `MainCallBackEventHandler` (SDL: key t toggles `TextShown`, o opens ESC, j with DEBUGLEHET stops; Active and AktKep != 0xd: live mouse motion/buttons and Shift/Ctrl/Alt key events dropped), `Mouse_Lekerdez`, `ReadNext`, `Colony_Belep`, `*_Autoscroll`, `FRISSIT_Starmap`, `MULTIGET_Starmap`, `FRISSIT_Spy`, `IdoNapValtozott`, `ScrollUzenet`, `Options_*`, diplomacy handlers, `Show_Stars2d3d`, `SpaceWarKeyboardHandler`, `Design_Main`, `Spy_Main`, `Colony_Main` | UI behaviour while a tutorial runs; `IdoNapValtozott`, `FRISSIT_Spy`, `FRISSIT_Starmap`, `MIvisszavonulasvizsgalat` are sim code: what they change when the flags are set was not read (UNCERTAIN) |
| TutorialWaitForBattle | `StartGrWar`, `SpaceWar_Belep` (clear) | |

Sim-code readers of Active/Paused that I did read: `IdoNapValtozott` (ELF, elf3): the automatic monthly save (`Nap==1 && Honap%6==1`, SP, `OptionsKellAutoSave`) is skipped while Active or Paused.
`FRISSIT_Spy` (elf3 ~lines 232 and 457): while Active/Paused it zeroes one local (`local_a0`) and forces another (`local_51`) to true, i.e. the spy pass behaves differently in the tutorial; exact meaning UNCERTAIN.
`FRISSIT_Starmap` explore-rename (event 1) and `MULTIGET_Starmap` (event 0x15) as in section 4.

## 6. Mouse and keyboard playback

`TutorialPlayMouse(name)` (ELF 0x7b3310 / exe 0x611db0): path = `tutorialpath` ("tutorial/", exe char* at 0x928eac) + `tutorialactpath` ("main/", set by TutorialStart) + name; reads `u32 count` into
`ActMouseAllapotMax`; frees the previous buffer, `malloc((count+2)*0x24)`; per sample reads 12 bytes (x, y, button, left, right, mods) at +0, one flag byte at +0x20, and when flag==1 20 bytes (4 virt + 4 u32 key) at +0xc;
then `Mouse_Akt_Bal=Jobb=Gomb=0; ActMouseAllapot = 0`. A new PLAYMOUSE replaces a running one. (Exe: same; file helpers 0x4c9a30/0x4cb000/0x4cb020/0x4cafa0, `free`/`malloc`.)

`Mouse_Tutorial_Kezel` (ELF 0x5b7230 / exe 0x510930), once per 10 ms tick, before TutorialFrissit:
1. If `(MouseSave || Active) && AktKep != 0xd && OldCounter != TutorialCounter` (i.e. every tick the counter moved; not while Paused): if replaying (`!MouseSave`) or index > 49999: `Mouse_Akt_Bal = Jobb = Gomb = 0` (live clicks wiped each tick);
   if recording and index <= 49999: `index++`; then if Active and `index < Max` and `index != -1`: `index++`; `OldCounter = TutorialCounter`.
   So **the sample index advances by 1 per UI tick** once PLAYMOUSE set it to 0, until it equals Max. From -1 (the Start state) it does not advance.
2. Recording (TutorialMouseSave): writes sample `index` from `Mouse_Akt_x/y`, the three button flags, modifier bits (Alt 1, Shift 2, Ctrl 4) and clears the key flag; Shift + left + right ends it (`TutorialStopMouseSave` writes the file, message "Tutorial successfully saved.").
3. Playing: if `!Active || index == Max || index == -1`: when Active and AktKep != 0xd the pointer is parked: `Mouse_x=0x140; Mouse_y=200; Mouse_Jobb=Bal=Gomb=0; Mouse_Tipus=-2` (exe 0xdb4654/58/52/51/50, 0xdb9414).
   Else (AktKep != 0xd): `Mouse_Akt_x/y = sample.x/y; Mouse_Akt_Gomb/Bal/Jobb = byte & 1; if Bal: Mouse_LastPressed_Bal = (Mouse_x, Mouse_y); if Jobb: same for Jobb; KeyAlt = mods&1; KeyShift = (mods&2)!=0; KeyCtrl = (mods&4)!=0; Mouse_Move(x, y, true)`.
   (exe globals: Mouse_Akt_x/y 0x916008/0x91600c, Akt_Gomb/Bal/Jobb 0xdb467d/e/f, KeyAlt/Shift/Ctrl 0xbae294/0xbae295/0xbae27f, LastPressed 0xdb9424.. / 0xdb942c..).
Keys: `ReadNext` (elf2/ReadNext.c, exe 0x57f9b0): when Active and AktKep != 0xd live keys are not returned; if `index != -1` and the sample flag == 1 it returns the first non-zero key slot of sample `index` (`Key_Buffer = key[i]; Key_Virt = virt[i]`) and zeroes that slot
(each recorded key is delivered once; up to 4 per sample, one per call). When recording it stores the live key (`FKBuf[fk_from]`) into the first free slot of the current sample and sets the flag. `Mouse_Lekerdez` copies `Mouse_Akt_*` into `Mouse_*` directly while Active or recording.
SDL layer (`MainCallBackEventHandler`): while Active and AktKep != 0xd the mouse motion and button events and the Shift/Ctrl/Alt key events are dropped, so only the recording drives the pointer. `TutorialCounter` stops while Paused, so a sample is consumed once per UI tick of Active play.

## 7. Start, stop, launch

* Launch: main-menu tutorial choice -> `MainScr_Kilep` (ELF 0x59bec0 / exe 0x4fecc0): `Mode_Tutorial_Start=0; ElinditTutorial(TutorialStartMelyik)`. ELF `ElinditTutorial` (0x59c3b0): `MainKorokTavolsagCel = 6.0 (0x40c00000); TutorialSetCampaignGameOptions(n); TutorialStart("main", n)`.
  **Exe: inlined** in `MainScr_Kilep` (`DAT_00bbb0bc = 0x40c00000; TutorialSetCampaignGameOptions(DAT_00bbb230)` call at exe 0x4feec2; `TutorialStart("main", n)` at 0x4feecd; flag `DAT_00bbb22c`). The symbol-table row "0x553c20 <-> ELF 0x59c3b0 ElinditTutorial" is **wrong** (exe 0x553c20 is an unrelated vector clean-up).
* `TutorialSetCampaignGameOptions(n)` (ELF 0x59c5f0 / exe 0x504250): `ProgramMode=0; SingleStartGame(); SetIdoSpeed(1,false); ProgramMachines=1; ProgramPlayers=8; ProgramPlayers2=0x12 (18)`; for i=1..18: `PlayerKi[i]=2; PlayerFaj[i]=i; PlayerNev[i*11] = Faj_Nev[PlayerFaj[i]]`
  with overrides after the assignment for i==15,16,17,18 -> `PlayerFaj[15..18] = 1,3,1,1` (exe 0xbae583..0xbae586; the name copy uses the overridden race); `EN=1; PlayerKi[1]=1` (exe 0x912a3c, 0xbae561); `ReadDebugData(); InitNewGame(SDL_GetTicks(), 1, 0, 0)`
  (**RNG seed = tick count: not deterministic**; mode argument 1); `TutorialLoadScenario(n)`; `if (debug story id 0x144c4b0 (exe 0xb77260) != 0) StartStory(id, false)`; `OldIdo = OldIdo2 = (SDL_GetTicks()/50)*50; ProgramNowStarting = 1`.
* `TutorialLoadScenario(n)` (ELF 0x7b3bd0 / exe 0x611c80): `LoadScenario("tutorial/scn/tutorial%d.scn", 0)`; on failure it builds a "Scenario load failed:" CString that is discarded (no message); then the same `OldIdo`/`ProgramNowStarting` reset.
* `TutorialStart(name, chapter)` (ELF 0x7b3d50 / exe 0x611f80): `TutorialStop(true)`; `DefaultChapter = chapter`; `tutorialactpath = name + "/"`; opens `tutorial/main/script/<name>.tut` (`.tut` = `tutorialextension`, exe char* 0x928eb4); reads the file; sets the Start column of section 1;
  `KepernyokLetiltva[0..19] = 0; TutorialEpuletekLetiltva[1..0x3b] = 0` (index 0 not cleared by Start, cleared by Stop); `TutorialDisableHajokEladasa = 1` (sell ships off by default); Mouse click flags zero; saves the debug AI flag;
  `GlobalMusicVolumeCsokkento = (int)((double)GlobalMusicVolume * 0.4)` (f64 multiply, truncate; exe 0x612278..0x6122cc).
  Other callers: console `ctut <name>` (`ExecuteString`, exe 0x4dc140: `TutorialStart(name, 0)` and `KellCounterKirakas = 1`), `StartDemoMode` (exe 0x4fe3c0: `TutorialStart("demo", 0)`; `demo.tut` is not in the install).
* `TutorialStop(bool restoreMusic)` (ELF 0x7b4280 / exe 0x6124e0): only if `Active || Paused`: free script + mouse buffer; `Active=0`; clear `KepernyokLetiltva[0..19]`, `TutorialEpuletekLetiltva[0..0x3b]`; `UjKemekLetiltva=0; SpyMelyikLehet=-1; WaitForBattle=0; Split/ProductionDisabled=0;
  DisableEpuletLerombolas=0; Paused=0; ExitGombDisabled=0; Starmap/Colony-Minden flags=0; DisableHajokEladasa=0; SelfPlaying=0; Tutorial_MainBolygoNeve=empty`; if `KellOverrunVisszaAllitas`: restore the four battle debug values;
  `if (restoreMusic) GlobalMusicVolumeCsokkento = 0`; debug AI flag = saved. Not reset: TutorialEvent, TextShown, DisableTextWindow, VisszaSzamlalo, StartCounter, WaitFor* flags except WaitForBattle.
  Callers: END/RETURNTOMAINMENU/QUIT/unknown tag/EOF (`true`), `MainCallBackEventHandler` window close (`false`), `ExitToMainScreen` also writes both flags (value not read, presumably 0).

## 8. Exe vs ELF

* Same 74 tests in the same order; same event ladder (ids and strings); same semantics for every command I diffed (flags, FlottaNew/TankNew/HajoNew args, LOADSAVEGAME, GAMESPEED, IRRITATEME `(6,EN,4,-1)`, SPY MISSION, DISABLE OVERRUN, WAITFORBATTLE, CENTER*/FIND*, TYPE, SEEKCHAPTER, EXPLORE EMPIRE).
* **Real difference, TutorialCheckEvents**: ELF tests `(design.byte[+0x36] & 1) == 0`; the exe compares the whole byte `CMP byte ptr [..+0x36],0` (exe 0x60bf48). Same result unless another bit of that byte is set.
* Exe inlines: the four wait checks (into TutorialFrissit), `TutorialUnPause` (into TutorialCheckEvent, calls 0x5afc10 / 0x4db030 / 0x462890), `TutorialPause` and `TutorialInitEvents` (calls 0x611d80 / 0x611a30 from the WAITFOREVENT code), `ElinditTutorial` (into `MainScr_Kilep`), `TutorialKeretFrissit` (into `GFX_DateMoneyScrollFade`). `SlideQueue` = 0x60bd70, `TKTombSoremeles` = 0x60bda0, `DemoMode_Frissit` = 0x60b740, `ResearchDoubleGomb` = 0x522020, `CountMyTanksInFleet` = 0x60b6d0, `CountMyTanks` = 0x60b5d0 (these are unnamed in the symbol table; matched by call position).
* Exe `TutorialGetNextCommand` decompiles as `void` but returns the offset in EAX (updated along the `>` scan; unknown-keyword path `LEA EAX,[EDI-1]` at 0x611a09). If the closing `>` is missing at the end of the buffer the exe returns the offset before the scan, the ELF the remaining length (only for malformed scripts).
* `GIVE KILLER FLEET` / `GIVE KOLONIZALO FLEET`: with no EN planet in 1..HanyBolygo-1 the ELF reads an uninitialised local, the exe uses planet 0 (`iVar10 = 0`).
* `TutorialStart`: the exe leaves `Tutorial_LastMp2_Length` at 0 via the TKTomb-init loop counter (0xf67f88); the ELF sets 0 explicitly. Same value.
* `Mouse_Tutorial_Kezel`: exe nests the recording and replay blocks under `(MouseSave || Active)`; logically identical.
* `tutorialpath` / `tutorialextension` are char arrays in the ELF (0x119d988 / 0x119d998) and `char*` globals in the exe (0x928eac -> "tutorial/", 0x928eb4 -> ".tut").
* Symbol table: "0x553c20 ElinditTutorial" is wrong (section 7); `TutorialKeretFrissit` has no exe row (inlined).

## 9. Check in the asm (exe addresses; full listings in `exe-asm/`)

1. `AT` adders, exe 0x60c7d2..0x60c856 (the y copy 0x60c825..0x60c856): `CVTDQ2PS` int->f32, `SUBSS` against `TKActX/Y`, widen (`CVTPS2PD`), `ANDPD [0x848550]` (abs), narrow and widen again, `DIVSD [0x8523f8]` (= 40.0 double), `CVTPD2PS`, f32 store at 0xf673c0 / 0xf673c4. Reproduce as `((diff as f32).abs() as f64 / 40.0) as f32`.
2. `TYPE` Novelo, exe 0x60cdb2..0x60cdd4: **f32** `DIVSS`, `MULSS [0x852500]` (50.0f), `MULSS [0x852b84]` (160.0f); zero-length file -> 15.0 (0x60cdb0 `JZ`). Product order: divide, then 50, then 160.
3. Window motion, exe 0x60c0d2..0x60c186: pure f32 `COMISS`/`ADDSS`/`SUBSS`; the two directions are separate ifs with clamping (fall-through paths at 0x60c108..0x60c129 and 0x60c165..0x60c186).
4. Typing accumulator, exe 0x60c2b6..0x60c2eb: f32 `ADDSS Novelo + Counter`, `COMISS` against Max (100.0), `SUBSS`; one character per tick at most.
5. Music ducking, exe 0x612278..0x6122cc: `CVTDQ2PD`, `MULSD [0x848528]` (0.4 double), `CVTTSD2SI` (truncation) -> 0xe4166c.
6. Command-loop condition, exe 0x60c1b5..0x60c2ab: signed compare `Counter - StartCounter < TargetCounter`, the five flags and their clearing order; the Active byte is refreshed after every command (0x60c29f).
7. `TutorialCheckEvents` +0x36 byte compare, exe 0x60bf48 (see section 8); the vehicle record index uses `*8` with the type at +2 and the count at +4 (0xbb41a8 / 0xbb41aa / 0xbb41ac).
8. `Mouse_Tutorial_Kezel` exe 0x510930: the signed compares `49999 < idx`, `idx < Max`, `idx != -1`; the mod-bit shifts (`>>1 &1`, `>>2 &1`).
9. `GIVE KILLER FLEET` coordinates: exe 0x610dc8..0x610dee use f32 `ADDSS/SUBSS [0x8524e0]` (20.0f); the ELF decompile shows double constants; results identical for planet coordinates, keep f32. The `Unbeatable` fleet x = `(float)((6-e)*0x14)`.
10. `TutorialKeretFrissit` inlined copy in `GFX_DateMoneyScrollFade` (exe site not located): ELF computes `(int)(TKActX - 5.0)` with a double constant; same result as f32 for |x| < 2^19.
11. `DISABLE ALL SCREENS` skips index 13 and loops to 0x13 (confirmed in the exe loop); `DISABLE ALL BUILDINGS` bound 0x3c.
12. `WAITFORBATTLE` TankNew call exe 0x60fd1f..0x60fd24 (push 2, push 9, push fleet) verified.

## 10. Unknowns

* Meaning of tech state 3 (event 9), planet byte +0x186, `MoneyHozzaadLog` second argument (0), `DiplomaciaTalkAbout(6,EN,4,-1)` parameters, `FlottaNew` third argument, fleet `+0x16 = 0x29a` and `+0x18` bit 0 for traders (see the fleet/trader specs in `re/rules/`).
* What `FRISSIT_Spy` (two sites), `FRISSIT_Starmap` and `MIvisszavonulasvizsgalat` change while Active/Paused (only the sites found by xref; their bodies were not read).
* `TutorialRepeatLastSzovegReszlet` (elf2) and `TutorialStartMp2` (ELF 0x7b4b20, old numbered files `Ncom_tutp%d_%03d.mp2`) have no caller found.
* Events 0x0e and 0x12 have no `TutorialCheckEvent` caller (not used by `main.tut`).
* Exe site of the inlined `TutorialKeretFrissit`; `DemoMode_*` and the `TEXTURE` consumer (`DemoMode_Poll`) not read.
* `ctut` / `demo.tut`: `demo.tut` is not shipped, so `StartDemoMode` would fail to open it (not checked what `DatFile::dopen` does then).

Resolved at runtime (§11): `SEEKCHAPTER` for a chapter the script lacks leaves the pointer at the end and the
interpreter Active and idle (traced with chapter 6: the script's sixth part is labelled "Spying screen", so no
chapter number reaches it).

## 11. Port and verification

**Port** (`engine/crates/ig2-sim/src/tutorial.rs`): `Tutorial` holds §1's state; `start`/`stop`, `mouse_tick`
(`Mouse_Tutorial_Kezel`'s replay step), `frissit` (`TutorialFrissit` with the command loop, the window glide and the
typing step), `count_tick` (`TutorialCounter++` unless paused), `ui_tick` (the three in `Fake_FRISSITES`' order),
`next_command` (`TutorialGetNextCommand`), `check_event` / `check_events`, `battle_started`. The game and the UI are
reached through `TutorialHost` (the screen, the 20 Hz `Counter`, transitions, the speech stream and its length, the
`.mou` sample count, the planet/vehicle counts) and everything a command does outside the interpreter goes out as an
`Effect` (§3; *sim* ones are tagged). `Game::tutorial_effect` (`game/tutorial.rs`) carries out the in-place writes:
money, tech state, `SHOW PLANET`, `EXPLORE EMPIRE` (and `FelfedezBirodalom` for the diplomacy form), autobuild,
traders, the planets' +0x186 mark. New fleets/vehicles/ships, the trade market's destroyers, `IRRITATEME`, messages,
the clock and `LOADSAVEGAME` are left to their systems (not wired yet). The overlay reads `Tutorial::window` (grid,
position, cursor column, `blink` brightness), `Tutorial::gates` and `Tutorial::mouse_sample`. The tutorial is single
player; its *sim* effects belong in the lockstep queue at the tick they happen. Deviation: a quoted name across a line
break where the exe hangs stops the tutorial (`Effect::Hang`, `re/deviations.md`).

**Verification** (`tools/trace/run_tutorial.py` + `tutorial.js`): the tracer picks the chapter the way the main menu
does (`Mode_Tutorial_Start` 0xbbb22c, `TutorialStartMelyik` 0xbbb230, `AktKepKell = 2`), records every
`TutorialFrissit` with its inputs and the state after it (the grid and queue when they change), each command's hooked
calls (`ExistAnyTutorialMp2`, `TutorialStartMp2_2`, `TutorialPlayMouse`, `MoneyHozzaadLog`, `TankNew`) and every
`TutorialCheckEvent` outside `TutorialFrissit`. An event awaited for 300 UI ticks is fired by the tracer (the player's
action); `WAITFORBATTLE` is cleared after 600. The replay (`tests/tutorial_trace.rs`) feeds the port the same inputs
and compares every tick's state bit for bit (f32 window position, steps and typing counter included). The screen
gates are also written between ticks by `InitNewGame`, `Starmapvisszaallit`, `screenfrissites`, `Fake_FRISSITES` and the
space battle; the replay takes them from each tick's entry. Results: `traces/2026-10-22-tutorial.md`.
