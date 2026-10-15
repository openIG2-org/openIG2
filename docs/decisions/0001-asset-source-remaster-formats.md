# ADR 0001: Load assets from the GOG remaster formats

Date: 2026-10-15. Status: accepted.

## Context
The GOG release is the Gyroscope remaster. Its assets are re-encoded (PVR textures, Ogg, MP4) and
wrapped in the `ecGs`/`neGs` container. The original 1999 CD formats (8-bit sprites, Bink video)
aren't in the install. Users own the GOG build.

## Decision
- `ig2-formats` reads the remaster layout directly from the user's install: the container,
  then the payload format. The keys are derived at runtime by the recovered recipe; no key
  material or assets ship.
- Path resolution mirrors the game's `AssetHelper` rules: `localization/<lang>/<path>` first, then
  `<path>`; case-insensitive.
- Original 1999 formats (`.xv`, `.scn`, colony maps, `.ani`, text) are read as-is from inside the
  container. They're the same formats the 1999 engine used.
- Support for a 1999 CD install is deferred. If added later, it gets its own VFS backend behind the
  same `Install` API.

## Consequences
- The engine works with the only legally obtainable current release.
- The 1999 Bink cutscenes aren't needed; MP4 decoding is (Phase 3 video ADR).
