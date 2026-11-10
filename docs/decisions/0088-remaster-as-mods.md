# ADR 0088: the remaster ships as mods; a mod's `visuals.toml`

Date: 2026-11-10. Status: accepted. Builds on ADR 0044 and 0068 (mods), ADR 0054 (texture and model packs), ADR 0055
(modern lighting), ADR 0057 (battle effects), ADR 0064 (smooth glows) and ADR 0086 (the mod repository). Implemented
by `ig2-formats::visuals` (the file), `ig2-formats::vfs` (found in a mod), `ig2-client::visuals` (laid over the
settings) and `ig2-client::starmap` (the planets' halo).

## Context
The game is to get a remastered look: better lights and effects, a richer starmap, sharper textures, smooth
animations in place of the frame-by-frame sprites, more detailed models; true to the original's shapes, colours and
timing. The classic look must stay exactly the original's, and the project ships no game files.

A mod holds data only (ADR 0086: no programs, no scripts), so a mod cannot bring a shader or an effect of its own.

## Decision
1. **Nothing is replaced in the game; the remaster is a set of mods** the player installs from the Mods page's
   Browse tab (ADR 0086) or by hand. Without them the game looks as before.
2. **The engine gains abilities, the mods switch them on.** Each new effect is engine code that does nothing by
   default. A mod turns it on and sets its values in `visuals.toml`; pictures and models come as a `hires/` pack
   (ADR 0054).
3. **`visuals.toml`** at a mod's top (not a game path):

   ```toml
   [lighting]
   anti_aliasing = true     # ADR 0055's four switches
   per_pixel = true
   soft_shadows = true
   bloom = true

   [effects]
   battle = true            # ADR 0057
   smooth_glows = true      # ADR 0064

   [starmap]
   atmosphere = 0.6             # the planets' halo: strength 0..1
   atmosphere_size = 1.35       # its radius in planet radii, 1..3
   atmosphere_color = "#7fb2ff"
   ```

   - A switch set `true` is on while the mod is enabled, whatever the player's own setting; a switch left out (or
     `false`) leaves the player's setting alone. A mod never switches anything off.
   - With several mods a switch on in any is on; a value comes from the highest-priority mod that sets it.
   - An unknown table or key, or a value out of range, is an error: the file is skipped and named in the log, on
     the Mods page (the mod's problems) and by `openIG2-check` (also in `--package`).
   - The file is display only: it is never in the multiplayer fingerprint, the save or the lockstep, whatever the
     mod's `kind`. Players with and without it play together.
4. **The planets' halo** (the first new ability): on the starmap each drawn planet gets an additive glow under its
   mesh, full at the planet's limb and fading to `atmosphere_size` radii, in `atmosphere_color` × `atmosphere`;
   it fades with the planets behind a battle. Halos under 1.5 pixels are not drawn.
5. **The remaster's packages**, each a row of `docs/ENHANCEMENTS.md` (562–569), in this order:
   1. Look switches and effects in the engine (562–566): this file, HDR with tone mapping and bloom over the whole
      scene, per-pixel lights and shadows on every 3D screen, a richer starmap, effect definitions as data.
   2. Textures (567): a tool that makes a `hires/` pack from the player's own install on their machine.
   3. Sprite animations (568): the frame-by-frame effects as smooth particle effects, the rest interpolated.
   4. Models (569): more detailed models as `hires/` packs.
6. **What the index can carry.** A mod in the index holds no file copied or derived from the install (ADR 0086).
   Look files, newly made pictures, effects and models can be downloaded; a pack upscaled from the original's
   textures cannot, so the tool of 567 makes it on the player's machine.
7. **Fidelity rule for every remaster mod**: outlines, palette, timing and layout stay the original's; resolution,
   light and detail may change.

## Consequences
- The classic look and every capture comparison are untouched: with no mod enabled no code path changes a pixel.
- The remaster can grow without releases of the game once an ability exists, and players can make their own looks.
- Each new ability costs a `visuals.toml` key and its check; shaders stay in the engine.
- Rejected: shaders inside mods (code from a download, and three back ends to keep working); a built-in "Remaster"
  switch that replaces the classic drawing (the remaster is to be optional content, not the game's default).
