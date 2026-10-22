# Sounds and input scripts

How ig2.exe starts its sounds, how the sound-id tracer records them per action, and how an input script drives the
exe and the client through the same frame sequence (roadmap §2 steps 3 and 5: "a sound-id log per action", "the
sound ids match in order", the capture diff for every state). Tools: `tools/trace/sound_tables.py`, `sound.js`,
`run_screen.py --script --sounds`, `run_screendiff.py --script`; the client's `--snapshot --script`
(`ig2-client::script`). Results: `diff-results.md`.

## 1. Sound tables and `PlaySound`
- A `Hang` record is 0x11c bytes (exe and ELF alike): the file name at +0 (0x100 bytes, relative to `sounds/`),
  the first voice's handle +0x100 (−1 none), loop +0x104, the second and third voices' handles +0x108 / +0x10c,
  the volume in per cent +0x110, "load when first played" +0x114 [`PlaySound` exe 0x55cd20, ELF 0x671070;
  `InitSounds` exe 0x55c790].
- `InitSounds(Hang*, n)` loads every record whose +0x114 is clear (`SoundEngine::LoadWave("sounds/" + name, loop,
  &handle)`), sets the others' handle to −1, and clears both extra voices.
- `PlaySound(Hang* h, bool raw)`: nothing unless `SOUND` (exe 0x919930) is set. A record still unloaded (+0x114 set,
  handle −1) is loaded first, from `"sounds/" + name` (`raw` 0) or `name` (`raw` 1). Then, by the first voice's
  status (`SoundEngine::GetStatus`): 1 (stopped) → its volume and play; 2 (playing) → the second voice (made on first
  use by copying the first, `FUN_0055d9d0`), then the third, each played if stopped; all three playing → the first
  voice stopped and restarted. Volume = `(int)(G·vol/100 · M/100)` with G = exe 0x91992c and M = exe 0x91a82c
  (f64 arithmetic).
- `PlaySoundGlobal(id)` exe 0x55d0b0 (ELF 0x6714c0) = `PlaySound(&Global_Hangok[id], 0)`.
- Voices and message audio go elsewhere: `PlayWav(name, loop)` exe 0x563e60 (`SoundEngine::LoadWave(name, 0, &h)`,
  loop flag, volume from G through 0x55c430), `PlayMp2` exe 0x563df0 (the tutorial's `mfx/tutorial/*.mp2`),
  `Speech::Play` exe 0x563d50, `PlayMessagesHang(name, bool)` exe 0x55c9b0 (the one record `MessagesHang`, filled at
  run time). Music (`PlayMusicMain/Menu/SpWar/GrWar` exe 0x61f360…) is not traced yet.

The tables (`<Screen>_Hangok` in the ELF's symbols; `sound_tables.py` finds the exe's copy by its names at the
record stride). A sound id = table + index:

| Table | exe | Records |
|---|---|---|
| `Colony_Hangok` | 0x8ed140 | 58 |
| `Gr_Hangok` (ground war) | 0x8f51f0 | 44 |
| `Des_Hangok` (design) | 0x902a68 | 4 |
| `Dip_Hangok` (diplomacy) | 0x903008 | 3 |
| `Mainscr_Hangok` | 0x9140e8 | 6 |
| `Mes_Hangok` (messages) | 0x914950 | 19 |
| `Res_Hangok` (research) | 0x918c88 | 4 |
| `Global_Hangok` (`PlaySoundGlobal`) | 0x919a60 | 11 |
| `SpWar_Hangok` (space war) | 0x91a838 | 42 |
| `Spy_Hangok` | 0x926b90 | 3 |
| `Trade_Hangok` | 0x928af0 | 2 |

The names come from the user's binaries: `sound_tables.py --names` prints them (keep the output under `_local/`).
In the GOG install a name `dir/Name.wav` is the file `dataraw/sounds/dir/name.ogg` (lower case, `.ogg`, in the
remaster's `neGs` container like the other `dataraw` files); every record has its file except `Colony_Hangok[27..36]`
(ten night bird calls, no file in the install). Two records repeat a file (`Colony_Hangok[0]`/`[1]`,
`SpWar_Hangok[37]`/`[38]`). No table names the colony speaker's and the starmap's speech (`colony/speech`,
`starmap/speeches`), the scroll voices or `dataraw/messagesounds` (those go through `PlayWav`/`PlayMessagesHang`).

## 2. The tracer (`sound.js`)
Appended to `screen.js` by `run_screen.py --sounds OUT.jsonl`. It hooks the functions of §1 and writes one record
per call: `{k: 'sound', n, fn, table, idx, global, name, raw, loop, vol, on, ui, frame, t, kep, input, dui}`; `ui` =
the 100 Hz UI counter (exe 0xB772A4), `frame` = drawn frames, `t` = the script's tick (null outside a script), `on` =
`SOUND`, `input` = the last press, key or pointer move before it and `dui` the UI ticks since (meaningful for script input; outside a script it may be a stale start-up key). Input records
`{k: 'input'}` come from the game's SDL2 event filter `MainCallBackEventHandler` (exe 0x4e2db0; real events) and from
the script's injected ones (Frida runs none of our hooks inside our own `NativeFunction` calls, so the injector reports
them itself). `run_screendiff.py --script` turns the log into the (input → sound) table.

## 3. Input scripts
One JSON file drives both sides (`tools/trace/scripts/*.json`; `ig2-client/src/script.rs` documents the format):
`{events: [{t, mouse: [x, y]} | {t, press: [x, y], button?, hold?} | {t, key: NAME, hold?}], grabs: [t…], tail?}`.
`t` counts UI ticks (10 ms) from the first frame the screen is up (`AktKep` = the screen); points are virtual
(640 × 480); a press is the pointer moved there, the button down and up `hold` (3) ticks later; key names: down,
up, right, left, enter, pageup, pagedown, escape, d, k, space.

The exe side (`screen.js`, `run_screen.py --script`):
- **The clock**: `Fake_FRISSITES` (exe 0x4de720) runs one UI tick per 10 ms of `SDL_GetTicks` (exe 0x689e80):
  `while (ticksNow − uiStamp > 10) { UI++ (0xB772A4); uiStamp += 10; DoKeyboard(); … FRISSIT_<screen>2 … }`
  (`ticksNow` 0xBAE270, `uiStamp` 0xBAE278). From the script's first frame `SDL_GetTicks` returns a clock that moves
  10 ms per `Fake_FRISSITES` (`uiStamp` set one below it), so every drawn frame runs exactly one UI tick, whatever the
  drawing speed: the run is deterministic and the 20 Hz sim stays paused.
- **Events at t** go in at the entry of the first frame whose tick count is ≥ t (before the next tick) through
  `MainCallBackEventHandler`, as SDL2 events: 0x401/0x402 button down/up (button +0xc, state +0xd, clicks +0xe,
  window x/y +0x10/+0x14; the handler maps them to virtual pixels with the virtual frame 0xB770D0..0xB770DC = w, h,
  x0, y0, so the script's point goes in as `x0 + ⌈x·w/640⌉`); 0x300/0x301 key down/up (keysym at +0x10: scancode,
  +0x14 sym; `remapKeyCodes` exe 0x4e4670 maps it into the key queue 0xB773A8, kinds 0xB77394, 20 entries).
  The pointer: `Mouse_Akt` (0x916008/0x91600C) and `Mouse_x/y` (0xDB4654/0xDB4658) written every frame, as
  `screen.js` already does.
- **Grabs**: the first `SwapBuffers` (gdi32) after exactly t ticks is read back in the game with `glReadPixels` (the
  default framebuffer's back buffer; the read framebuffer, pack buffer, read buffer and pack alignment put back after)
  → `<prefix>-t<T>.png` and `<prefix>-frames.json` (per grab: ticks, drawn frames, the swap's thread, the screen's
  state; the research screen's `resframe`, open/closing, `ResearchTerulet`/`Mezo`/`Talalmany`). No window grab: the
  frame is the one the game presented, at the tick it was drawn.
- `--state-out` saves the script's first frame (`LoadSave(slot, name, 2)`, before any event) with its UI clocks.

The client side (`ig2-client --snapshot OUT.png --save STATE --screen research --script S`): the screen opened
animated on that save, then per tick: the tick, the grab (`OUT-t<T>.png`, drawn after t ticks), then the events at t
(the same order as the exe's: a grab at t shows the state before t's events). Presses go to the screen's press handler,
keys to its keyboard handler, commands through `World::send`/`pump` as the exe's `MULTIGET_*`.

## 4. The research screen's sounds (observed, `research.json`, 2026-10-22)

| Action | Sound | When |
|---|---|---|
| the screen opens (`Research_Belep`) | `Res_Hangok[0]` | at the entry, before the script's first frame |
| a press on the urgent button (507, 50) | `Global_Hangok[0]` (`PlaySoundGlobal(0)`) | the next tick |
| ↓ (the next field) | `Res_Hangok[1]` | 10 ticks after the key down |
| PgDn (a category), D (urgent by key) | none | |

The sequence diff of the same run (diff-results.md, 2026-10-22): the exe's `resframe` is 0.4·t from t = 1 (4.0 at 10,
60 and open at 150); the client's trails it by one tick. `Res_Hangok[3]` is the closing's (`Research_Kilep`, research.md §5.1); `Res_Hangok[2]` wasn't heard. Esc on the
research screen opens screen 13 at once (no closing animation, no sound), and a press on the top bar's research spot
(620, 10) does nothing in the exe: `re/deviations.md`.
