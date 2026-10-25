# `.xv`: 3D vector-object archives (original 1999 format)

Status: **fully verified** on the GOG install. 937 archives, 1,816 nodes (`.xvb`), 14,594 meshes
(`.xvo`) and 16,074 materials (`.xvm`) all parse. Every member's computed size matches the directory
exactly, except the two documented cases below. 936 scene trees resolve (`ig2_formats::xv`,
golden test `every_xv_archive_parses_exactly`).

Files: `dataraw/vector/<screen>/**/*.xv` (colony 449, spacewar 165, talalmanyok 134, design 73,
starmap 54, …). The game asks for `<name>.xvb`, and `openxv` maps that to `<VectorDir><name>.xv`.

## Evidence (ELF `ig2`; decompiled with `tools/ghidra/DecompileFunctions.java`)

| Symbol | ELF address | Role |
|---|---|---|
| `openxv(const char*, int)` | 0x5fd8a0 | loads `<VectorDir><stem>.xv` into memory; reads the directory from the end; `assert(count < 1400)`; uppercases names; prefix = `<stem>-` |
| `openf(const char*)` | 0x5fdc70 | `toupper(name)`, linear `strcmp` over the directory, seek to offset |
| `LoadXVBx` | 0x602ad0 | `openxv(name)` → `LoadXVB(name, mainobjectname)` → `BoxUjraszamol` (recompute bounds) |
| `LoadXVB` | 0x601c20 | node (`.xvb`) |
| `LoadObject` | 0x600e30 | mesh (`.xvo`), source file `../sources_base/rotate.cpp` |
| `loadmaterial` | 0x5ff760 | material (`.xvm`) → `loadtexture2(texture)`, `loadtexture(texture2, 1)` |
| `loadtexture` | 0x5ff310 | `TextureDir + name`, cached in `texture_vector` (≤ 2000) |
| `D3DTexture::Load` | 0x4f5e60 | loads; on failure retries with the last character → `t`, then → `f` |
| `command_glLoadTexture` | 0x426e30 | `.` → `_`, `dataraw/` + path as a texture resource (`.pvr`) |
| `Kocka2` | 0x5fac80 | scene walk: child translation, matrices, frame propagation |
| `Kocka` | 0x5f6a40 | scene walk for rendering: node animation keys (interpolated) |
| `kirak` | 0x60e160 | textured mesh renderer: vertex animation (interpolated positions) |
| `wirekirak` | 0x6083f0 | wireframe renderer: the only reader of the face flags |
| `SetFrame(meshid*, float)` | 0x4564f0 | sets a child's frame (child `+0xe4`, 8.8 fixed point) |

## Archive

```
[member data …]
directory: count × { u32 offset; char name[64]; }   // 68 bytes, NUL-terminated names
u32 count                                             // last 4 bytes of the file
```
Member size = next higher offset (or directory start) − offset. Names are matched
case-insensitively. Member naming: `<object>.xvb` is the root; children are `<object>-<child>.xvb`
(sub-node), `<object>-<child>.xvo` (mesh) and `<object>-<material>.xvm`.

Archives describe themselves; they don't depend on their filename. `colony/a.xv` holds members of
`kozepes1` (an alternative build of `kozepes1.xv`), so pick the root via the requested stem and
fall back to the single dash-free `.xvb`. `spacewar/flag22.xv` has no node at all (meshes only).

## Common header (`.xvb` and `.xvo`)
```
u16 marker   0xFFFF → v1, 0xFFFE → v2, anything else → v0
v1/v2: f32 origin[3]
v0:    origin = (marker, u16, u16) as floats
```

`LoadXVBx` re-measures the root after loading (`BoxUjraszamol` 0x602750): the half-extents become the vertices'
reach from the pivot, and the root's scale `+0x4c` = `max/1024` when the file's root is exactly 1024 wide
(re/screens/design.md §14.3).

