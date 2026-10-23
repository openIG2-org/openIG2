# The audio layer

How ig2.exe plays music, sound effects and voices, and what `ig2-client::audio` copies. The sound tables, `PlaySound`'s
three voices and the sound-id tracer are in `sound.md`; this file adds the engine under them, music, volumes and the
call sites per screen. Sources: the ELF (`sgsnd` names, -O0) for the structure, the exe for the build we match (both
use the remaster's `sgsnd` library over SDL_mixer). Decompiles: `_local/decomp/audio/{elf,exe,exe2,exe3}`; the full
call-site table (297 ELF sites): `_local/audio/callsites.md`.

## 1. Files
- Every sound, voice and music file of the GOG install is **Ogg Vorbis** (44.1 kHz; the effects mono), all in the
  remaster's `neGs` container (`ig2_formats::Install::read`/`open_stream` take it off).
- `SE_GetRealName` (ELF 0x6725a0): the VFS path, then a `.wav` or `.mp2` suffix becomes `.ogg`. Table names
  `dir/Name.wav` are `dataraw/sounds/dir/name.ogg` (lower case), else `sounds/dir/name.ogg` (the 20 `colony/st/`
  files live only there). Ten `Colony_Hangok[27..36]` (night birds) have no file: silence.
- Music `mfx/<name>.ogg` (16 files). Message speech `dataraw/messagesounds/<race>/<NNNN_NN>.ogg` (races 1, 3, 5).
  Localised voices (colony speaker `colony/speech/*`, `starmap/speeches`, `scroll/*`, `start`) under
  `localization/<lang>/dataraw/sounds/` (no German message speech). The tutorial's `mfx/tutorial/*.mp2` are not in the
  GOG install.

## 2. The engine (`SoundEngine`, exe 0x55dfb0..; ELF 0x672e10..)
- `InitMainSound` exe 0x55c6a0: `SoundEngine::Init(…, 44100)`, `SOUND` 0x919930 = `MUSIC` 0x919931 = 1 (config
  `sound`/`music` "0" clears them; no music without sound), then `InitSounds(Global_Hangok, 11)`.
- Handles: waves `0..`, streams `100000+i`, the one music `400000`; slots are the first free, no reserved channels.
- `LoadWave(name, flags, &h)` exe 0x55dfb0: `flags & 2` → stream, `& 0x10` → music, else wave. For a `Hang` the
  flags are its `+0x104` field (0, 4 or 6 in the tables); `SE_Wave::LoadWave` exe 0x55dd70 loads the sound with
  sgsnd flag 2 when bit 1 is set (a sound that is stopped before it is played again), never sgsnd's loop bit (bit 0).
  **No sound effect loops**: the colony's ambients are restarted by `Frissit2_Colony`.
- `Hang+0x114` set = **loaded by `InitSounds`** (ELF 0x670ea0, exe 0x55c790); clear = loaded when first played
  (`PlaySound` exe 0x55cd20). Every record in the GOG tables has it clear (sound.md §1 said the opposite).
- `SoundEngine::Play(h, loop)` exe 0x55e2a0: `sgsnd::sound_t::play(0, (v·100) >> 7)` with v the handle's 0..128
  volume; `loop & 1` is only stored (+0x15). sgsnd 0x43c200 then `Mix_PlayChannelTimed(−1, chunk, loops, −1)` with
  loops = −(flags & 1) = 0. `GetStatus` 1 stopped, 2 playing. `Poll` (exe 0x55d0d0 → 0x55e340, every frame from
  `UpdateFrame`) frees finished auto-release voices.
- `SetPan` has no effect (`SE_Wave::SetPan` is empty in the ELF; only the main menu's flybys call it).

## 3. Volumes
- `ConvertVolume(v)` ELF 0x670660 = `(int)(v·128/100)` in f64 (`SoundEngine::MaxVolume` 128).
- Effects: `PlaySound` volume = `(int)((G·vol/100)·128/100)` (f64; G = `GlobalSoundVolume` exe 0x91992c, vol =
  `Hang+0x110`), then `(v·100) >> 7` per cent reaches sgsnd. `MainPlaySound` (menu flybys): 70 per cent fixed.
  `PlayWav`/`Speech::Play`/`PlayMp2`: `ConvertVolume(G)`.
- Music: `PlayMusic` ELF 0x670d70 / `SetGlobalMusicVolume` ELF 0x670940 (exe 0x55d0f0):
  `x = (int)((Hangero/100.0)·(float)M − duckReal)`, clamped ≥ 0, volume `ConvertVolume(x·0.9)`; M =
  `GlobalMusicVolume` exe 0x919928 (0..100, Options).
- Ducking, the target `GlobalMusicVolumeCsokkento` exe 0xe4166c: `PlayMessagesHang` sets `(int)(M/3.0)` (f64 divide by
  the qword 3.0 at 0x852360, `cvttsd2si`; M 100 → 33) and `Messages_Kilep` (exe 0x5083f9) sets 0 again: the music stays
  ducked for the rest of the screen's visit, after the speech too. `Starmap_Belep` sets `M·60/100` for the new
  campaign's start voice; `UpdateFrame` exe 0x4e3e90 sets 0 when **that** voice (`HangLejatszasID` exe 0x912a44) is no
  longer playing or `AktKep > 0xd` (only for it, not for the message speech: the part 1 note was wrong).

## 4. Music
- Tracks `ZeneNevek` exe 0x91a698 (19 `char*`, read from the user's exe): 0/1 gwar1, 2 spwar1, 3–5 brutal1–3, 6–8
  human1–3, 9–11 pol1–3, 12 menu, 13–15 gwar1–3, 16–18 spwar1–3.
- The selectors end in `PlayMusicExistCheck(id, a, immediate, loop)` exe 0x540430 (a missing file falls back to its
  group, then to a libc-`rand()` group; with every file present it plays `id`) → `PlayMusicGlobal` ELF 0x671b00:
  - **Main** `PlayMusicMain` exe 0x5407c0: nothing while `ZeneEppenJatszott` (exe 0xe3e3ac) is 3..11. Else the next
    of the race's three from `ZeneRegiMainZene` (exe 0xe3e3b0; 3→4→5→3, 6→7→8→6, 9→10→11→9); from 0 a libc
    `rand() % 3` pick by the player's race `PlayerFaj[EN]` (exe 0xbae574[0x912a3c]): 1 → 6.., 3 → 9.., 5 → 3..,
    else 9; `SelfPlaying` (0xbae267) → 1. Not looped.
  - **Menu** 12 (looped flag, `ZeneRegiMainZene = 0`), **ground war** `13 + random(3)`, **space war**
    `16 + (u32)(SDL_GetTicks()·7) % 3` exe 0x540920.
  - The loop flag only sets bit 4 of `Zene+0x104`; `SE_Music` loads with sgsnd flags 2 and `Play` ignores it:
    **no track loops**.
- `PlayMusicGlobal`: with nothing loaded or `immediate`: fade level `Hangero` (exe 0xe4167c) = target `Cel`
  (0x91a694) = 100, the old track stopped and the new one started at once. Else `Cel = 0` and the name waits in
  `KovetkezoZene` (0xe41680).
- `FRISSIT2_SoundSystem` exe 0x55c4f0, once per 100 Hz UI tick (`Fake_FRISSITES`): the duck level steps ±0.5 (f32)
  toward its target; `Hangero` −1.0 per tick toward 0, then the next track starts with `Hangero` 0, `Cel` 100, and
  rises +1.0 per tick: **a crossfade is 100 ticks down, 100 ticks up**; the volume is re-applied every tick.
- `UpdateFrame` exe 0x4e3e90, every frame: a loaded track that is no longer playing (and not paused) →
  `ZeneEppenJatszott = 0; PlayMusicMain(0, immediate)`: when any track ends (the menu's too) the next main track starts.
- Callers: `MainScr_Belep` → Menu(immediate); `Starmap_Belep` exe 0x5b19a0 → Main (crossfade; not while a space-war
  return is pending); `SpaceWar_Belep` exe 0x562680 → SpWar (the client plays it at entry and the main music on leaving); `Frissit2_Colony` exe 0x477dc0 → GrWar when a ground
  battle starts on the shown planet, Main when it ends. `Pause/UnPauseMainMusic` have no callers.

## 5. Voices and message audio
- `PlayWav(name, loop)` exe 0x563e60: a wave of its own at `ConvertVolume(G)`, only with `SOUND`. `Speech::Play`
  exe 0x563d50 the same, only with `KELLBESZED` (exe 0x996469, = `SOUND` at `Colony_Belep`). The file goes through
  `SE_GetRealName` (§1); the VFS takes `localization/<lang>/` first (es, fr, hu, it have every voice; de only the
  scroll set).
- **Scroll messages**, `FRISSIT_Scroll2` exe 0x556210 (ELF 0x66ebd0): when a sim message is taken onto the free line,
  its id (`ScrollUzenetek+0x38`) speaks unless it is 1, 0x14, 0x15 or 0x1a: `PlayWav("SOUNDS/SCROLL/BRUTAL/%04d.WAV",
  1)` when `EN == 5`, else `…/HUMAN/%04d.WAV` (64 files each, `0001`..). Not for the screens' own error lines
  (`HibaUzenetSzoveg`).
- **New campaign**, `Starmap_Belep` (ELF 0x6f27bc): `SOUND && HangLejatszasKell > 0` (exe 0xbae26c, set by
  `InitNewGame` to the campaign type 1..3) → `HangLejatszasID = PlayWav("SOUNDS/START/faj%dstart.WAV", 0)`, the music
  ducked to `M·60/100` (§3), `HangLejatszasKell = 0`; the last thing the entry does. The client plays it (`--new-campaign`: intro, then music + this voice with the duck).
- **Speakers**, `SpeakingDirector` (ELF `speech.cpp`): a list of `Speaker`s (id), each a list of `Speeches` (kind:
  `+0` count, `+8` the `Speech` list, `+0x10` cooldown), each `Speech` a file name. Built once at start-up
  (exe 0x4dfe46) by `InitColonySpeech` exe 0x47f400 (ELF 0x4d2250, `ColonySpeaker` exe 0xa3ea3c, 753 `Add`s; its
  first statement `KELLBESZED = SOUND`) and `StarmapInitSpeech` exe 0x5ad820 (ELF 0x6f0150, `StarmapSpeaker` exe
  0xf17d88, 220 `Add`s): literal names, no conditions, each `push kind; push speaker; push name; mov ecx, director;
  call SpeakingDirector::Add` (exe 0x563b00), so the client reads both tables from the user's exe
  (`ig2_rules::speech`). Speakers 1–3 `H_soldier1..3`, 4–6 `B_…`, 7–9 `P_…`. Kinds: starmap 0 `Move`, 1 `Select`,
  2 `Attack`; colony 0 `Move`, 1 `Selection`, 2 `Attack`, 3 `Retreat`, 4 `Self-Destruct`, 5 `Hit`, 6 `Dying`, 7
  `Dead`, 8 `enemy_hit`, 9 `enemy_dead`, 10 `enemy_retreat`, 16 `Weapons` (`Sounds/<Colony/Speech|Starmap/Speeches>/
  <Dir>/<H|B|P>_soldier<n>_<NN>.wav`). Original slip: `Dead/P_soldier3_84.wav` is added to speaker 8.
  - `SpeakingDirector::Play(line, kind)` exe 0x563cd0 (ELF 0x67fe30): nothing for line 0 or `Lang == 3`; speaker =
    line, + 3 for `EN` 4/5, + 6 for 3/7/8 (a missing speaker or kind asserts). `Speeches::Play` (ELF 0x680200): only
    with the cooldown at 0: cooldown = 10, `random(count)` (libc), the n-th of the list (`Add` prepends: the n-th from
    the last added), `Speech::Play` (exe 0x563d50: a wave at `ConvertVolume(G)`, only with `KELLBESZED`).
  - `Refresh` (every set's cooldown −1, floored at 0): the starmap's in `FRISSIT_Starmap2` (ELF 0x719070) every 10th
    `StarmapCounter` (+1 a UI tick on the starmap), the colony's in `Frissit2_Colony` every 10th `ColonyCounter` (+1
    a UI tick on every screen): **a line can speak again after 91..100 ticks (about a second)**.
  - Starmap (`Starmap_Main`, ELF 0x6f50a0): line `AktFleet % 3 + 1` for a fleet of `EN` only: kind 1 when a pick on
    the map makes a fleet the selection (ELF 0x6f8e3b, exe 0x5b4a7f); after a right-click order (`Multi_Send(0x15)`)
    kind 0 for `StarmapVillogasMode` 1, 0xd, 0xe without Shift (with Shift `PlaySoundGlobal(0)`; ELF 0x6f81e5, exe
    0x5b51ac) and kind 2 for 2 (an attack; ELF 0x6f8d1c, exe 0x5b5231).
  - Colony: a left release on the 3D view (`AktIkon2 == 0x14`, no armed air strike, not `Belep`/`Atlepes`) with
    `WF::GetSelected(0)` an own unit that isn't a mine (`+0x190 & 0x20000`) → line `sel % 3 + 1`, kind 1 (`Colony_Main`
    ELF 0x4dc4c7); Ctrl+A (select all own non-pod units) the same (`ColonyKeyboardHandler` ELF 0x4ddb61); the
    retreat message 0x12f on the shown planet (`MULTIGET_Colony`, line 1, kind 3 own, 10 another's); the ground war's
    `WF::PlaySpeechGrWar` (exe 0x48c670: shown planet, colony screen, not `Belep`/`Atlepes`/`Simulate`): a hit
    (kind 5 the human's victim, else 8 the shooter's), a group's death (7 / 9), a move order (0), an attack order
    (2), a detonation (4: no set). Every site and condition: ground-combat.md §11.12. The client gets the sim's
    sites as `GroundEvent::Voice` (with owners; the window picks the kind for its `EN`) and plays them while its
    colony screen shows the war's planet; the sample comes from its cosmetic stream (deviation 38).
- `PlayMp2` exe 0x563df0 (stream): `TutorialStartMp2_2` exe 0x612430 (`mfx/tutorial/%s`).
- **Message speech**, `PlayMessagesHang(name, flag)` exe 0x55c9b0 (record `MessagesHang` exe 0x91a6e8, volume 100):
  `t0 = SDL_GetTicks()` (exe 0x91a800); with a name and `Lang == 1` (exe 0x912724 holds 1 in the GOG install whatever
  the UI language) and the file `dataraw/messagesounds/<EN>/<name>` there (`EN` exe 0x912a3c, the empire number: the
  install has folders 1, 3, 5 with 591, 668, 265 files, no localised copies): `StopMessagesHang`, the duck (§3),
  `PlaySound(&rec, 1)`. Else the handle (`+0x100`, exe 0x91a7e8) is 0: `MessagesHangJatszikMeg` exe 0x55c890 then
  counts `SDL_GetTicks() − t0 < 6000` ms as speech (a message without its file waits 6 s). `flag` only switches
  `+0x104` 2 → 6.
  - `StartAnim` exe 0x5095b0 (ELF 0x5a1340; when it fires: video.md §5.3): `MuteSoundGlobal(7)` (the typer), the
    clip, `PlayMessagesHang(SoundFile + AnimAkt·0x100, 0)` with `AnimAkt` 0, i.e. `SoundFile` = `%04d_%02d.ogg`
    (message id, part; `GetUzenetAnimLista` sets it for every `TE` part read, the `%04d_00-LOOP.ogg` beside it is
    never played and no such file exists), `PlayHalkAlienHang`, then `MesHangIndult` (exe 0xd909ad) = 1 when `SOUND`
    and `Lang == 1`.
  - **The speech's end**, `FRISSIT_Messages2` exe 0x505180 (ELF 0x5aad60), each UI tick on the open messages screen;
    every 11th (`MessagesGyorsito` > 10, then 0): `SOUND`, `Lang == 1`, `MesHangIndult`, `AnimAkt == 0`, `SoundFile`
    holding `ogg`, the speech over (`MessagesHangJatszikMeg` 0) and a message open (`FolyamatbanU != −1`) → for a
    conversation (`UzenetFajtak[KerUz(id)] + 4`) `VegeAzElsoAnimnak` (the wait clip, `MesHangIndult = SOUND`; the end
    clip when the wait clip doesn't open: `bink_playing` exe 0xdb9438 0), else `StartAnim_EndOfMessage`.
  - `StartAnim_EndOfMessage` exe 0x5096a0, once (`AnimAkt` → 2): `StopHalkAlienHang`, the end clip,
    `PlaySound(&Mes_Hangok[2], 0)` (`screens/messages/messages-end.wav`, exe 0x914b88) and `GlobalMusicVolumeCsokkento
    = 0`: **the music comes back when the end clip starts**, else at `Messages_Kilep`.
  - `StopMessagesHang` exe 0x55d1e0 with `StopHalkAlienHang` and `MesHangIndult = 0`: a left or right press in the
    text (`0xb9 < x < 0x201`, `0 < y < 0xa8`; the right one also `SkipAll`), a new message taken
    (`VoltUjUzenet == 1` in `Messages_Main` exe 0x508560: an inbox row or an automatic one), `Messages_Kilep`.
- `PlayHalkAlienHang` exe 0x509120 (ELF 0x5a1470): the message video's alien voice. `idx` from the clip name
  `AnimFile` (`pirate` → 10, `trader` → 9 for `EN` 1/3/5; a `misc` clip → `UzenetekRandomKitol[UAktUzenet] − 1` unless
  that equals `EN`; `UzenetFaj` exe 0xd90ddc overrides when set), `StopHalkAlienHang` (exe 0x509760: stops all 16),
  then for `idx != −1` record `[1→11, 2→9, 3→3, 4→3, 5→1, 6→1, 7→2, 8→8, 9→7, 10→4, 11→15, 12→4, 13→0, 14→0, 15→4,
  16→4, 17→12, 18→4, 19→10, else 4]` of the 16 `alienhangok/Alien-*.wav`, which are `Mes_Hangok[3..=18]` (exe
  0x914ca4 = `Mes_Hangok + 3·0x11c`): `PlaySound(&Mes_Hangok[3 + rec], 0)`, `Translating` (exe 0xda65ce) = 1.

## 6. Sounds per screen (exe functions; ELF in `callsites.md`)
Ids: `Global[0]` Click1 (the click), `[2]` Click3, `[5]` benyilas1 (zoom), `[7]` Kiirashang (the typer), `[10]`
Click5 (speed). "Press" = `Mouse_Lenyom` (exe 0xdb465f), "release" = `Mouse_Felenged` (0xdb465c); `AktIkon2` =
`AktIkon − 0x1e` (exe 0xbae240). The libc draws below are cosmetic (deviation 38).

### 6.1 Starmap (`Starmap_Main` exe 0x5b2b70: 62 calls to `PlaySoundGlobal` 0x55d0b0, `StarmapBeZoomol`/`KiZoomol` inlined 11 times)
| Sound | Trigger |
|---|---|
| G10 | the top bar's speed buttons (`AktIkon` 3..5, `IG2_gameUpdate` exe 0x4e08c6) **on the press** when paused or at another speed; the keys 1/2/3 (`DoKeyboard` ELF 0x5800d0) whatever the speed; both single player only, `AktKep < 0xd`, not 10, speed not locked. Pause and space are silent |
| G0 | minimap press (7); the production view's buttons: previous/next own planet 0xc/0xd, repeat 0xe..0x15, into-fleet 0x16..0x19, the counts 0x1a..0x29 on the press edge only (held repeats and the transfer cells 0x43..0x52 silent), a design list pick 0x2a..0x31 on the release that changes the slot; exit 9 |
| G0 ×2 | the tanks (10) and production (11) buttons: their own click, then the radial icon arm they run (`AktRopIkon = AktIkon2 − 9`) |
| G0 | radial icons 1, 2 (not with `TutorialProductionDisabled`), 3, 5–8, 9 (vehicle info), 0xd (split, before its zoom), 0xe (join); 4 (colony) and 0x12 silent |
| G5 | `StarmapBeZoomol` when not zoomed in (`StarmapZoomMode` 0), `StarmapKiZoomol` when zoomed; the long info's leave (8, 9, 0xb, exe 0x5b303c); the info box in planet mode (left: the long info, right: the colony; exe 0x5b6373); a planet-list pick (0x32, release) |
| G0 | the info box in fleet mode (exe 0x5b6a07), again when it closes a transfer panel (0x5b6b99); the long info's buttons 0x5b..0x63 on the press; the tax/auto-build/priority menus on the release over an item; the station sale menu on its opening press and on its pick |
| G0 | the rally pick (planet mode, own planet, right press; or the left pick in the redirect mode), before the pick (0x5b5406); an own fleet's order with Shift (0x5b51b3; without Shift the fleet's voice, §5) |

### 6.2 Colony (`Colony_Main` exe 0x4666d0, `Colony_Belep` 0x465b40, `Frissit2_Colony` 0x477dc0)
Planet type = `Bolygok+0x22`; `C[i]` = `Colony_Hangok[i]`, `Gr[i]` = `Gr_Hangok[i]`; `r(n)` = libc `rand() % n`.
| Sound | Trigger |
|---|---|
| entry | after `Init3dColony`: type 3/6 → `C[12+r(2)]`; a rainy field (`WF+0x8d0 == 2`, `OptionsKellIdojaras`) → `C[37+r(3)]` (a test of its own); type 1/2/5/8/9 → `C[53+r(3)]`; 4 → `C[56+r(2)]`; 7 → `C[43]`. No music; `Colony_Kilep` silent |
| ambient | every UI tick on every screen `ColonyCounter` (f64 0x99823c) += 1; on the colony screen (`AktKep == 3`, not `Belep`/`Atlepes`) every 5th: `r(1000) < 4` → type 1/5: `rand()&1 == 0 ? C[17+r(10)] : C[14+r(3)]`; 2/8/9: `C[49+r(4)]`; 3/6: `rand()&1 == 0 ? C[9+r(3)] : C[8]`; 7: `rand()&1 == 0 ? C[44+r(3)] : C[3+r(5)]`; 4: none (about one a 12.5 s). The night birds `C[27..36]` are never used |
| music | on the colony screen, not self-playing: a ground war starting on the shown planet (`WF::GetStatus == −1`) → `PlayMusicGrWar(1, 0)`, ending → `PlayMusicMain(0, 0)` (`VoltHarc` 0x99646a) |
| G0 | radar 5 and build menu 6 (own, `NemSzabadEpiteni == 0`) on the press; the on/off switch 0x11; E (owner `== EN`, the planet's `+0x44` clear) after its maybe empty 0x132; 3 (deselect); the 3D view 0x14 when another building becomes the selection, and its right click that drops the build cursor; the ground war's buttons 8, 0xa..0x12; the info modes 0x16..0x1e; the menus 0x15/0x19/0x1b on a release over an item |
| `C[r(2)]` | the build start (`colony_start_build` exe 0x4a88a0, before `Multi_Send(0x1f)`); the repair window's yes (exe 0x47d170, with or without the building) |
| `Gr[13+r(4)]` | the demolish window's yes with the building there (exe 0x47ced0, after `Multi_Send(0x20)`); Shift+Delete (`rand()&3`) |
| `C[3]` / `C[4]` | `Build3dBuilding` exe 0x49c780 / `Remove3dBuilding` 0x497760: any building made or removed on the shown planet (`EpuletNew`/`EpuletDelete`), whoever's |
| thunder | `UpdateRain` (ELF 0x4bffb0), called by `ShowColony` every drawn colony frame: with `KELLVILLAM` (`InitWeather` ELF 0x4c1160: the field's weather `WF+0xa00 == 2` and `OptionsKellIdojaras`) `random(400) < 1` → `PlaySound(&Colony_Villamok[random(4)], 0)` (exe 0x8f1198, 9 records `Groundwar/Thunder01..06`, `Thunder-BGR-1..3`, volume 75; only 0..=3 are drawn) and the light `villamid` at (0x60, 0x60, 0x80) for that frame, else black. A frame-rate dependent draw: about 0.15 a second at 60 Hz |

### 6.3 Info (`Info_Main` exe 0x4e81c0, 10 sites, all G0)
On the press: chart mode 0xe, star log 0xf, the charts 0x10..0x19, list up/down 0x1e/0x1f and star log scroll
0x23/0x24 (every press, whether anything moves). On the release over an item: the tax/auto-build/priority menus
0x1a..0x1c (with their order); opening them is silent.

### 6.4 Spy (`Spy_Main` exe 0x58d410, all on the press)
G0: send 0xe, cancel 0xf, the mission/race/target lists 0x10, 0x11, 0x12/0x13, kill 3 (not while `TutorialPaused`),
the verdict and report buttons 0x14..0x18, hire/reject/later 0x1e..0x20, repeat 0x21; G2: the skills 0x19..0x1c. G0 on
a carousel pick (`BeallitKemekAtlatszosaga` exe 0x583cb0: a spy, `z > −880`, no dialog; the current one too).
`Spy[1]` when the turn to another spy starts (`Spying3DUpdate` exe 0x590410: `spyframe ≥ 25`, `AktKem != SpyOldKem`,
`(int)spyAtfordulas != 0`), only once the screen is open (`spyKINYITVA`). Enter `Spy[0]`, exit `Spy[2]`.

### 6.5 Diplomacy (`Diplomacy_Belep` 0x4bf530 `Dip[0]`, `_Kilep` 0x4bf810 `Dip[1]`)
`DiplomGfx2D_2` exe 0x4bc610: a press within 40 px of another empire's head → G0, and `Dip[2]` when its turn is at rest
(`(int)Atfordulas == 30`, which restarts it). `DiplomGfx2D_1` exe 0x4b89e0: 15 G0 sites, each a press over its menu
(the main menu's opening, exchange, the send/cancel and answer entries, OK).

### 6.6 Messages (`Messages_Belep` 0x507f90 `Mes[0]`, `Messages_Kilep` 0x5083d0 `Mes[1]`)
The typer (`MessagesGfx2D_2` exe 0x507020), `Global[7]`: a new message (`KiertekelUjuzenet`) stops, starts and mutes
it; each typed character `PlayOrContinueSoundGlobal(7)` (restarts it when it has ended: nothing loops), mutes it, and
unmutes it (`40·G/100`) unless the character is a space; the typer passing the end stops it (once), so does
`Messages_Kilep`. `Mes[2]` and the speech: §5.

### 6.7 Design, trade
Des[0]/Des[3], Trade[0]/Trade[1]; Global[0], Global[2] (screens not built).

## 7. The client (`ig2-client::audio`)
- `ig2-rules::sounds` reads the eleven tables and `ZeneNevek` from the user's exe; `audio::decode` decodes the Ogg
  files with `lewton`; `audio::mixer` is SDL_mixer's model (channels with a 0..128 volume, one music stream);
  `audio::output` plays it through cpal. Nothing is shipped.
- `Audio` keeps the exe's state: three voices per record (`PlaySound`), the music globals of §4 driven by `tick()` at
  the 100 Hz UI tick, the watchdog by `frame()`; `play_wav` (voices, a channel of their own), `play_or_continue`,
  `mute`, `stop_sound` (the typer). Every played id goes to a log (`--snapshot --script` writes it as
  `<out>-sounds.jsonl` in the tracer's record shape, for `run_screendiff.py --script`).
- The screens queue their ids (`sounds` on `Research`, `ColonyUi`, `InfoScreen`, `SpyScreen`, diplomacy's `Screen`;
  the starmap's panels and menus return `Action::Sound`; the window's own sites go to `App::sounds`; the typer's steps
  `Dialogue::typing`); `window.rs::audio_frame` plays them after the frame's input and UI ticks.
- Part 3: `ig2_rules::speech` reads both speakers' tables from the exe; `audio::speech::{Director, Speakers}` keeps
  the cooldowns (refreshed from `window.rs::audio_frame`: the starmap's only while it is the screen) and plays a line
  through `play_wav`; the starmap's select/order sites and the colony's release/Ctrl+A queue `Speak`s.
  `Audio::{play_message_speech, message_speech_playing, stop_message_speech, play_alien, stop_alien, set_duck}` and
  `window.rs::head_clip_frame` carry `StartAnim`, `FRISSIT_Messages2`'s switch, `StartAnim_EndOfMessage`'s sounds and
  the stops (`ClipStep::Hush`); `colony_ui::thunder` the thunder (60 draws a second); `SpaceWar_Belep`/the return
  call `PlayMusicSpWar`/`Main`. Without a sound card a found speech lasts its decoded length.
- `--snapshot --script S` on the starmap (or with `--colony P`) runs the window's own input and frame code headless
  (`window::script_run`: one UI tick a frame, the sim held, the script's input fed in as window events) and writes
  only the sounds. Scripts: `tools/trace/scripts/{starmap,colony}-sounds.json`.

## 8. Verified (2026-10-22)
- **Music** (`sound.js` music hooks, `run_screen.py --kep 2 --sounds`, quick-1.sav, `_local/traces-audio/music2.jsonl`):
  the main menu's `PlayMusicMenu` → `ExistCheck(12, 0, immediate 1, loop 1)` starts `mfx/menu.ogg` at once; before it
  `ZeneHangero` rose from 0 by 1 a tick (its start value). On the starmap `PlayMusicMain(0, 0)` at UI tick 825 with
  race 6 → id 9; `ZeneHangero` 99 at 826 … 1 at 924, `mfx/pol1.ogg` starts at 925 at 0, 100 at 1025;
  `ZeneRegiMainZene` = 9 after. No ducking in the run. Tests: `audio::tests` (pick, crossfade, volumes).
- **Research sounds** (sound.md §4's script `research.json`, the exe log `_local/screendiff/seq/research-seq-sounds.jsonl`
  against the client's `--snapshot --script` log): `Res[0]` at the entry, `Global[0]` at t 330 (the urgent button),
  `Res[1]` at t 479 (↓, 9 ticks after the key: the field's model reload with the wheel within 14 frames) in both. The
  client also clicks on D at 531: the exe ignores it because the urgent press's order never returns while the script
  clock holds the 20 Hz sim (`ButtonPressed` stays set); the client applies orders at once. A harness difference, not
  the game's.
- **Starmap sounds** (`starmap-sounds.json`, quick-1.sav, `_local/traces-audio2/starmap-seq-starmap-sounds-*`): the
  exe and the client play the same ids at the same ticks: G10 at t 50 and 100 (keys 2, 1), G10 at 150 (the ×2
  button's press; the client acted on the release before, now on the press like `IG2_gameUpdate`), G0 at 200 (the
  minimap press), G5 at 263 (the info box's long info: 3 ticks after the press, at its release) and 500 (its leave).
- **Colony sounds** (`colony-sounds.json`, planet 42, surface 8): the entry's `Colony_Hangok[54]` at t 0 (the client
  drew 55, one of the type's 53..55: the draw is the client's own), G0 at 50 (radar), 120 and 200 (the build menu's
  presses) in both. The E key can't be scripted: `ColonyKeyboardHandler`'s character doesn't come from the injected
  key event (the digits reach `DoKeyboard`). Tests: `colony_ui::tests` (entry and ambient picks).
- All 186 table sounds with a file and the 19 tracks decode (`audio::tests::the_installs_sounds_decode`).
- Diplomacy traced 2026-10-23 (diplomacy sequence): `Dip[0]` = screen13.wav only with the opening, `Dip[1]` = screen20.wav when the closing starts (none towards messages/help/options); the client matches.
- In the client and not traced: the info, spy and typer sites (§6.3–§6.6, from the decompiles), the
  colony's ambients, build/demolish/placed/removed sounds and war music, the scroll messages' voices.
- **Speakers, message speech, alien voices** (2026-10-23, `re/rules/traces/2026-10-23-speech.md`, a probe calling
  the exe's functions): the cooldown 10 and its refresh every 10th UI tick, the duck target 33 at M 100 and its 0.5
  steps, the 6000 ms of a missing speech, a found speech not looping, the alien records `Mes_Hangok[7]`, `[15]`, `[6]`
  as `alien_record` gives. The 220 + 753 lines are read from the exe and every file is in the install
  (`exe_tables::speech_tables_name_the_installs_files`).
- In the client from the decompiles and not traced: the speakers' trigger sites, the speech-ended switch, `Mes[2]`
  at the end clip, the thunder, `PlayMusicSpWar` at `SpaceWar_Belep` and `Main` on the return.
- **Ground war's voices** (2026-10-23, `re/rules/ground-combat.md` §11.12): the move and retreat calls verified with `run_screen.py --grwar --script tools/trace/scripts/grwar-voices.json --sounds`; the hit/death lines by `ground_rt_trace::battle_tick_matches_trace` (the `Voice` events reproduce the exe's ColonySpeaker cooldowns, 181 lines, 0 mismatches). Detonate (kind 4) plays nothing.
- Not yet: the space war's speaker (`SpaceWarSpeaker`, deviation 52), the right click's `SkipAll` stop, the thunder's
  light flash, the new campaign's start voice (the client starts from saves), other empires' builds' `C[3]`.
