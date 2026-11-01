# ADR 0047: play in the browser

Date: 2026-11-01. Status: accepted; part 1 implemented (`ig2-formats::vfs::Files`, the CI job "Browser target check").

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
4. **Renderer and window.** wgpu with the WebGPU backend, WebGL2 as the fallback (wgpu's downlevel limits; what the
   renderer needs beyond them is found in part 2); winit's web backend with an async main loop (`spawn_local` instead of
   `pollster::block_on`, the event loop owned by the browser). Frame timing uses `web-time` behind the same names.
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
    -p ig2-rules -p ig2-sim -D warnings`, so the shared crates keep building for the target; each later part adds its
    crates to that job.

## Order of the parts
1. **Shared crates build for wasm32** (done): `Files` behind `Install`, the CI check.
2. **Main menu in a page**: `ig2-client` builds for the target behind a `web` cfg (wgpu WebGPU/WebGL2, winit canvas,
   async start, `web-time`), the folder picker and the OPFS store, settings in OPFS, cpal WebAudio; done = open the
   page, pick the install, the main menu shows and plays its music.
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