## `.xvb` node
```
header
u32  frames_raw          // & 0x7FFFFFFF = animation frame count; high bit = flag
u32  lod_param           // stored to XVB+0x10 when a LOD archive is chained [inf: switch distance]
char lod_archive[16]     // if set (and the load depth allows): openxv(lod) + LoadXVB with prefix "<lodstem>-"
u16  nchildren
nchildren × 68-byte child record:
    +0x00 f32[3] translation t (Kocka2/Kocka read +0, +4, +8)
    +0x0c [4]    never read (often 0xCDCDCDCD exporter fill)
    +0x10 f32[9] 3×3 matrix M (copied to +100, then MultipleMatrix → +0x88)
    +0x34 char name[16]
frames × nchildren × 84 bytes   // frame-major: key(f, c) at (f·nchildren + c)·84
    f32[9] matrix   // full transform; replaces M while the child is animated
    f32[9] rotation // rotation-only chain (normals/lighting, spsorttype+0x24)
    f32[3] translation
```
Node animation (`Kocka` 0x5f6a40): only for a child whose frame (child `+0xe4`, −1 after load) was set
by `SetFrame` (8.8 fixed point: key `f = frame >> 8`, blend `(frame & 0xff) / 256` towards key `f + 1`,
clamped to the last frame) and when the node has frames. The child's position becomes
`parent·(t + key.translation − key(0, c).translation) + parent.t`; its matrices are
`MultipleMatrix(child[+100], MultipleMatrix(key.matrix, parent))` and the same with `key.rotation` and the
parent's rotation chain (`child[+100]` is the identity until `RotateObj` sets it). 1–2-frame nodes often hold uninitialised fill here: on 1,015
children of nodes with ≥ 3 frames, frame 0's translation equals `t` for 995 (golden test).
Child resolution (`LoadXVB` loop): `<prefix><name>.xvb` via a recursive `LoadXVB`, otherwise
`<prefix><name>.xvo` via `LoadObject`. A missing member leaves a NULL child.

### Level of detail (verified 2026-10-25, FX13 leftovers)
- **Chain.** `LoadObject` (ELF) reads the 16-byte `lod_archive`; a non-empty name gives `+0x18` = the next level's
  object and `+0x10` = 1.0f (the file's `lod_param` word is not used; screens may set it later: the starmap planets' heads run
  at 5 [obs: `probes/lod.js`]), an empty one `+0x10` = `+0x18` = 0. Shipped
  chains: `spacewar/rombX0` → `rombX1` → `rombX2` (the destroyers: 1, 4, 4 meshes), `flagX0` → `X1` → `X2`, the
  starmap planets `bXY0` → … → `bXY3`.
