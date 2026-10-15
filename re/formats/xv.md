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
| `loadmaterial` | 0x5ff760 | material (`.xvm`) |

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

## `.xvb` node
```
header
u32  frames_raw          // & 0x7FFFFFFF = animation frame count; high bit = flag
u32  lod_param           // stored to XVB+0x10 when a LOD archive is chained [inf: switch distance]
char lod_archive[16]     // if set (and the load depth allows): openxv(lod) + LoadXVB with prefix "<lodstem>-"
u16  nchildren
nchildren × 68-byte child record:
    +0x00 [16]  unknown
    +0x10 f32[9] 3×3 matrix (copied to +100, then MultipleMatrix → +0x88)
    +0x34 char name[16]
nchildren × frames × 84 bytes   // 21 f32 per child per frame [inf: matrix + translation keys]
```
Child resolution (`LoadXVB` loop): `<prefix><name>.xvb` via a recursive `LoadXVB`, otherwise
`<prefix><name>.xvo` via `LoadObject`. A missing member leaves a NULL child.

## `.xvo` mesh
```
header
u32  nverts
u32  frames_raw          // & 0x7FFFFFFF = frames
u32  unknown
char chained[16]         // if non-empty, LoadObject(chained) recursively; stored at +0x18, +0x10 = 1.0f
u16  nfaces
v2:  i32 nmat            // < 1 → 1 material with empty name, no names read
     (v0/v1: nmat = 1)
nmat × char[16]          // material names → loadmaterial(prefix + name + ".xvm")
nverts × 32: f32 pos[3], f32 normal[3], f32 uv[2]    // normal renormalised at load; zero → (1,0,0)
nfaces × 8:  u16 a, b, c, u16 material              // material used by v2 only; clamped to nmat−1
frames × nverts × 24 bytes                          // [inf: 6 f32 per vertex per frame]
nfaces × u8 face flags                              // optional: if missing, every flag = 7
```
After loading: `CalcCullPoints`, and `optimizeobject` when `(frames_raw & 0x7FFFFFFF) < 3` and the
high bit is clear (`quadpatch` objects use a different optimisation mode).

## `.xvm` material (52 bytes)
```
char name[16]; char texture[16]; char texture2[16]; u32 flags;
```
The texture names are 1999 `.bmp` names (`panel_4_3.bmp`). In the remaster they resolve to `.pvr`
under `dataraw/textures/` [inf; the mapping is in `loadmaterial` → texture loader, to verify].
Five shipped materials are 48 bytes, with the flags missing.

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
  `dataraw/vector/<screen>/` (fallback: any screen). Resolves 99.8% of 16,310 references; the 30 misses
  are runtime-generated (planet maps, loading screens). `Install::resolve_model_texture`.
- `Pod*` children of ship designs have untextured materials: equipment slots filled at runtime.

## Open questions
- The child record's first 16 bytes, the meaning of the node animation keys (21 floats), and the
  vertex-animation layout.
- Face flag bits (7 = default: probably visible | lit | textured).
- Confirm the texture mapping in code (`loadmaterial` → texture loader); currently empirical.
