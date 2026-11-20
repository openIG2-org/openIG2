# ADR 0091: standard sound, music and font formats for mods; new entries' own sounds and pictures

Date: 2026-11-20. Status: **accepted** (2026-11-20). Extends ADR 0044 (file overrides), ADR 0069 (new techs, buildings,
hulls and weapons) and ADR 0086 §6 (rows 556 and 560). The packages are rows 575 (sound and music formats), 556 (looks
and sounds of new entries), 576 (fonts) and 560 (languages) in `docs/ENHANCEMENTS.md`.

## Context

Every sound, voice and music track of the GOG install is Ogg Vorbis (`re/screens/audio.md` §1). A mod could replace one
only with another Ogg Vorbis file at the same path, and an author's tools mostly write WAV or FLAC. A mod's new
building or weapon (ADR 0069) looks and sounds like the original it is `like`, and an added space weapon fired in
silence: `lovedeksound` (exe 0x57adb0) knows the exe's 40 weapons only. The music has 19 fixed tracks
(`ZeneNevek`, exe 0x91a698); a mod could replace them but not add any. The game draws no 2D icon of a tech, building,
hull or weapon: every list picture is a model rendered live (the build menu's preview model, the research screen's
model, the design screen's hull and part models).

The client already has `lewton` (Vorbis) and `symphonia` (the videos' AAC, MPL-2.0), both pure Rust and built for the
browser too; symphonia's FLAC and WAV parts are features of the same crate.

## Decision

### 1. Sound and music files may be Ogg Vorbis, FLAC or WAV

- **Where.** A mod replaces a sound, voice or music track by a file at the game path of the original
  (`dataraw/sounds/colony/click.ogg`, `sounds/colony/click.ogg`, `mfx/pol1.ogg`, `dataraw/messagesounds/1/0011_01.ogg`,
  the localised copies) with the extension `.ogg`, `.flac` or `.wav`. New files (an added entry's sound, §2; music, §2)
  live under paths of the mod's own (`sounds/<mod>/…`, `music/…`).
- **Which wins** (`Install::resolve_sound`): the game's names for one sound (the `dataraw/` and plain copies, the
  localised and the plain one) are taken together. Any mod's file under any of them beats the install's; among the
  mods the highest priority (the Mods page's order) wins; within one mod the game's order of names, then the
  localised copy, then `.ogg` before `.flac` before `.wav`. A mod's `sounds/colony/click.wav` thus replaces the
  install's `dataraw/sounds/colony/click.ogg`.
- **Decoding** (`ig2_formats::sound`): the file's first bytes decide the decoder, never its extension (`OggS`,
  `fLaC`, `RIFF…WAVE`). Ogg Vorbis goes through `lewton` exactly as before, so the install's sounds decode
  byte-exact; FLAC (any bit depth) and WAV (integer and float PCM; no compressed WAV) through `symphonia`. Mono plays
  on both sides; 1 or 2 channels; 8000 to 192000 Hz, resampled linearly to the mixer's rate as the originals are.
- **Limits.** Effects, voices and speech are decoded whole: after 120 seconds the rest is ignored. Music is streamed
  and has no length limit. The zip rules of ADR 0068 cap a file at 1 GiB.
- **Malformed files** are the mod's problem, never a crash: a file that doesn't open or decode plays nothing (a
  broken packet ends a music track early), the game goes on, and `openIG2-check --mod` names it. `--mod` decodes each
  mod sound file whole and notes its kind, rate, channels and length, and a sound over the 120 seconds.
- `flac` and `wav` join the kinds a mod zip may hold.

### 2. New entries' own sounds and pictures; mod music (row 556)

**Sounds.** The game plays an entry's own sound in two places, and those take the mod's file:

```toml
[[buildings.new]]
key = "fusion_reactor"
like = 55
sound = "fusion/start.wav"   # relative to sounds/ as the game's own tables name theirs: the mod's sounds/fusion/start.wav
icon = "fusion_prev.xv"      # the build menu's picture (the preview model; `preview` is the same field)

[[space_weapons.new]]
key = "plasma_lance"
like = 3
sound = "fusion/lance.flac"  # its firing sound
```

- A building's `sound` plays when the player starts building it (`Build-Building1`, exe 0x4a88a0); without it, the
  like's (one of the two building sounds, drawn as the original draws them).
- A weapon's `sound` is its firing sound at the like's volume; without it, the like's firing sound (before, none).
- The files are appended as new records after the exe's `Colony_Hangok` (58) and `SpWar_Hangok` (42), in the rules'
  order, at 100 per cent: they play through the same three voices per record, volumes and options as every effect.
- The value is a path relative to `sounds/`, `.ogg`, `.flac` or `.wav` (§1 picks the file), without `..`, a drive or a
  leading `/`; anything else fails the load naming the entry. `--mod` refuses a `sound` no mod or the install has.
- Techs and hulls have no sound of their own in the game (the design screen's sounds are the same for every hull):
  `sound` there fails the load with that reason, so a modder isn't misled.

**Pictures.** `icon` names the picture a list shows: for a building the build menu's preview model (the existing
`preview` field, ADR 0069; `icon` is its name in this ADR's terms). A tech's picture is its research-screen `model`
(ADR 0069). A new hull's and weapon's own model (the design screen's hull model, the battle's projectile form) is
not open yet: both index tables the battle's projectile pools and the design screen share, and the standard model
format comes first (glTF, ADR 0090). `model` or `icon` on a hull or weapon fails the load naming that. Models are the
game's own `.xv` format until ADR 0090's formats land.

**Music.** Every sound file under a mod's `music/` joins the music: `music/ground_war/` and `music/space_war/` to the
ground and space battle picks, the rest to the in-game rotation (one track per name whatever its kinds). The tracks
are numbered after the exe's 19 in path order. The in-game rotation plays the race's three tracks as the original
does (`PlayMusicMain`, exe 0x5407c0), then each mod track in turn, then the race's first again; a battle picks
among its three and the mods' (`13 + rand() % 3` becomes `% (3 + n)`). Without mod tracks every pick is the
original's draw for draw. The menu track is replaced, not added to (`mfx/menu.ogg`).

### 3. Presentation only: what is and is not hashed

- **Sim state and saves**: none of this is game state. The sound tables, the music list and the picture names live
  in the client; `AddedBuilding::sound` and `AddedItem::sound` sit in the rules (`ig2-rules`), which the sim reads
  but neither hashes per tick nor writes into a save. A save played with a sound mod loads without it.
- **The multiplayer fingerprint** (`mods_fingerprint`): sound and music files (`.ogg`, `.flac`, `.wav`, `.mp2` by
  name) are left out, as a pack's pictures and models are (ADR 0054 §7): they are only heard. A room's players may
  use different sound and music mods. `rules.toml` stays hashed whole (ADR 0044 §9), so the `sound` and `icon`
  fields in it count with the rest of the file: two players whose added entries differ only in a sound name are told
  their mods differ. A mod that holds only sounds and music should say `kind = "visual"` in its `mod.toml` (ADR 0086
  §2), so the room's mod list (§3 there) leaves it out too.
- **Lockstep hash and replays**: unchanged; the cosmetic random streams the client draws for sounds and music are
  its own (`re/deviations.md`), and without mod tracks they draw as before.

### 4. Fonts (row 576), planned

A mod's TrueType or OpenType font (`.ttf`, `.otf`, already a kind a mod zip may hold) in place of the game's bitmap
font, with its code page coverage, sizes and the original's metrics as the fallback. Its own ADR section here when it
is built.

### 5. Languages (row 560), planned

A mod adds a whole language: every text file, a choice in the options, the speech falling back to English, and the
glyphs from §4. Its own section here when it is built.

## Consequences

- Authors save their sounds and music as WAV or FLAC and drop them in; nothing needs converting to Ogg Vorbis.
- Added buildings and weapons can sound like themselves, and an added weapon is no longer silent.
- Players in one room may run different sound packs.
- The install's audio decodes as before (the install tests' sample counts are unchanged).
- Not done: a hull's and weapon's own model, a tech's own sound; menu music added rather than replaced; WAV's
  compressed codecs (ADPCM, µ-law) and more than two channels are refused.
- Rejected: a separate decoder crate per format (`hound`, `claxon`): symphonia is already in the tree; telling the
  format by extension (a renamed file would fail for no reason); hashing sound files into the fingerprint (they
  don't change the game).
