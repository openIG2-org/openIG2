# ADR 0047: play in the browser

Date: 2026-11-01. Status: accepted; part 1 implemented (`ig2-formats::vfs::Files`, the CI job "Browser target check");
part 2 implemented (the client in a page up to the main menu with its music, `web/`, `tools/build_web.py`); part 3
implemented but for video (single player on WebGPU or WebGL2, the player's files in OPFS, `tools/serve_web.py`).

## Context
Opening a URL and playing needs no install step and runs on any machine with a current browser. The engine ships no
game data, and a web page must not either: the files have to come from the player's own install, on the
player's machine. Browsers can do that: the File System Access API (Chromium) or a folder `<input webkitdirectory>`
(every browser) hands the page the install folder, and the Origin Private File System (OPFS) keeps a copy for the
next visit. Rust builds for `wasm32-unknown-unknown`; wgpu renders through WebGPU or WebGL2, winit runs in a canvas,
cpal plays through WebAudio. What does not carry over: `std::fs`, `std::thread`, `std::time::Instant` (it panics on
that target), blocking sockets, `openh264`'s C code, `arboard`, QUIC over UDP.

## Decision
1. **Game data stays the player's.** The page never uploads or hosts a game file. The player picks the install folder
   once; the page copies the install's files into OPFS and opens them from there on later
   visits. The install check (`ig2-rules::install::check`, today a folder walk) moves onto `Files` in part 2.
2. **Files behind one trait.** `ig2_formats::vfs::Files` (scan a root, read a file, open a seekable stream) is the
   only way `Install` touches storage; `Disk` is the native one and the default, so native behaviour, tests and
   replays are unchanged. The browser build adds an OPFS store: a worker's synchronous access handles serve `read`
   and `open`, the scan is the copy's file list. Every reader of the exe and the game files goes through `Install`
   (`read_raw("ig2.exe")` instead of `root().join("ig2.exe")`); the client's remaining direct reads move over in
   part 2.
3. **Settings and saves.** On that target `std::fs` and `std::env` return "unsupported", which the settings, locale and
   install search already treat as "nothing there" (defaults, no automatic install search), so they need no cfg.
   Part 2 gives options, saves, the mods list and replays a small storage interface with an OPFS implementation.
4. **Renderer and window.** wgpu with the WebGPU backend; winit's web backend with an async start (the adapter and
   device requested with `spawn_local` instead of `pollster::block_on`, the event loop owned by the browser through
   `spawn_app`). Frame timing uses `web-time`, a re-export of `std::time` natively. WebGL2 is **not** a fallback yet: the
   renderer's gamma-space passes view every sRGB texture and the surface also as linear (wgpu's `VIEW_FORMATS` and
   `SURFACE_VIEW_FORMATS`), which WebGL2 cannot; a WebGL2 path needs linear copies of the 2D textures or the sRGB
   decode moved into the shaders; part 3 takes the second way (below). The 2D shader turns off WGSL's derivative
   uniformity check (`diagnostic(off, derivative_uniformity)`), which browsers enforce and naga does not.
5. **Audio.** cpal's WebAudio backend; the mixer runs on the audio callback as on native. Decoding stays in Rust
   (lewton, symphonia). Browsers start audio only after a click: the first click on the main menu resumes it.
6. **Video.** `openh264` does not build for the target; the browser build decodes the remaster's MP4s with WebCodecs
   (`VideoDecoder`) fed from the same demuxer (`ig2_formats::mp4`) and uploads frames as textures. Native keeps
   openh264.
