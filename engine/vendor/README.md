# Vendored crates

## openh264-sys2 0.9.9

The crates.io release (OpenH264 1.4 sources, BSD-2-Clause, `openh264-sys2/upstream/LICENSE`) with one decoder fix,
wired in through `[patch.crates-io]` in `engine/Cargo.toml`.

`upstream/codec/decoder/core/src/parse_mb_syn_cavlc.cpp`, `CavlcGetLevelVal`: OpenH264 stops at a CAVLC `level_prefix`
of 15 (`MAX_LEVEL_PREFIX`), the Baseline/Main limit. High profile streams may use 16 and above for very large
coefficients (H.264 9.2.2.1: `levelCode = (Min(15, level_prefix) << suffixLength) + level_suffix`, a suffix of
`level_prefix − 3` bits, and `(1 << (level_prefix − 3)) − 4096` more for 16 and above). The game's
`anim/mpeg4/misc/credits.mp4` codes one coefficient of its first picture with 16, so stock OpenH264 lost the first
250 frames. The fix follows the formula and raises `MAX_LEVEL_PREFIX` to 16 (enough for 8-bit video).

Other changes from the release: the crate's tests (`tests/*.rs`, `BINARY_LICENSE.txt`; `reference.txt` stays, `src/lib.rs`
reads it), upstream's `README.md`, `.def`, `.rc` and `.vcproj` files and two `.gitignore` files (which hid
`version_gen.h`) removed; `autotests = false`; one comment in `wels_preprocess.cpp` reworded.

To update: copy the new release over this folder and reapply the `CavlcGetLevelVal` change (or drop the patch once
upstream accepts levels above 15).
