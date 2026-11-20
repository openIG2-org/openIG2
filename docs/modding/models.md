# Models from Blender

This page puts a model you made into the game: a building in the colony view, a ship, any 3D object the game draws.
You export it from Blender (or any tool that writes glTF 2.0) as one `.glb` file, drop it into a mod at the
original model's path, check it and see it in the game. The rules behind it are in ADR 0090; the user guide's
"Texture and model packs" covers the rest of a pack.

Your mod ships only what you made: never the game's own models or pictures.

## 1. Find the model to replace

The game's models are the `.xv` files in the install's `dataraw\vector\<screen>\` folders: `colony` (buildings,
tanks), `starmap`, `spacewar` (ships in battle), `design`, `research`, `spy`, `main` (the main menu). Your file takes
the same name with `.glb` for `.xv`: `kozepes1.glb` stands for `dataraw\vector\colony\kozepes1.xv`.

## 2. Model it in Blender

- **Axes**: model as Blender shows it, Z up. The exporter turns it into glTF's Y up and the game turns it back, so
  what stands up in Blender stands up in the game.
- **Size**: any units. The game scales your model so that it is as large as the original it replaces: it fills the
  original's place, whether you modelled it in metres or millimetres. Keep its proportions close to the original's;
  a model much taller than the original stands taller.
- **Origin**: the scene's origin (Blender's world origin) is the point the game places and turns the model about.
  Build the model around it where the original's is (a building's on the ground in its middle).
- **Parts**: every mesh object becomes a part named after the object (up to 15 letters and digits). Some screens
  move parts by name (a turret that turns, a part that blinks); keep the original's part names for those.
- **Triangles**: up to 65 535 triangles and 65 536 vertices per mesh object (split bigger ones), about a million
  vertices in the whole model.

## 3. One texture set per material

Each material gives the game at most two pictures, both through the Principled BSDF:

- **Base Color** with an image texture: the part's texture. UVs come from the first UV map.
- **Emission** with an image texture: the self-illumination picture (windows, lights), drawn added on top where the
  original draws lit parts: the ships' lit windows in the design, starmap and trade screens. Colony buildings draw
  none, as in the original.

Which picture the game draws depends on the image's name in Blender:

- Any ordinary name (`hull.png`, `Hull Paint`): the picture inside the `.glb` is used, drawn as it is. At most 4096
  pixels a side; PNG or JPEG (Blender converts other formats when it exports).
- A game texture's name ending in `.bmp` (`kozepes.bmp`, at most 15 characters): the game's own texture of that name
  is used and the picture inside the file is ignored. This lets a model wear the original look without shipping it.

For a see-through material set Blend Mode to Alpha Blend: the Base Color's alpha value (the number, not the
picture's alpha) sets how see-through the whole material is. Metallic, roughness, normal maps, occlusion, vertex
colours and the base colour's tint are not used: the game keeps its own lighting.

## 4. Export

File → Export → glTF 2.0, then:

- Format: **glTF Binary (.glb)**. The text form (`.gltf`) with separate files is not read.
- Include: the objects of the model only (Selected Objects, or a scene that holds only it).
- Transform: **+Y Up** ticked (the default).
- Data → Mesh: Apply Modifiers ticked; UVs and Normals ticked.
- Data → Material: Export; Images: Automatic (PNG or JPEG inside the file).
- Compression (Draco) off: a file that needs it is refused.
- Animation: off. Animations in the file are ignored (§7).

Save it as `<name>.glb`, with the original's name.

## 5. Put it in a mod

Make a mod folder (the [first mod](first-mod.md) shows how) and put the file at the original's path, in one of two
places:

| Place | What it does | Multiplayer |
|---|---|---|
| `mods\<mod>\dataraw\vector\colony\kozepes1.glb` | replaces the model | counts: every player in a room needs the same file |
| `mods\<mod>\hires\dataraw\vector\colony\kozepes1.glb` | drawn in its place, a look only | doesn't count: players may differ |

A look-only model never changes the game, so it is the place for most models. A model at the game path also suits a
model your mod adds under a new name (a new ship's look). In one mod a `.glb` wins over an `.xv` of the same name;
across mods the mod higher in the list wins.

### Level of detail (optional)

The original swaps a model for a simpler one far away. To do the same, give the Blender scene a custom property
`lod` whose value is the simpler model's name (`kozepes1_far`), tick Include → Custom Properties when exporting, and
put `kozepes1_far.glb` in the same folder.

## 6. Check it

```bash
openIG2-check --quick --mod my-models
```

Each `.glb` is read on its own and every picture inside it decoded. A broken one shows up with the mod, the file and
the reason, for example `mod my-models: dataraw/vector/colony/kozepes1.glb: image 0 names a file or data URI; embed
it in the .glb`. Then switch the mod on and open a colony: your building stands where the original did.

## 7. What is not supported

- **Animation**: models are static. The parts the game moves by name still move (that is why part names matter);
  the file's own animations, skins (drawn in their rest pose) and shape keys are ignored.
- **Files outside the `.glb`**: everything must be inside the one file; external pictures, web addresses and data
  URIs are refused.
- **Extensions a file requires** (Draco, meshopt, KTX2 pictures in glTF): refused. Cameras and lights are skipped.
- **The glTF text form** (`.gltf`).
- **Physically based shading**: only the base colour and emission pictures and the material's alpha are read.