7. **Threads.** The sim is single-threaded (`ig2-sim` uses no thread, clock or file) and stays so. Work the client
   puts on threads (the video decoder, the lobby's network loop) becomes async tasks on the page or a Web Worker.
   No `SharedArrayBuffer` requirement, so the page needs no cross-origin isolation headers.
8. **Clipboard.** The async web clipboard (`navigator.clipboard`) instead of arboard, for join codes.
9. **Multiplayer.** Browsers cannot open raw UDP or QUIC sockets. The lockstep protocol (ADR 0007) is unchanged; the
   transport becomes WebRTC data channels (peer to peer, unreliable-ordered for commands) with the rendezvous service
   (ADR 0018) as the signalling server, and a relay over WebTransport or WebSocket when no direct path works. A
   browser and a native player meet in the same room when the native client speaks the same transport; until then
   browser rooms are browser-only (the room's terms carry the transport).
10. **Checked in CI.** The job "Browser target check" runs `cargo clippy --target wasm32-unknown-unknown -p ig2-formats
    -p ig2-rules -p ig2-sim -p ig2-net -p ig2-client -D warnings`, so the crates keep building for the target.

## Part 2 as built
- The page (`web/index.html`, `web/main.js`, our own code only) takes the install folder through
  `showDirectoryPicker` or a `webkitdirectory` input, checks for `ig2.exe` and `DataRaw/Text/Scenario.txt`, copies
  every file into OPFS (`install/`, a marker file written last) and asks for persistent storage; later visits start
  from that copy. It then hands every file but the videos to the client (`web::add_file`) and calls `web::start`.
- The client's store is in memory (`web::Store`, about 400 MB): the main thread cannot read OPFS synchronously, and a
  worker with synchronous access handles (§2) is the change once memory matters or the videos come in.
- Behind `cfg(target_arch = "wasm32")`: the install root and store (`install_root`, `open_install`), the QUIC lobby
  (`mp_lobby::ClientNet` is `OfflineNet`: hosting reaches nobody, joins fail), the H.264 decoder (`Frames` fails, so the
  title and cut-scenes are off), the clipboard (none). Settings, mods and saves read `std::fs`, which answers
  "unsupported" there: defaults, no mods, no saves. The client's own direct `ig2.exe` reads go through `Install`.
- `cpal`'s WebAudio host plays the mixer as on native; the page's click on Play is the gesture browsers ask for.
- CI's "Browser target check" lints `ig2-net` and `ig2-client` for wasm32 too.

## Part 3 as built
- **The player's files** go through `ig2_formats::userfs`: `std::fs` natively; in the browser a store in memory under
  `/user` (`Options::path` is `/user/options.dat` there, so the slots, the quick save, autosaves, replays, statistics
  and fleet groups sit beside it). The page reads its OPFS folder `user/` into it before the start
  (`web::add_user_file`) and stores every change the client hands back (`ig2_user_file`, written in order); a replay
  is stored when its writer is dropped.
- **WebGL2**: where the adapter has no `DownlevelFlags::VIEW_FORMATS` the window takes a non-sRGB surface format. A frame
  without sRGB is the renderer's switch: textures are stored raw, and the sRGB pipelines (2D `Alpha`/`Additive`, the
  lit mesh pass) use entry points that decode their texels and encode their result (`fs_srgb`, `fs_opaque_e`,
  `fs_glass_e`); the gamma-space passes are unchanged. Blending in those pipelines then happens on encoded values, a
  small difference at translucent edges against WebGPU and native. The browser asks for the adapter's limits
  (WebGL2's are below wgpu's defaults); WebGPU is used where `navigator.gpu` gives an adapter
  (`new_instance_with_webgpu_detection`), the page's `?webgl` forces WebGL2.
- **Testing**: `tools/serve_web.py --install DIR` serves the install on 127.0.0.1 only and the page's `?dev-install`
  (honoured on localhost only) copies it from there without the folder dialog. The client's `eprintln!` lines go to the
  console. The menu's Exit returns the page to its start.

## What is left
- **Video:** WebCodecs (`VideoDecoder`) fed from `ig2_formats::mp4` for the intro, cut-scenes and message clips; until
  then `Frames` fails and they are skipped, as in part 2.
- **Clipboard, mods, memory:** the web clipboard for join codes; mods in OPFS; reading the install on demand from a
  worker instead of holding it in memory (about 400 MB).
- **Audio:** works; speech and effects are untested beyond the menu.
- **Multiplayer (part 4)** and **hosting (part 5)** as below.

## Order of the parts
1. **Shared crates build for wasm32** (done): `Files` behind `Install`, the CI check.
2. **Main menu in a page** (done): `ig2-client` builds for the target (wgpu WebGPU, winit canvas, async start,
   `web-time`), the folder picker and the OPFS copy, cpal WebAudio; the page picks the install, the main menu shows and
   plays its music. Settings in OPFS moved to part 3.
3. **Single player**: saves and replays in OPFS, WebCodecs video for the intro, cutscenes and messages, the web
   clipboard; done = a campaign started, saved, reloaded and its videos played in the page.
4. **Multiplayer**: the WebRTC transport and signalling on the rendezvous service, the WebTransport/WebSocket relay;
   done = two browsers play a skirmish through a join code.
5. **Hosting**: a static page build (the wasm module and loader, no game data) published by the release workflow.

## Consequences
- `Install::open_stream` returns `DecryptingReader<Box<dyn ReadSeek>>` (one dynamic call per buffered read).
- A save, replay or MP room made in the browser is the same format as a native one; the sim's results are
  byte-identical because the sim does not depend on the platform (ADR 0007's determinism rules apply as on native).
- The first visit copies the install into OPFS, which needs the browser's storage quota (it is granted for that size
  on desktop browsers after the page asks for persistent storage).
