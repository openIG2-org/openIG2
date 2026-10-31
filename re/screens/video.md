# Video playback

The 1999 code asks for Bink names (`title.bik`, `misc/race_01_loop.bik`, …); the GOG remaster plays them from
`anim/mpeg4/*.mp4` through its own `videoentity` (an FFmpeg wrapper, `SDL_ffmpeg_*`). The ELF `ig2` holds the same
remaster code, so there is no RAD Bink logic in either binary. Decompiles: `_local/decomp/video-exe/0x<addr>.c` and
`_local/decomp/video-elf/`. Decoder decision: ADR 0013. Client status: §6.

## 1. Files

`anim/mpeg4/` holds 423 `.mp4` (ISO-BMFF inside the `neGs` container, `formats/ecgs-container.md`), all H.264
High profile at 25 fps (CABAC, B-frames, 8×8 transform, implicit weighted bi-prediction):

| group | count | size | sound |
|---|---|---|---|
| top level (cut-scenes, story videos, intros, endings) | 43 | 640×400 (`tutorial.mp4` 512×384) | AAC-LC 44.1 kHz stereo |
| `misc/` | 36 | 640×400 (6), 332×188 (30) | only `altalanos_elvesztes`, `altalanos_megnyeres`, `credits` |
| `messages/<race>/` (brutal, human, pirate, politikus, trader) | 344 | 332×188 | none (the voice is a separate ogg, `PlayMessagesHang`) |

Some clips' sound track ends before the picture (`arriving.mp4`: 11.5 s of sound, 13.0 s of picture).
`ig2_formats::mp4` demuxes them (sample tables, `avcC`, `esds`). Unreferenced by name: `tutorial.mp4`,
`wormhole.mp4`, `misc/{kereskedo,pirate,worm}_{loop,wait}.mp4` (probably reached through message data, §5.3).

## 2. API (exe)

- `OpenBink(name, fullscreen, x, y, force)` exe 0x7325b0 (ELF 0x425ca0) → bool. Returns false at once when
  `!force && KellenekCutScenek == 0` (`DAT_0091605c`, the options' cut-scene flag 0x6a, default 1: exe 0x7325c6,
  0x517155). Else it queues `command_OpenBink` exe 0x733380 on the render thread and waits; the result is
  `ig2_binkres` (`DAT_00fe6427`).
