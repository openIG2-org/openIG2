# The GOG remaster's render layer

The 1999 engine transforms, lights and fogs its vertices on the CPU and hands them to Gyroscope's GL layer (`sg3d`),
which draws them with its own shaders, materials and state blocks. Those ship in the install (`render/`, decoded to
`_local/extracted/render/`) and decide how every 3D screen looks: colour space, blending, depth, texture mapping, fog,
shadows and two procedural materials. This spec lists them with the ELF evidence and what the client does with them
(`ig2-client::render3d`, `Scene3d::gamma`). Status: specified, applied to the colony, research, spy, diplomacy and
starmap screens; capture results in `diff-results.md` (`render-layer`).

## 1. Files (`render/`)

| File | Content |
|---|---|
| `ig2.material` | the 1999 engine's passes: `OPAQUE` → `ig2.program`, `CLOUD` → `ig2_cloud.program`, `VIDEO` → `ig2_video.program`, `FONT` → `ig2_font.program`; every pass uses `video.state` |
| `video.state` | `BLEND` on, `SRC_ALPHA`/`ONE_MINUS_SRC_ALPHA`, `CULL_FACE` off, `DEPTH_TEST` off, `DEPTH_MASK` on, `DEPTH_FUNC LESS` (the defaults; §2.2 toggles them) |
| `ig2.program` | 8 variations of `ig2_vertex.shader` + `ig2_fragment.shader` (bits `BIT0`..`BIT2`, plus `DUAL`) |
| `ig2_vertex.shader` | pre-transformed vertices: `dp = (x·w, y·w, z·w, w)` with the engine's `rhw` in `w`, `gl_Position = proj·dp`; `var_uv = (uv·uv_mul.xy·w, w)` (perspective-correct, `uv_mul` from the texture, §3); `var_specular.w = 1 − specular.a` (the D3D fog factor); `BIT1`: point sprites of `sg3d_window.y·0.008` pixels; `BIT2`: `var_uv1 = (uv1·w, w)` |
| `ig2_fragment.shader` | `c = texture(uv/z)` (`DUAL`: alpha from a second texture's green); `BIT2`: `c.rgb *= shadow(uv1)` (desktop: four taps at (−¼, −¾), (¾, −¼), (−¾, ¼), (¼, ¾) texels of a 2048 map, averaged; mobile: one); `c *= vertex colour`; `c.rgb = mix(c.rgb, ig2_fog.rgb, var_specular.w·ig2_fog.w)`; `BIT0`: discard below alpha 0.5; `BIT1`: the star texture's red × the colour at `gl_PointCoord` |
| `ig2_fragment_cloud.shader` | the planets' procedural clouds (starmap.md §3.3) |
| `ig2_fragment_video.shader` | four taps ±0.5/256 diagonally, averaged; `rgb *= abs(fract(gl_FragCoord.y·256/sg3d_window.y + sg3d_time.y·4) − 0.5)·1.5 + 0.5`; × the colour; no fog |
| `ig2_fragment_font.shader`, `ig2_vertex_font.shader` | the bitmap font: red channel × colour, UVs clamped to the glyph cell (`sg3d_position0.zw`, `texcoord1`) |
| `blit.*`, `default.state`, `unlit.*`, `font.*`, `video.*` | Gyroscope's own front end (menus, videos `anim/mpeg4/*.mp4` as YUV textures, the credits text); not used by the 1999 screens |

No program converts colours: the 2D pass (hud.md §1.1) and the 3D pass work on the stored gamma-space values of
plain `GL_RGB`/`GL_RGBA` textures on a non-sRGB frame, and the vertex colour multiplies the texel without doubling.
The specular colour's RGB is never used (only its alpha, the fog factor).

## 2. How the engine drives them (ELF)

### 2.1 Commands
The engine calls `SGL_gl*` wrappers that queue `command_gl*` records for the render thread (`ig2_pushcommand`).
`ig2screen::render` (ELF 0x423b80) binds the star texture (`SG3D_TEXTURE_OPACITY`), the plasma noise
(`SG3D_TEXTURE_NOISE`, starmap.md §3.3) and the shadow framebuffer's colour (`SG3D_TEXTURE_SHADOW`, else white), and
sets `sg3d_time = (acttime/1000, (acttime % 1000)/1000, 0, 0)` (`game::acttime`, exe 0xFD1520, ms). `gl_draw_tlv`
(0x4f68f0) uploads 0x28-byte `D3DTLVERTEX`es (x, y, z, rhw, colour, specular, uv, uv1) and draws them with
`command_glDrawArrays`/`DrawElements` (0x426b60/0x4269f0): the material `ig2.material`'s pass for the bound texture
(`ig2_actx`), the variation bits, then `uv_mul` reset to (1, 1, 0, 0) after every draw.

### 2.2 State toggles (`command_glEnable`/`Disable` 0x426430/0x4265e0)
They edit a private copy of the state block (`ig2_sb`, re-sent on the next draw) or the variation:

| GL enum | Effect |
|---|---|
| 0xbe2 `GL_BLEND` | state bit 0 |
| 0xb44 `GL_CULL_FACE` | state bit 1 |
| 0xb71 `GL_DEPTH_TEST` | state bit 3 (`glDepthMask` 0x426c80: bit 4) |
| 0xb60 `GL_FOG` | `ig2_fog.w` = 1 / 0 (`command_glFogColor` 0x4278e0 sets `ig2_fog.rgb` = the colour bytes /255) |
| 0xbc0 `GL_ALPHA_TEST` | variation `BIT0` |
| 0xdeadbeef | variation `BIT2` (the shadow map) |
| `GL_POINTS` draws | variation `BIT1` (`command_glDraw*`: primitive mode 0, the stars as point sprites) |

`command_glBlendFunc` (0x426780) stores the factors. `gl_set_defaults` (0x4f6400): depth test and write on, blend
`SRC_ALPHA`/`ONE_MINUS_SRC_ALPHA`, fog off with colour 0xffffff, alpha test off, culling off. The 2D draws
(`DDBrushToScreen3d`, `KiirStr`, `polygonsfader`, `line2d`, …) turn depth and fog off.

### 2.3 Per object (`renderstateallitas` ELF 0x5fa410, called by `Kocka` for every mesh)
From the node's property word `spsorttype+0x6c` (cached `renderstate_*` globals skip repeats):
- bit 1 (2): depth test; bit 2 (4): depth write (meshes are depth-tested, in the old engine's sorted order);
- bit 3 (8): blend `SRC_ALPHA`/`ONE_MINUS_SRC_ALPHA`, else `ONE`/`ONE` (additive);
- a transparency (`param_3 != 0xff`) forces blending on and clears bit 0x2000;
- 0x200 (set when the texture has alpha, `d3dt+4`): alpha test `GREATER 0.5` (`BIT0`) unless `shiti740`;
- 0x2000: fog on with `fogszin` and blending off (opaque, fogged); else fog off and blending on;
- lighting (`fenyekszamolasa` exe 0x52f190): 0x1 = per-vertex lighting; 0x100 without 0x1 = flat light (ambient + each parallel light's colour·205>>8, no normal: the colony's trees, colony.md §3.1); neither = full bright;
- 0x800: no texture.
- the transparency leaves the depth bits alone: a transparent object still writes depth (the design hull, design.md §14.3);
- after each batch, when the mesh's first material has a second texture, `selfillumkirak` (0x613da0) draws it again
  additively (`ONE`/`ONE`) with the batch material's second texture, unlit in the grey `255 − t·255` (design.md §14.3).

### 2.4 Textures (`command_glLoadTexture` 0x426e30, `sg3d::texture_t::load_pvr` 0x86db50)
`dataraw/<name with . → _>.pvr`; the pass comes from the name: `charset_bmp` → `FONT`, `xx_bmf` → `VIDEO`,
`ethcldbp_bmp` → `CLOUD`, else `OPAQUE`. The PVR's `SGLB` metadata: `TXNV` (bit 0/1 repeat s/t, bit 2 trilinear,
bit 3 trilinear + the maximum anisotropy, bit 4 nearest; mip filters only with more than one level) and `CNST`
(named shader constants handed to the material: `load_constants`). `load_pvr` uploads each file's own mip levels; the client
uses them too (`Pvr::decode_levels`, `render::mip_chain`): filtering from the top blurs alpha-tested textures (the scaffold's rails).

### 2.5 Environment mapping (`vagashatarkirak` exe 0x5395d0, ELF 0x606c90)
Property 0x8000 makes `Kocka` (dispatch exe 0x52848f: 0x8000, then 0x1000000, then 0x40 = `wirekirak`) draw the mesh with
`vagashatarkirak`: no shadow-receive, no self-illum pass. 0x4000 (tested only inside it) switches on the env texcoords and
`changeaktmatarialself` (exe 0x52dda0), which binds the material's texture2 (null: the bound texture1 stays). 0x1000000 is the
construction cut (`clippolygonvagashatar`), not env mapping.
- Texcoords (`envszamol` ELF 0x6110e0, inlined in the exe; `normalizevektor` exe 0x4501f0), f32, in the mesh's own space:
  `e = normalize(camera − P)` (left raw when the length is under 1e-5), `uv = (e.x − N.x, e.y − N.y)` with the vertex normal `N`;
  no scale or offset, it replaces texcoord0, `nz` unused. Neither P nor N is rotated: the highlight follows the camera's place
  relative to each vertex. `scene::env_uv`.
- One pass, the lit vertex colour, fog as `kirak`, back faces culled. Blend by `renderstateallitas`: 0x2000 (inherited from the
  building root) makes it opaque and fogged; a transparency byte (`_C_` 0.6, ponds 0.28, `VIZ` 0.63) clears 0x2000 and blends
  `SRC_ALPHA`/`INV_SRC_ALPHA`.
- The texture: `loadmaterial` (exe 0x534470, swap at 0x5346b9) replaces a texture2 named `reflec.bmp` or `fenyek_2.bmp` by the
  texture1 name of `HAmp` (the colony background box, exe 0xde0298, set by `Init3dColony` before the building loop) = the planet's
  sky texture; `ChangeTexture` (exe 0x451af0) also sets the dome's texture1 to it. Ponds (`tavak`, planet types 1/5):
  `ChangeSelfTexture` (exe 0x451ab0) loads the sky texture's name with its first letter replaced by `v`.
- Users: the colony only (`Init3dColony` exe 0x47d520: `_B_` 0xc000, `_C_` 0xc000 + 0.6, ponds, terrain water `VIZ` 0xc008;
  `ShowColony` exe 0x49f130: the panel preview and the build cursor's copy; `EpuletFelepult` exe 0x4741a0). `HAmp` is a shared
  global, so elsewhere the swap uses a stale texture (SpaceWar `valami.xvb` `_B_Cylin02`, row 370).
- Verified (`probes/envmap.js`, `quick-1.sav` planet 17): 66 drawn triangles match the formula to 1.3e-7; test
  `scene::tests::env_uv_matches_the_exe`. Client: `colony::{building_env, env_texture, env_map}`, `EnvPart` (drawn per frame from
  the eye), the panel previews (`colony_ui` `PanelBuilding` takes the sky).

## 3. What the install's textures carry
`ig2-formats` example `pvrmeta` (`cargo run --release -p ig2-formats --example pvrmeta -- <install>`) over all
2,081 PVRs: every one has `TXNV` = 11 (repeat both ways, trilinear with anisotropy when it has mips); only the nine
starmap planet maps `dataraw/textures/starmap/p1..p9` (BC1, 11 levels) carry `CNST`: `uv_mul` = (1, 0.5, 0, 0)
(starmap.md §3.3). Screen pictures and 2D brushes have one level (hud.md §1.1); models' textures mostly 5..10.

The `VIDEO` material: `PlayAnimVector` (ELF 0x45ea70) uploads each head-video frame as plain RGB into the disc's
texture `xx.bmf` (`SGL_glTexImage2D`, no interlacing; `DrawAnimInterlace` 0x5a37a0 belongs to the messages screen's
2D animation only); the scan lines on the spy and diplomacy discs are the shader's, running with `acttime`.

## 4. The shadow map (`IG2_USE_SHADOW` 0x114bd00 = 1)
The colony screen, with `OptionsKellArnyek`, `lehetarnyek` (0x118b388 = 1) and for the terrain's objects
`kelltereptargyarnyek` (0x1166818 = 1):
- **Who.** `Init3dColony` sets property 0x400000 (casts) on every building (`Epulet3d`) and the terrain's `tt*`
  and `fak*` groups, 0x800000 (receives) on the terrain object, the roads (`AddUtDarab`) and the buildings' `+r`
  parts (whose cast flag it clears). `findboxxvb2` (exe 0x453970, case-insensitive substring) searches only the
  nodes of each LOD level's root `.xvb` (`getnextmeshpointer` exe 0x453b70 steps LOD levels, not children), so only
  a part whose top-level node name contains `+r` receives; the Park's `+r+Rectangle01/2` tree quads sit in the child
  archive `prkv2-fa.xvb`, keep casting and don't receive (row 391). Properties inherit down the tree (`PropertiesOn` 0x456a50 sets `+0x54` and
  clears the bit from the inherit mask `+0x58`; children take `parent & mask | own & ~mask`).
- **The map.** `ShowColony` calls `rendershadowmap` (0x604f50) before the scene: `SGL_glSetTarget(1)`
  (`command_glSetTarget` 0x427c00: the shadow framebuffer, cleared to white, projection `scale(1/128)` then
  `translate(−1, −1)`), a camera `CameraRotate(0, (2, 2, 2))` (its rows `a`, `−b` of the look-at basis, colony.md §1),
  then `Kockashadow` (0x5fc430) draws every caster's triangles at `(rows·p)/32 + 128` (`shadowzoom` 1/32, `shadowkp`
  128: a 256-unit map, 8,192 world units across) with colour 0x80808080, no texture, no blend, no depth; whole triangles, alpha ignored (alpha-tested quads shadow as full quads).
- **The receivers.** `Kockarecshadow` (0x5fcae0) stores each receiver vertex's `(rows·p/32 + 128)/256` as its second
  texcoord (`VertType+0xc/+0x10`); `Kocka` draws receivers through `shadowkirak` between `SGL_glEnable(0xdeadbeef)` and
  `Disable`: `BIT2` multiplies the texel by the map's green (0.5 in shadow) before the vertex colour and the fog.
- The units have their own `WF_Tank::DrawShadow` (0x481d80) [not ported].

## 5. The client
`render3d::Scene3d::gamma` draws a scene as the remaster does: on the frame's raw (non-sRGB) view with the textures'
raw views and their mesh sampler (repeat, trilinear, anisotropy), `texture × vertex colour` (lights clamped to 1 like
the colour bytes), the fog mix, `BIT0` at 0.5, the frame's clear colour stored as given. Scenes rendered into a
target (research, diplomacy, the colony's building panel) are laid over the frame with `Blend::GammaPremul`
(`c·a + frame·(1 − a)`), which replaces the earlier per-screen corrections for linear blending (`spy_scene::gamma`,
the `/a²` glass tint). Shader-lit scenes (the colony) use the engine's parallel light (`fenyekszamolasa`'s
`shade[float2int(|n − L|²·2047)]`, research.md §2.1) instead of Lambert. `Scene3d::shadow` is §4 (`Shadow::Cast` /
`Receive` per draw, a 2048² map), `MeshDraw::video` the `VIDEO` material (`--acttime` pins `sg3d_time`; the capture
tool passes the exe's).

Light directions for the colony: `fenyekszamolasa` takes `|n' − L|` with the file normal's model z negated; the
colony's models are quarter-turned (`RotateObj(0, π/2, 0)`, model z → −h), so for the client's file normals the
lights `light2` (2, 2, −2) (the terrain and buildings, `mainmeshid1`) and `light5x` (10, 10, −10) (the building
panel's model, `mainmeshid2`, ambient 0x404040, colour 0xffffff) act from (−1, 1, −1) [obs: the colony view 18.3 →
11.7 against the other seven sign choices; the panel 13.0, best of nine].

The info and messages frames (`screen_frames`) take the engine's lights per vertex (info.md §1.1, messages.md §4) in
the gamma pass, laid over the frame with `Blend::GammaPremul` (messages panel 14.9 → 0.3, diff-results.md
`infomsg-lights`).

Open: the units' shadows; the research and spy side lights (research.md §5.6); the colony terrain's own brightness in
places (the rock's lit faces); the info frame's faces turned away from the lights (the pillar's right half, lit grey
in the exe, dark in the client).