- **Choice** (`Kocka` ELF 0x5f6a40, exe 0x5281f0; per sort record, so the root and every child box decide alone):
  `size = |M·(1, 1, 1)|²·(fov/−z)²` (`M` the record's matrix `+0..+0x20`, `z` its depth `+0x50` in scene units clamped
  to −0.001, `fov` = 600·zoom, exe 0xE033E0; the root record's size is the literal 3147076), stored at `+0x7c`/`+0x84`
  by the parent; on entry `size·4`; then `level = 0, thr = valtashatar` and, while the node has a next level, `level < 10` (or `maxlevelszint`
  with flag 0x10000000) and `thr < 4·size·lod·lod` (skipped with flag 0x4000000: down to the chain's end):
  `level += 1`, `thr *= 30` (all f32). `valtashatar` is per screen: `Starmap_Belep` 1600, `Main3DInit` 3600,
  `Infoing3DInit` 2400, `ShowColony` 5625, `Research3DInit` 10000. The chosen level's boxes and vertices are drawn
  under the head's record (`AddToScene` sizes only the head; `AddToXVB` hands its slot to every level).
- **The head's box spans the chain**: `BoxUjraszamol` measures the vertices down the `+0x18` chain, so `romb60`'s box
  (1024, 857.8, 406.6) takes x and y from `romb60`, z from `romb61`/`romb62`.
- [obs: `probes/lod.js` on the starmap fleet panel: `romb60` at `AddToScene` 0.04 (the cells) has size 3119.55,
  at 0.08 (the big preview) 12478.14, `z` −0.5859, `valtashatar` 1600: the cells draw `romb61`, the preview `romb62`;
  test `scene::tests::panel_hulls_step_down_their_lod_chain_like_kocka`.]
- **Drawing** (`kirak` ELF 0x60e160, exe 0x531460; `kirak2` exe 0x530b20): every vertex as stored in the `.xvo`
  (no header scale), `X = row0·v + pos.x` etc. with the box's own record; a child box's record is
  `parent·(box +0x88)` with `+0x88` = `diag(origin/e)·M` after the recount [obs: `probes/kocka.js`, `probes/kirak.js`
  on `kolonizalo`: all 21 children match to 1e-4, the drawn vertices are the file's]. After the recount a child's
  `+0x10` = `diag(ec/E)` and `+0` = `(o_c − o)/E` (`boxujraszamol` ELF 0x5ed9f0: its own box extent `ec` and pivot `o_c`
  over the parent's `E`, `o`), the vertices `(v_root − o)/ext` per axis: `AddToScene`'s `diag(ext·s/1024)` restores the
  shape, and `ScaleObja` (which replaces `+0x10`) stretches every axis to the same size [obs: the SpaceWar debris,
  re/screens/spacewar.md §9.13].

## `.xvo` mesh
```
header
u32  nverts
u32  frames_raw          // & 0x7FFFFFFF = frames
u32  unknown             // read into a local and dropped
char chained[16]         // if non-empty, LoadObject(chained) recursively; stored at +0x18, +0x10 = 1.0f
u16  nfaces
v2:  i32 nmat            // < 1 → 1 material with empty name, no names read
     (v0/v1: nmat = 1)
nmat × char[16]          // material names → loadmaterial(prefix + name + ".xvm")
nverts × 32: f32 pos[3], f32 normal[3], f32 uv[2]    // normal renormalised at load; zero → (1,0,0)
nfaces × 8:  u16 a, b, c, u16 material              // material used by v2 only; clamped to nmat−1
frames × nverts × 24 bytes                          // f32 pos[3], f32 normal[3] per vertex per frame
nfaces × u8 face flags                              // wireframe edge mask; if missing, every flag = 7
```
After loading: `CalcCullPoints`, and `optimizeobject` when `(frames_raw & 0x7FFFFFFF) < 3` and the
high bit is clear (`quadpatch` objects use a different optimisation mode).

Vertex animation (`kirak` 0x60e160, the textured path): only when `frames ≥ 3` and the child has a frame
(as above). Positions are lerped between frame `f` and `f + 1` (clamped) and transformed like the base
vertices; the frame normals aren't read by any renderer. Frame 0 equals the base vertices in all 3
shipped animated meshes (`colony/teszt`, `spacewar/karika`, `spacewar/ora`); 1–2-frame meshes carry a
frame block too, often uninitialised fill.

Face flags (`wirekirak` 0x6083f0, the wireframe path selected by render flag 0x40): bit 0 draws edge
c–a, bit 1 a–b, bit 2 b–c (face `= a, b, c`). The textured and shadow renderers ignore them, so 7 =
"all edges".

## `.xvm` material (52 bytes)
```
char name[16]; char texture[16]; char texture2[16]; u32 flags;
```
The texture names are 1999 `.bmp` names (`panel_4_3.bmp`). `loadmaterial` loads `texture` with
`loadtexture2` and a non-empty `texture2` with `loadtexture(…, 1)`: a self-illumination map, drawn additively over
the first by `selfillumkirak` (re/screens/design.md §14.3; ships' `*si*.bmp` window lights);
`fenyek_2.bmp` and `reflec.bmp` as `texture2` are swapped for a runtime texture.
Five shipped materials are 48 bytes, with the flags missing.

Texture path (verified in code): `loadtexture` prepends the global `TextureDir`, which each screen's
init code sets (`textures/colony/`, `textures/colony/terrainmap/<n>/` for terrain objects,
`textures/design/screen/`, …; `Init3dColony` sets it next to `VectorDir`). `D3DTexture::Load` tries the
name, then with its last character replaced by `t`, then by `f` (`foo.bmp` → `foo.bmt` → `foo.bmf`;
the install ships `diplomac/*_bmt.pvr` and `colony/shadowmap_bmf.pvr`), and `command_glLoadTexture`
replaces every `.` with `_` and loads `dataraw/<path>` as a `.pvr` resource. `Install::resolve_texture`
implements exactly this; `Install::resolve_model_texture` guesses `TextureDir` from the archive's
screen directory (15,958 / 15,983 references resolve).

## Deviations seen in data
- 68 v0 meshes omit the face-flag block (sphere planets in `info/bolygo_*.xv` and similar). The game
  handles this (`xvread` returns 0 → flags = 7).
- Name fields hold uninitialized memory after the NUL (exporter artefact). Never compare past the NUL.

## Verified by rendering (2026-10-15, `ig2-viewer --snapshot`)
- Child placement is `p' = M·v + t`, with `t` = child record bytes 0x00..0x0C as `f32[3]` (the 4th f32
  at 0x0C is junk). Cruisers, colony buildings and alien ships assemble correctly.
- **All 15,464 child matrices in the install are pure diagonal scales**: the exporter baked
  rotations into the vertices. `M` = child half-extents ÷ parent half-extents (node/mesh `origin`
  fields are per-axis half-extents; vertices are normalised to about [-1, 1]).
- Texture V needs no flip.
- Texture mapping: `foo.bmp` → `dataraw/textures/<screen>/foo_bmp.pvr`, where `<screen>` is the archive's
  `dataraw/vector/<screen>/` (fallback: any screen). The code rule is above (`TextureDir`); the misses
  are runtime-generated (planet maps, loading screens). `Install::resolve_model_texture`.
- `Pod*` children of ship designs have untextured materials: equipment slots filled at runtime.

## Open questions
- None for the file layout. The material `flags` word and the render-flag words of the child records
  (`+0x54`/`+0x58` masks, set at runtime) belong to the renderer port (Phase 3).
- (Settled 2026-10-24.) The material's last word is an f32 transparency, used as below.

## Material transparency (`.xvm` +0x30)
- `loadmaterial` (ELF 0x5ff760 / exe 0x534470) reads it into the material's +0x30 (absent file: 0; the five 48-byte
  `.xvm`s leave it as uninitialised heap in both binaries). Nothing writes it after load; `changeaktmatarial(self)`
  never read it.
- `Kocka` (ELF 0x5f6a40 / exe 0x5281f0), per drawn mesh: `t = node.t != 0 ? node.t : material[0].t`: the object's
  transparency (`SetObjTransparency` ELF 0x456d10 / exe 0x4533a0, box +0x60 ELF / +0x58 exe, no clamp; screen
  wrappers such as `spySetObjTransparencyH` clamp to ≥ 1e-6) wins when non-zero; else the mesh's FIRST material's,
  whatever the batch. Never multiplied or maxed. A child node takes its own `t` (a file child) or `1 − (1 − parent)(1 −
  own)` (a hierarchical child, box +0xe8 bit 0).
- Opacity byte `o = float2int(255 − 255·t)` (exe 0x542d80, f32 round-to-nearest-even, no clamp): 0.65 → 89 [obs: the
  trade panels' light probe], 0.25 → 191, 0.7 → 76. `fenyekszamolasa` (exe 0x52f190) puts `o` in the vertex alpha
  (property bit 8, the default) or scales RGB by `o/256` additively (bit 8 clear); `rgbac` = `o` drives the
  self-illumination pass. `renderstateallitas` (exe 0x536f90): `o != 255` turns blending on; the depth write stays.
  Engine-wide, no screen gate.
- Port: `scene::Part::transparency` (first material), `Part::opacity(object_t)`, `Part::alpha` (the screens' bakes);
  test `scene::tests::material_transparency_is_the_fallback`.
- Where it shows (inventory of every shipped model, 2026-10-24): trade panels/backdrop (0.65) and grid (0.25), and the
  messages frame's `messbck` 5/0/0 (0.6 → 102/255; drawn until the clip hides it) [obs: `messages-open.json`
  t129/t132 5.2/6.0 → 0.8, t290 2.6 → 0.6]. Elsewhere the exe overrides or hides every material-transparent part:
  design `desbck` 1..5 and diplomacy `dipbck8` 1/8, 1/9 hidden; research `resbck` Box01 (`KELLVILLOGAS`), spy pedestals
  and figures (`spySetObjTransparencyH`, ≥ 1e-6), spy `Object05/06` (0.8), info heads (`Infoing3DInit` 0.01) and list
  discs, messages 1/6, 2/7, 4/0 (0.4), colony `_A_` (0.6), spacewar shells `bolygo3` (0.7) set their own; spacewar shots are particles (form alpha), not `Kocka` meshes. Not drawn by the
  client: `main/csillag` 0.9, `starmap/cirk11/12` 0.5, `spacewar/hajoinfo` 0.2, `infopanel` 0.36, `halal` 0.5.