- `command_OpenBink`: marks credits (`DAT_00fe6425`, name contains `credits.bik`) or title (`DAT_00fe6426`, contains
  `title.bik`); path = `"anim/mpeg4/" + remove_extension(name) + ".mp4"`, `stringhelper::fixup` (slashes, case);
  `ig2screen::openAnim` exe 0x736960: entity `+0x290` = `fullscreen`, `+0x29c` = 1 (clock-timed), `+0x2f4` = 0xe5b
  (the sound's lead-in, §4), `initvideo` 0x70ec40 → `prepare` 0x70f980 (decodes sound only when `fullscreen`) →
  `Loader_Video_Entity` 0x70e7d0. Sets `bink_playing` (`DAT_00db9438`) = 1 and places the quad (§3).
- `PlayBink(blocking, loop, _)` exe 0x732660 (ELF 0x426100) → `command_PlayBink` 0x7335b0 → `ig2screen::playAnim`
  0x736d40: nothing without a decoder or while playing. Else: `blocking` → `sgsnd::stop_channels("music")` (the
  music is **stopped**, not paused or ducked); `ig2_continousplay` = blocking; visible; `setLoop(loop)` 0x7100c0;
  `play()` 0x70f870 (start time `+0x2a8 = SDL_GetTicks() − DAT_00fd1534`, the decoder thread `workerfunc`
  0x7104d0, `Mix_HookMusic(soundhook)` when `fullscreen`). A blocking play sets `ig2_suspended` (`DAT_00fe6420`):
  `updateIG2` 0x737bb0 stops resuming the game thread, so the game code waits inside `PlayBink` while the render
  thread keeps drawing (exe 0x737c6b). Non-blocking plays (the message clips) leave the game running.
- `CloseBink()` exe 0x732540 (ELF 0x426240, `command_CloseBink` 0x426270): clears the credits/title marks and the
  credits text; `closeAnim`: `bink_playing = 0`, stop, hide, `ig2_continousplay = 0`.
- End: at the file's end `workerfunc` seeks back to the start when looping (`ctx+0x44`), else it queues an end
  marker; `videoentity::render` stops on it, and `ig2screen::render` (exe 0x737737) then clears `bink_playing`,
  hides the quad and ends `ig2_continousplay`, which resumes the game thread. The caller runs `CloseBink`.

## 3. Pacing and placement

- **Pacing** (`videoentity::render` exe 0x70f9e0, once per drawn frame): `elapsed = SDL_GetTicks() − DAT_00fd1534
  − start` (the pause-compensated base), `target = (int)((float)elapsed / (1000.0f / fps))` with `fps` the
  stream's (f32, 25 → 40 ms exact); it pops decoded frames while `target ≥ shown`, at most 10 per call, and shows
  the last: frame *n* appears at `elapsed ≥ 40·n` ms, late frames are dropped. The wall clock alone times the
  picture; the sound is not a clock. A loop continues the count (no restart of the clock). [asm checked at exe 0x70fa44..0x70fa91: u32 elapsed → f64 → f32, `divss`, `cvttss2si`]
- **Shader** `render/video_fragment.shader` (Y, Cb, Cr planes, bilinear, clamped): `Y' = 1.1643·(Y − 0.0625)`,
  `R = Y' + 1.596·Cr`, `G = Y' − 0.391·Cb − 0.813·Cr`, `B = Y' + 2.018·Cb` (the uniforms are named the other way
  round: `video_u_texture` carries Cr), times the vertex colour; `video.state`: blend `SRC_ALPHA /
  ONE_MINUS_SRC_ALPHA`, no depth test. No scan lines (unlike the 1999 screens' `VIDEO` material, `render.md`).
  The 8-bit planes map to `n/255` in GL, so neutral chroma 128 is +0.002: grey gets R and B one step up.
- **Fullscreen** (`fullscreen = 1`, openAnim's else-branch): in units of a centred space 1024 wide and
  `1024·H/W` high (`W × H` the window), `h = 1024·H/W`, `w = vidW·h/vidH`; if `w > 1024`: `w = 1024`,
  `h = vidH·1024/vidW`. Centred: aspect kept, letter- or pillar-boxed over the cleared window (black). 640×400 in
  800×600 → 800×500 at y 50; in 16:9 full height with side bars. Linear filtering.
- **In the screen** (`fullscreen = 0`): `x, y` in the 640×480 virtual space of the 4:3 viewport
  (`FUN_004d84d0`, `DAT_00b770d0..dc`): width `vidW·viewW/640` px (integer division), left
  `offX + viewW·x/640`, likewise top. The messages screen uses (201, 204): a 332×188 clip at 201..533 × 204..392.
- **Title overlay** (remaster): while `title.bik` plays, `gfx/thqnordic.pvr` on a quad of `(w/2, w/8)` units at
  `(0, w·240/1024)` (`w` the video quad's width), for `t < 5649` ms of the clip: alpha `t >> 3` up to 2048 ms, 255
  to 3602 ms, then `255 − ((t − 3602) >> 3)`; the colour `0xFFaaaaaa` modulates RGB (fades through black).
- **Credits text** (remaster): while `credits.bik` plays, for `t < 12288` ms, "REMASTERED BY GYROSCOPE. PUBLISHED
  BY THQNORDIC" in the platform font (`PlatformUtils::RenderText`, `render/font.material`) at (W/2, 0.96·H) with a
  2 px shadow; alpha `t >> 4` up to 4096 ms, 255 to 8192 ms, then `255 − ((t − 8192) >> 4)`.
  Rendered once (`ig2screen::render` exe 0x7370f0, cached in 0x92fcc0): `RenderText(text, w = (H>>5)<<6, h = H>>5,
  size = H>>5, flags 0xc0 = centred)` (exe 0x6e9130); `GetFont` 0x6e84b0 returns 0 for the empty font name, so
  `RenderTextToSurface` 0x6e9670 takes its GDI fallback 0x6f0e90 (not SDL_ttf): `CreateFontA(nHeight = +p = H>>5, weight 400,
  ANTIALIASED, "Arial")`, `DrawTextW` DT_CENTER top-aligned in a (p<<6) x p box, the blue byte as coverage, the top 4 bits as
  alpha, white texture, both sides rounded up to 4; drawn 1:1 in window pixels (`FUN_007115e0`, quad 1152 × 20 at 800 × 600 [obs:
  `probes/credits_text.js`]) centred at `((W>>1), 0.96·H)` over a black copy at +2, +2 with `(a·0x333333334)>>34` =
  4a/5 alpha. GDI Arial 18 gives capitals on rows 3..14 and 457 px, the exe's capture 456 [obs].

## 4. Input and sound

- **Skip** (`ig2screen::anyEvent`, exe `FUN_007329d0`, ELF 0x425350), while `bink_playing`: the first key, mouse
  button or finger press arms it (`DAT_00fe6438`); the next release of any of them stops the video, **only when
  `fullscreen`**. So the title, credits and every `st_ShowAnim` video (intros, endings, story videos, the
  console's `anim`) skip on any key or click (on release, after a press made during the video); the message clips
  never skip. ESC has no special case. `MainSkipAllAnims` (0xbbb3f6) only skips the title.
- **After the skip logic** `anyEvent` (exe 0x7329d0; `bink_playing` 0xdb9438, armed 0xfe6438): a gate (`this+0x270`, its
  byte +0x20, an unknown object) swallows only 0x300/0x301/0x303/0x401/0x700; the button and finger releases (0x402,
  0x701) always reach `FUN_004df9b0` -> `FUN_004e2cd0`, the game's handler (`MainCallBackEventHandler` 0x4e2db0 sets the
  mouse state 0xdb467d..f on 0x402). So a release that skips a video is also seen by the game (not
  captured). SDL_QUIT (`MainCallBackEventHandler`): `CloseBink` if a video plays, `TutorialStop(0)`,
  `SaveGlobalRegistrySettings`, `MouseIkonKep = 0`, quit flag 0xb771e0 = 1.
- **Sound** (fullscreen only): `soundhook` exe 0x7100e0 through `Mix_HookMusic`, outside sgsnd: the game's other
  sounds keep playing, the options' volumes are not applied (raw S16 stereo at the mixer's 44.1 kHz). The first
  0xe5b = 3675 frames (83.3 ms) of each play are silence, and an empty queue (underrun) resets that count (another
  lead-in after a stall). In-screen clips decode no sound.
- **When the sound starts** [obs: `probes/video_sound.js` with `run_video.py --real`]: the hook runs from
  `play()` on, but `workerfunc` (exe 0x7104d0) decodes the sound (`fill_audio`) only from the packets
  `SDL_ffmpeg_get_next` reads on its way to the next picture, and it decodes at most the ring's 2 pictures
  (`Loader_Video_Entity` 0x70e7d0 sets `+0x2a4 = 2`; `play()` hands it 2 semaphore permits and waits for the first
  picture) plus the H.264 reorder delay ahead of the shown one: **4 pictures** observed. Until the picture nears the
  file's first sound chunk every hook call (1024 frames) is an underrun, then the lead-in, then the track sample-exact
  (the hook's output equals FFmpeg's decode delayed, no gap later). `title.mp4` (and the other 640×400 clips:
  `intro_*`, `arriving`, `altalanos_*`, `tutorial`) store 13 pictures before the first sound chunk: 16 empty calls
  (picture frame 9 = 360 ms), first non-zero frame 22310 = 3675 + 16384 + the track's own 2251, at video time 495 ms;
  the hook then stays 16-17 ms ahead of the picture clock to 30 s. `misc/credits.mp4` stores 1: no wait, first
  non-zero frame 4042. Nothing else sounds during the title (no `SoundEngine::Play`).
- **The title's music is the clip's own**: `title.mp4` is one 88 s clip, the Imperium Galactica II logo (with the THQ
  Nordic overlay, §3) to ~6 s, the Digital Reality logo to ~12 s, then the intro; its single sound track (84.3 s) is
  continuous music from 0.5 s, so the exe plays that music under both logos too. No music ducking: a blocking play stops the music, and
  once the game runs again `UpdateFrame` (exe 0x4e3e90) starts the next main track because the loaded one stopped
  (`audio.md` §4). [unknown: SDL_mixer applies no music volume to a hook; not read in the asm]

## 5. Callers

| caller (exe) | name | mode | after |
|---|---|---|---|
| `StartDemoMode` 0x4fe3c0 (ELF `MainScr_Belep`), call 0x4fe3f6 | `title.bik` | fullscreen, blocking | once per run (`VoltGTLogo` 0xbbb22f); skipped with `DEBUGLEHET` or `MainSkipAllAnims`; `IdoPauseMertAtlepesAlattVan` = 1 around it; then the main menu |
| `Mainscreen_Update3D` 0x500270, call 0x50147d | `misc/credits.bik`, `force` | fullscreen, blocking | the credits ring; the menu resumes |
| `CampaignGameStart` 0x4f5e90 (0x4f6008/1f/36) | `intro_human.bik` / `intro_politikus.bik` / `intro_brutal.bik` by `EN` 1/3/5 (three plain `if`s: nothing for other races) | `st_ShowAnim`; no guard of its own (no `MainSkipAllAnims`, debug, MP or `CampaignType` test): only `OpenBink`'s cut-scene flag (force 0) | its last step, after `InitNewGame`, `OldIdo`/`OldIdo2`, `ProgramNowStarting = 1`; then `MainScr_Kilep` 0x4fecc0 → `ScreenBetolt(2)`, `FRISSITES_IGAZIT`, `Starmap_Belep` (the music, the start voice) |
| `st_ShowAnim(name)` 0x602bb0 (ELF 0x739500) | any (empty: nothing) | `OpenBink(name,1,0,0,0)`, `PlayBink(1,0,0)`, `CloseBink`, `FRISSITES_IGAZIT()`; pause flag 0xbae265 around it | the caller goes on |
| `MEGHALAS` 0x53b630 | `VideoLoss[PlayerFaj[EN]]` (exe 0x919724, char* × 9; outside a campaign `misc/altalanos_elvesztes.bik`), then `misc/end_gameover.bik` | `st_ShowAnim` × 2 | the main menu (`AktKepKell` 0xf) |
| `MEGNYERES` 0x53f6b0 / `MegnyeresEllenoriz` 0x53f8c0 | campaign: `VideoWin[PlayerFaj[EN]]` (0x919748), then `misc/end_campaign_win.bik`; else `misc/altalanos_megnyeres.bik`, then `misc/end_victory.bik` | `st_ShowAnim` × 2 | the main menu (`AktKepKell` 0xf) |
| `Story_Special` 0x5c9550 and ~40 story functions (`video-exe/refs4.txt`), the story script 0x5e6ba0 (two inlined copies) | from the story (`s_anims.txt`, table 6: 57 names (4 more commented out), all resolve) | fullscreen, blocking | the story goes on |
| `ExecuteString` 0x4dc140 (the cheat console's `anim`, call 0x4dd9e4; ELF 0x57dde0) | `animN` → `s_anims.txt` table 6 entry `N` (`atoi` after the 4 letters), else the typed rest as is (no extension or path added) | `st_ShowAnim` | only caller `DoKeyboard` exe 0x4db1a0 (0x4db73c) on Enter in the chat line; cheats need `DEBUGLEHET` (0xbae659, a command-line switch) or `DEBUGX` (0xbae587); not a tutorial command (re/rules/tutorial.md §3 has none) [asm checked at exe 0x4dd9a9..0x4dd9e4: `N == 0` skips the lookup; `_s` 0x542ce0 gets table 6, `N`] |
| `StartAnim` 0x5095b0 (ELF 0x5a1340) | `AnimFile` (0xd909c0) when `UzenetekRandomKitol[UAktUzenet] == 0`, else the loop clip of `VideoMessageClips[idx]` (exe 0x915e60, {loop, wait} × 15) | `OpenBink(name,0,201,204,1)`, in the screen, silent, forced | `PlayMessagesHang`, `PlayHalkAlienHang` |
| `MessagesAnimFrissit` 0x506f60 (ELF 0x5a36f0), every frame | – | when the message is open, not closing and `mesframe == 30`: hide sub-mesh 5 of the frame and `PlayBink(0, AnimAkt ≤ 1, 0)` (loop for the first two clips, once for the end clip); then `DrawAnimInterlace` | – |
| `VegeAzElsoAnimnak` 0x509e40 (ELF 0x5a1890) | `AnimAkt = 1`: the wait clip (`0x915e64 + idx·8`) or `d90ac0` | `CloseBink`, `OpenBink(…,0,201,204,1)`, `StopHalkAlienHang` | chosen by `FRISSIT_Messages2` 0x505180 when the speech ended and the part's flag is set; a failed open → `StartAnim_EndOfMessage` |
| `StartAnim_EndOfMessage` 0x5096a0 (ELF 0x5a1690) | `AnimAkt = 2`: `misc/end_%d.bik` for the human's race 1/3/5, else `misc/end_x.bik` | in the screen, once | `PlaySound(messages_end_wav)`, undock (`0xe4166c = 0`) |
| `Messages_Kilep` 0x5083d0 | – | `CloseBink` | – |

### 5.1 The message clip table
`VideoMessageClips` (exe 0x915e60): index 0 null, 1..=8 `misc/race_0N_{loop,wait}.bik`, 9..=12 empty strings,
13 `misc/scorpus_*`, 14 `misc/b5_*`. Read at run time (`ig2_rules::Videos::message_clips`).

### 5.2 The interlace overlay
`DrawAnimInterlace` (ELF 0x5a37a0, exe via `FUN_00504fb0`): a quad of `finalinter.bmp` over the clip (x 201..533,
y `InterlaceMover + 204` .. `+392`; two triangles), vertex colour 0x19ffffff (alpha 25/255), size 2^-16, rhw 0.5,
every frame with the clip. No writer of `InterlaceMover` found (0 assumed). The texture coordinates run `lmin`..`lmax`
both ways from the exe's floats 0x9148cc/0x9148d0 (the ELF's initial values -0.0049 / 1.4649): the texture repeats
~1.47 times (PVR `TXNV` 11 = repeat), not stretched (exe 0x504fb0; not captured).

### 5.3 The talking-head clip (ELF decompiles, checked against the exe)
- **Names**: `GetTxtFileCommands` (ELF 0x5a7c70) on a part whose header says `TE` (the other side) sets
  `MesKellAnimInditas` and calls `GetUzenetAnimLista(msg, part, AnimFile, AnimFile+0x100, SoundFile, …)` (ELF
  0x5a4a80): defaults `"messages/"` (no file); in `text/uzenetanim_<EN 1/3/5, else 1>.txt` the first group header
  (a line without `!` holding the message id), then the first part line `%02d` = the part (the scan stops at the
  first line not starting with a digit): `!clip` → `AnimFile`, a second `!clip` → the wait clip (`messages/` +
  name unless it holds `misc/`, + `.bik`); `#n` is `UzenetFaj` (the alien voice of `PlayHalkAlienHang`, clamped
  1..=19), not a clip. Parts are read when reached, so the last `TE` part read wins. Exe: `AnimFile` 0xd909c0,
  wait 0xd90ac0, `SoundFile` 0xd90bc0.
- **Start**: `MessagesGfx2D_2` (ELF 0x5a1d30), only at `mesframe == 30`: when the typer's front character has
  colour 2 (`MsgSzin`, the `|C`2 of a `TE` part) and `MesKellAnimInditas` is set → `StartAnim` (exe 0x5095b0):
  `CloseBink`, `AnimAkt = 0`, `OpenBink(name, 0, 201, 204, 1)` with name = `AnimFile`, or the loop clip of
  `VideoMessageClips[k]` when the inbox entry's `UzenetekRandomKitol` (exe 0xdabc10) `k` is set (story scripts'
  `SetMessageFajOverride`: 1..=8, 13, 14, else 1), then the voice (`PlayMessagesHang`, `PlayHalkAlienHang`).
- **Wait** (`VegeAzElsoAnimnak` exe 0x509e40): `AnimAkt` 0 → 1, the wait clip (or the table's wait clip for `k`);
  when it doesn't open (`bink_playing` 0) → the end clip. Called when the typer first reaches the end
  (`KiirtaMar` 0 → 1) with `AnimAkt` 0, or by `FRISSIT_Messages2` (every 11th call) when the speech ended
  (`SOUND`, `Lang` 1, `MesHangIndult`, `AnimAkt` 0, the ogg done) for a conversation (`UzenetFajtak[k] + 4` set;
  else the end clip).
- **End** (`StartAnim_EndOfMessage` exe 0x5096a0): nothing if `AnimAkt` is 2; else `misc/end_<race>.bik` for the
  human's race 1/3/5, else `misc/end_x.bik`, once; `PlaySound(messages_end)`, the music's ducking off. Called each
  frame the typer is at the end of a plain message or of a dialogue's last text (`ParbeszedVegeJon`).
- Answering, the next part or a new message close nothing: the next `TE` text's `StartAnim` does. `Messages_Kilep`
  closes the clip; `Messages_Belep` resets `AnimAkt` and the flag.
- `mesframe` (exe 0xdabe44, f32): +1/3 a UI update to 15 (opening), to 30 once a message is opened
  (`KELLKINYITAS2`), −0.6 closing; the clip waits for exactly 30 (messages.md §4.1). Hiding sub-mesh 5/0 shows
  nothing: from 30 on the exe draws the screen saved at 30 (`SaveScreen(3)`, before the 2D pass that hides it) instead
  of the model, and `Messages_Kilep` shows it again before the closing draws the model.

### 5.4 Open
- Which music follows `title.bik`; re-layout on a window resize during a video (none found).

## 6. Client status

`ig2-client::video`: `ig2_formats::mp4` + `video::decode` (OpenH264, Symphonia AAC, ADR 0013), §3's shader arithmetic
and pacing (`Player`, four frames ahead; `update_blocking` on the script clock for captures), the sound in the mixer's
video slot after the lead-in and `Clip::sound_wait` (§4; test `sound_starts_as_the_exe`: title 22310, credits 4042),
placement, skip (the skipping release also reaches the game), the interlace uv, the credits text via
the same GDI calls on Windows (an outline stand-in elsewhere). Callers wired: title, credits, story videos,
endings, message head clips (`video::message::HeadClip`, waiting for `MessagesScreen::text_up`), the campaign
intro. Missing: the console's `anim` (no cheat console). OpenH264 is vendored (`engine/vendor/README.md`) with
the High-profile CAVLC level fix (`level_prefix` 16, which `credits.mp4`'s first IDR uses); test `credits_first_gop`.
Captures: `tools/trace/run_video.py` (`video.js`: the title
with `SDL_GetTicks` driven by the frame limiter, grabs at video times; `--open NAME [--at X,Y] [--once]` plays a clip
in the screen) against `ig2-client --snapshot --screen video:NAME --video-ms T [--video-at X,Y] [--video-loop]`.
In multiplayer the session keeps running under a video (`World::run_on`; multiplayer.md §7.2, deviations 412).
