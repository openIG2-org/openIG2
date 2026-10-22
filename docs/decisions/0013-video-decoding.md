# ADR 0013: Video decoding

Date: 2026-10-22. Status: accepted. Builds on ADR 0001 (assets from the user's install), ADR 0002 (the
`stable-x86_64-pc-windows-gnu` toolchain, no MSVC) and ADR 0006 (the client). Spec: `re/screens/video.md`;
implemented by `ig2_formats::mp4` and `ig2-client::video`. Closes the decoder question of U30.

## Context
The remaster plays the 1999 code's `.bik` names from `anim/mpeg4/*.mp4` (423 files in the GOG install): ISO-BMFF
inside the `neGs` container (`re/formats/ecgs-container.md`), H.264 **High** profile (CABAC, B-frames, 8×8
transforms, implicit weighted bi-prediction), 25 fps, 640×400 for the cut-scenes and 332×188 for the message,
race-choice and story-panel clips, AAC-LC 44.1 kHz stereo on the cut-scenes only. The options:
- a pure-Rust H.264 decoder: the crates on crates.io in 2026 are young and claim "most of High profile";
- the OS's media framework (Media Foundation): a system library, Windows only, no bit-exact reference;
- FFmpeg: a system library or a large C build;
- OpenH264 (Cisco, BSD-2): C++ built from source by the `openh264` crate's `openh264-sys2` with the toolchain's
  g++ and nasm (WinLibs, already on PATH for `ring`, ADR 0005), no system library.

Measured on the install (`vproto`, 2026-10-22): OpenH264 2.6 decodes every sample of the High-profile clips; on
`arriving.mp4` 271 of 326 frames are bit-identical to FFmpeg's decode and the rest differ by rounding only (worst
frame 65.5 dB PSNR); 326 frames in 1.24 s on one thread (≈10× real time). The `openh264` crate's default "flush
after every decode" breaks B-frame streams (`dsOutOfMemory` from `PrefetchPic` from the 5th frame on): it must run
with `Flush::NoFlush` and `flush_remaining` at the end.

## Decision
1. **Demux** in `ig2_formats::mp4` (pure Rust, our own: the `mp4` crate rejects these files' sample entries): the
   `moov` sample tables into per-track sample lists and the `avcC` / `esds` configuration.
2. **Video**: OpenH264 from source (`openh264` 0.9, feature `source`), no flush per frame. YUV → RGB on the CPU
   with the remaster's own `video_fragment.shader` arithmetic (BT.601 limited range, its exact constants and
   8-bit-to-float mapping), so the 2D pass draws the result like the remaster's video quad.
3. **Audio**: Symphonia's pure-Rust AAC decoder (`symphonia` 0.6, feature `aac` only), resampled with the audio
   module's resampler into a stream of the mixer's own (`Mixer::play_video`).
4. Nothing is cached or shipped: files are read and decrypted from the user's install at play time.

## Consequences
- The client's build needs a C++ compiler (g++ from WinLibs on Windows) besides gcc; documented in `tools/README.md`
  next to `ring`'s gcc.
- H.264 is patent-encumbered in some countries. Cisco's royalty coverage applies only to its own prebuilt binaries,
  not to a build from source; the remake decodes the user's own files of a game the user owns and ships no codec
  binary of its own. The licence question of the project stays open (`publish = false`); revisit before any
  distribution (the `libloading` feature of `openh264` can load Cisco's binary at run time instead).
- If a pure-Rust decoder matures, the swap is local to `video::decode` (the frame and sound interfaces stay).
