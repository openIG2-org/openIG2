# ADR 0047: play in the browser

Date: 2026-11-01. Status: accepted; part 1 implemented (`ig2-formats::vfs::Files`, the CI job "Browser target check");
part 2 implemented (the client in a page up to the main menu with its music, `web/`, `tools/build_web.py`); part 3
implemented but for video (single player on WebGPU or WebGL2, the player's files in OPFS, `tools/serve_web.py`);
part 4 implemented (a browser joins a native host over WebRTC through the rendezvous service, videos through
WebCodecs; a browser hosts a room there too, STUN from the service itself, 2026-11-13). Hosting a public page (part 5)
waits for the maintainers' decision.

## Context
Opening a URL and playing needs no install step and runs on any machine with a current browser. The engine ships no
game data, and a web page must not either: the files have to come from the player's own install, on the
player's machine. Browsers can do that: the File System Access API (Chromium) or a folder `<input webkitdirectory>`
(every browser) hands the page the install folder, and the Origin Private File System (OPFS) keeps a copy for the
next visit. Rust builds for `wasm32-unknown-unknown`; wgpu renders through WebGPU or WebGL2, winit runs in a canvas,
cpal plays through WebAudio. What does not carry over: `std::fs`, `std::thread`, `std::time::Instant` (it panics on
that target), blocking sockets, `openh264`'s C code, `arboard`, QUIC over UDP.

## Decision
1. **Game data stays the player's.** The page never uploads or hosts a game file (a server may offer its own copy
   with the player's agreement: ADR 0081). The player picks the install folder
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
  (Since ADR 0081's language rule: the page's language choice goes to the client, and the other languages'
  `localization/` copies are not loaded.)
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

## Part 4 as built
- **The transport** is `ig2_net::webrtc`: one reliable, ordered data channel per link (not unreliable-ordered as §9
  planned: the lockstep messages need every frame, and QUIC's links are reliable streams too), messages in 16 KiB
  pieces each led by a "more follow" byte, behind the same `Transport` trait as QUIC. Natively (feature `webrtc`)
  `str0m` with its pure-Rust crypto drives the channel on a UDP socket of its own (host candidates for 127.0.0.1 and
  the LAN address); in the browser it is the page's `RTCPeerConnection`. The password binding (ADR 0008 §3.1) is a
  SHA-256 over both ends' DTLS fingerprints as the offer and the answer carry them, so a signalling server that
  swapped them fails the password check. Each end probes the round trip every 500 ms (lead bytes 2 and its echo 3,
  answered where the channel lives, between a message's pieces) for `Transport::rtt`, so the input delay adapts as
  over QUIC (ADR 0063). The native driver is woken by an empty datagram to its own socket for each message (a socket
  timeout of 4 ms waits ~16 ms on Windows: a round trip took ~100 ms) and its socket takes 4 MiB (a big message's
  burst overflowed Windows' 64 KiB default and a lost packet waited out SCTP's 1 s retransmit).
- **A window that draws nothing** (minimized, a hidden or covered browser tab: no animation frames) still runs a
  multiplayer game on from the event loop's idle callback once 100 ms passed without a frame, as under a video;
  otherwise every other player waits for its input behind the "Slow connection!" overlay.
- **Signalling** goes through the rendezvous service (ADR 0018), which grows two packets (15 Offer, 16 Answer: an offer
  id and an SDP of at most 4000 bytes) and an HTTP side on the same port number over TCP: the page POSTs its offer
  (no trickling: it waits for its candidates) to `/offer/{session}/{pin}` with the join code's session and pin; the
  service looks the room up as for Lookup and passes the offer to the room's host on its registered QUIC socket; the
  host answers there, and the service returns the answer as the HTTP reply (404 no such room, 429 a limit, 504 no
  answer in 8 s). Offers count against the control-packet limit, and a room holds as many waiting offers as it takes
  peers. CORS is open (`*`): the join code is the key, as for Lookup.
- **The client**: a native host with a relay server set answers browser offers beside its QUIC joiners (the lobby's
  `Listener::accept` admits a browser once its channel opens), so a browser and a native player meet in the same
  room and no room needs a transport flag. The browser's `WebNet` joins by join code through the relay server typed
  on the connection page (kept in `relay.txt` in the page's storage); it cannot host or search the LAN.
- **Video**: `video::web` feeds the `ig2_formats::mp4` samples as Annex B access units (the parameter sets before each
  key frame) to WebCodecs' `VideoDecoder` through a few functions of the page (`ig2_video_*`), and draws each frame
  through a canvas into RGBA for the player's channel. The YUV to RGB conversion is the browser's, not the remaster
  shader's arithmetic the native decoder copies. The page now hands the videos to the store too (about 300 MB more).
- **Tests**: `ig2-net/tests/webrtc.rs` runs the service on 127.0.0.1, a native offerer (`webrtc::connect`, the
  browser's HTTP path) against a relayed `QuicHost`, and two such peers in lockstep with identical hashes.
- **Testing in the pane**: `ig2-client --mp-host` hosts a room without a window (the connection page's own code, the
  relay server of `relay.txt`, its join code printed; it starts once the joiners are ready and prints the turns and
  any desync), and the page's `?args=` (localhost only) passes the native client's options, `--multiplayer` opening
  on the connection page. Verified: the title video plays; a browser joined `--mp-host` through a local service and
  the two played about 960 lockstep turns with the game clock running.
- **One divergence at the start**: the browser's first turn hashed differently from the Windows host's (native pairs
  agree from turn 0); the session resynchronised it from turn 2 (ADR 0021) and every later turn agreed. Likely
  cause: the sim's `sin`, `cos`, `acos`, `ln`, `exp`, `powf` come from each platform's maths library, and the galaxy's
  start uses them; a portable implementation for the sim (the `libm` crate) would make every platform agree, so the
  Consequences' "byte-identical" holds on one platform only until then.

## Part 4 continued: a browser hosts
A page takes no connections, so the service stands in for its socket (ADR 0018's relay is untouched):
- **The room**: the page long-polls `POST /host/{session}/{pin}/{key}` (16, 24 and 32 hex digits): the first poll
  opens a room for the session as Register does, each later one keeps it open, and the body is its encoded public
  listing (empty: unlisted). `key` is the page's own random secret, never in a join code: only the page that opened
  the room polls it and answers its offers (another key: 409; full: 429). The service holds the poll up to 8 s and
  answers with the page's address as it sees it, then an offer's id and SDP if a joiner's came. The page answers with
  `POST /answer/{session}/{key}/{id}` and the joiner's own `/offer/` request returns that answer. A page's room has no
  socket, so a QUIC Lookup of it is refused, a Register can't take its session, and its listing has port 0, which
  `Listing::join_code` turns into a code without an address.
- **The page** (`webrtc::PageHost`, the client's `WebListener`): a random 32-byte fingerprint stands in for the
  certificate's (the join code's pin is its start); each offer gets an `RTCPeerConnection` of its own, the channel's
  handlers are set as `ondatachannel` brings it (before its first message can come), and the lobby admits the link
  once open. The room shows a join code once the service answered, its seen address without a port.
- **Joiners**: a browser offers as before. A desktop joiner tries QUIC through the service first; when that fails and
  the code carries no address it offers over HTTP signalling to the same service (`webrtc::connect`), so desktop and
  browser players meet in a browser's room.
- **STUN, the project's choice**: the service answers STUN Binding requests (RFC 8489, XOR-MAPPED-ADDRESS, never
  larger than the request, counted against the control limit) on its UDP port, and every WebRTC end asks it: pages
  through `RTCPeerConnection`'s `iceServers` (`stun:` the relay's host and port), native ends (str0m) with one request
  from their link's socket before the offer or answer (300 ms at most), added as a server-reflexive candidate. No
  third party's STUN server learns the players' addresses, and the relay a player already trusts is the only server
  involved. There is no TURN: two players behind symmetric NATs still can't reach each other over WebRTC.
- **Tests**: `rendezvous` unit tests for the page room (keys, offers handed once, expiry, listing, limits, paths and
  replies), the STUN answer against RFC 5769's sample, and `ig2-net/tests/webrtc.rs`'s page host polling a running
  service over HTTP and answering a native joiner. Verified live: a page hosted through a local service, its room
  showed a join code, `ig2-client --mp-join` joined it and the two played in lockstep.
- **Known limit**: a page's game runs in its animation frames; a hidden or minimised tab stops them, and its joiners
  then take the host as gone (as for a browser joiner's own tab).

## What is left
- **A public page and an HTTPS service:** a page served over HTTPS can only reach an HTTPS signalling endpoint, and
  publishing the page is the maintainers' call (part 5). Until then the page runs from `tools/serve_web.py` on this
  machine against a service the player runs.
- **NAT traversal beyond STUN:** every end gets its public address from the service's STUN answers (above); players
  behind symmetric NATs would need a TURN-like relay over WebSocket.
- **Clipboard, mods, memory:** the web clipboard for join codes (typed for now); mods in OPFS; reading the install on
  demand from a worker instead of holding it in memory (now about 700 MB with the videos).
- **Cross-platform determinism:** the sim's transcendental functions from one portable implementation (above).
- **Audio:** works; speech and effects are untested beyond the menu.
- **Hosting (part 5)** as below.

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
