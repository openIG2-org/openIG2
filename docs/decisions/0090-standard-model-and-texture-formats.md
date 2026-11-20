# ADR 0090: standard model and texture formats for mods

Date: 2026-11-20. Status: **accepted** (2026-11-20; models part 1 built). Extends ADR 0044 (file overrides), ADR
0054 §5–7 (`hires/` packs) and ADR 0086 (what to open to mods next). The packages are rows 574 (glTF models), 577
(KTX2/DDS textures) and 578 (the convert tool) in `docs/ENHANCEMENTS.md`.

## Context

A mod replaces a model today only as an `.xv` archive (`re/formats/xv.md`): the 1999 exporter's format, which no
current tool writes. It does so at the game path (ADR 0044 §1–5: the file itself, hashed into the multiplayer
fingerprint) or under `hires/<game path>` (ADR 0054 §6: drawn instead, scaled to the original's size, outside the
fingerprint). Modellers work in Blender and similar tools, whose common exchange format is glTF 2.0.

What the `.xv` format carries, and how much of it glTF can say:

| `.xv` | What the game does with it | glTF 2.0 |
|---|---|---|
| Node tree (`.xvb`): children with a translation and a 3×3 matrix (`p' = M·v + t`), named | `findboxxvb2` finds parts by name; `RotateObj`/`MoveObj` move them | nodes with TRS or a matrix, names: maps exactly |
| Mesh (`.xvo`): vertices (position, normal, one UV), triangles with a material each, ≤ 65 535 faces, `u16` indices | drawn as stored; normals renormalised at load | triangle primitives: maps; more than 65 535 triangles per mesh does not fit |
| Material (`.xvm`): texture, second (self-illumination) texture, transparency `+0x30` | texture by 1999 `.bmp` name, self-illumination drawn additively, `t` → opacity `255 − 255·t` | base colour texture, emissive texture, `alphaMode` BLEND with the base colour's alpha: maps onto the same shading. Metal, roughness, normal and occlusion maps have nothing to drive (no new lighting model) |
| Root half-extents (always 1024, the vertices' largest reach 1) | `AddToScene` sizes the model by its own box (`diag(o/max o)·s`) | the same, by scaling the model (§2) |
| LOD chain (root's `lod_archive`: the next level's model) | `Kocka` picks the level by projected size | no core LOD: a scene `extras` key names the next model |
| Node animation (per-frame keys: matrix, rotation, translation) and vertex animation (per-frame positions) | screens set a frame (`SetFrame`) and blend between keys | animation channels sampled at frames, morph targets as vertex frames: feasible, not built (§4) |
| Wireframe edge mask per face | the wireframe renderer only | none: all edges |

## Decision

### 1. Lookup: a `.glb` stands for the `.xv` of the same stem

A mod's `<game path stem>.glb` (glTF 2.0 binary) stands for the `.xv` at the same path:
`mods/<name>/dataraw/vector/colony/kozepes1.glb` for `dataraw/vector/colony/kozepes1.xv`, and the same under
`hires/` (`mods/<name>/hires/dataraw/vector/…/<stem>.glb`). The file system converts it to that `.xv` when the
`.xv` is read (`Install::read`), so every model loader takes it unchanged. When several mods supply the path, the
higher mod's file wins, whatever its format; within one mod the `.glb` wins over an `.xv` beside it. The install's
own `.xv` always loses to a mod's `.glb`. A `.glb` without an `.xv` in the install adds a model under that name
(useful for the models a mod's new entries name, ADR 0069).

### 2. Axes, units and scale

glTF is right-handed with +Y up and +Z the front; the game's models are right-handed with +Z up
(`re/screens/colony.md` §2.4). The scene's top-level nodes are turned by `(x, y, z) = (X, −Z, Y)` (a rotation:
winding and handedness stay), which is what Blender's glTF exporter undoes, so a model made Z-up in Blender stands
as modelled (checked in the colony view: a test model's +Y pillar stands up on every building). UVs keep glTF's
origin (top left), the game's too. Units are free: the original exporter wrote every model with root half-extents
1024 and its largest reach from the pivot exactly 1 (measured: `main/meteor`, `colony/kozepes1`, `spacewar/flag10`,
`spacewar/romb60`), and the screens size models by that, so the conversion scales the whole model uniformly to the
same: a `.glb` stands as large as the original it replaces, whatever its units. The glTF origin is the pivot the
game places and turns the model about, as the original's.

### 3. What is read

- **Nodes**: the default scene's node tree. A node with a mesh and no children becomes an `.xvo` named after the
  node; a node with children an `.xvb` (its own mesh as a first child `<name>_m`). Names keep their first 15 printable
  ASCII characters and stay unique (case-insensitive, like `openf`); a missing or clashing name becomes `node1`,
  `node2`, …. Cameras, lights and empty groups are skipped; skins are ignored (drawn in the bind pose).
- **Meshes**: triangle primitives only; `POSITION` (required), `NORMAL` (else flat shading, as glTF says),
  `TEXCOORD_0`; 8-, 16- or 32-bit indices. The primitives of one mesh share an `.xvo`, one material each.
- **Materials**: the base colour texture's image **name** is the game texture it uses (`panel_4_3.bmp`, looked up
  like the original's, ADR 0054 §6), the emissive texture's the self-illumination one; `alphaMode` `BLEND` sets the
  transparency `1 − alpha` of `baseColorFactor` (else opaque). Everything else in the material is ignored.
- **Level of detail**: `scenes[n].extras.lod` names the next level's model (its stem, in the same folder), as the
  original's `lod_archive`. Without it the model has one level.
- **Limits**: 64 MiB per file, 1 048 576 vertices per model, 65 536 vertices and 65 535 triangles per mesh (split
  bigger ones), 1 200 nodes and materials together, nesting 32 deep, model names up to 40 characters. Anything else
  is an error with the reason, never a crash.
- **Nothing outside the file, nothing that runs**: every buffer is the GLB's binary chunk; a buffer or image with a
  `uri` (a file, a web address or a data URI) is refused; images must be embedded PNG or JPEG (checked by their
  signature); a file that lists `extensionsRequired` is refused (Draco, meshopt, KTX2 textures in glTF and the like
  are out); optional extensions and `extras` other than `lod` are ignored. Sparse accessors are refused.

### 4. Animation

Not built: a converted model has no frames, and a screen that animates the original's (the colony's `teszt`, the
space battle's `ora` and `karika`, the parts the screens move by name) moves its parts the same way, by name.
Mapping is feasible: the node keys are per-frame matrices, so animation channels sampled at the frame count the
screen expects give the keys exactly; morph targets give the vertex frames. It needs the frame counts each
animated model's screen uses, so it waits for a mod that needs it.

### 5. Multiplayer

Unchanged: a `.glb` at a game path is a mod file like any other and is hashed into the room's fingerprint (ADR 0044
§4); under `hires/` (or in a `kind = "visual"` mod) it only changes what is drawn and stays outside.

### 6. Checking

`openIG2-check --mod` converts each mod's `.glb` on its own and reports a broken one as a mod problem
(`mod <name>: <path>`, with the reason); the models check then loads every model as the game does.

### 7. Textures (row 577)

Planned: KTX2 and DDS textures (and the PNG/JPEG embedded in a `.glb`) where a mod supplies a `.pvr` today.

### 8. Convert tool (row 578)

Planned: a tool that turns an install's `.xv` into `.glb` (and back) so modellers start from the original's model.

## Consequences

- A modeller exports glTF binary from Blender (or any tool), names the textures' images after game textures and
  drops the file in the mod at the model's path. `openIG2-check --mod` says what it can't read.
- The converted model is an ordinary `.xv` in memory: the renderer, the level-of-detail choice, the parts screens
  move by name and the `hires/` scaling need no change.
- No new dependency: the container and the JSON are read with `serde_json`, which the workspace already uses. The
  `gltf` crate (MIT/Apache-2.0, fits AGPL-3.0-only) was not taken: it adds derive macros and URI loading the subset
  above refuses anyway.

## Rejected

- **glTF text form (`.gltf`)**: its data lives in other files or data URIs; one self-contained file is simpler to
  check and to ship in a zip.
- **Mapping PBR materials onto a new lighting model**: the original's shading stays the classic look; the
  remaster's lighting (ADR 0088) has its own switches.

## Port status

- Part 1 (built): `ig2_formats::glb::to_xv`, the lookup in `Install::read` (file override and `hires/`), the mod
  check; tests on generated files, malformed input included. Checked in the client: a test `.glb` drawn in place of
  the main menu's meteor and ships and of every colony building, upright and at the original's size, textured by
  its image name.
- Part 2 (to do): the embedded images as the model's own textures (decoded and registered under their names, with
  row 577; today an image whose name is no game texture draws untextured), the tutorial page in `docs/modding/`,
  the user guide's Mods section, and a look at a mod model on the remaining 3D screens (space battle, starmap
  planets, design, research, spy).
