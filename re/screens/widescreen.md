# Widescreen: how the GOG remaster lays out its screens

Status: **verified** (2026-10-18): decompiled (ELF + exe), the layout globals read out of a running ig2.exe at six
window sizes, and captures of the starmap, colony, info and research screens at 800 × 600, 1920 × 1080 and
2520 × 1080 (21:9). Decision: ADR 0010. Implementation: `ig2-client::kit::layout` (§6).

## 1. Where the size comes from
`ig2.settings` in the install (plain text): `window=1` (windowed), `width=`, `height=` (the GOG default is
800 × 600), `//toolbar=1` (the "mobile-style sidebars", off by default, §5). The window's size reaches the game as
`sdl_width`/`sdl_height` (exe 0xB7721C/0xB772A0) and `ig2_resize` lays out the screen from it. The captures change
these two lines for each run and put the file back afterwards (`run_screen.py` itself takes no size).

## 2. The rule: `ig2_resize` (exe 0x4D84D0, ELF 0x550050)
All integer arithmetic (C division truncates). Without the toolbar:

| Global (exe) | Meaning | Window at least as wide as 4:3 | Narrower than 4:3 |
|---|---|---|---|
| `IG2_WINDOW_POSX/POSY/WIDTH/HEIGHT` 0xB770C8/CC, 0xB77090/94 | the GL viewport everything is drawn in | `0, 0, W, H` | `0, (H − W·480/640)/2, W, W·480/640` |
| `IG2_VIRTUAL_POSX/POSY/WIDTH/HEIGHT` 0xB770D8/DC, 0xB770D0/D4 | the 640 × 480 virtual frame in window pixels | `(W − H·640/480)/2, 0, H·640/480, H` | the same as the window |
| `IG2_WIDTH/HEIGHT` 0xB770B8/BC | the whole window (the cursor's viewport) | `W, H` | `W, H` |

- **Wider than 4:3: the window keeps the full width.** The 2D layer is drawn into the centred 4:3 frame
  (`ig2_render_first` exe 0x4D7FE0 / ELF 0x550290: the ortho matrix scales x by `(4/3)/(WIN_W/WIN_H)/320` and y by
  −1/240 around (320, 240) over the full-width viewport, so 1 virtual pixel = `H/480` window pixels and virtual x 0
  sits at the frame's left edge). Nothing 2D is anchored to an edge or stretched, except the background pictures
  (§3). The strips beside the frame belong to the 3D scene.
- **Narrower than 4:3 (5:4, portrait): letterboxed top and bottom.** The viewport itself becomes the centred
  `W × W·480/640` band, so nothing is drawn above or below it (the bars stay black). The virtual frame then has no
  margins (`ModifyViewPort`'s margin below is 0).

Observed in ig2.exe (`layout_probe.py`, window `[x, y, w, h]`, frame `[x, y, w, h]`):

| Window | `IG2_WINDOW_*` | `IG2_VIRTUAL_*` | `HatarX1..X2`, `HatarXm` |
|---|---|---|---|
| 800 × 600 | 0, 0, 800, 600 | 0, 0, 800, 600 | (0..639, 640) |
| 1920 × 1080 | 0, 0, 1920, 1080 | 240, 0, 1440, 1080 | −106..745, 852 (read) |
| 2520 × 1080 | 0, 0, 2520, 1080 | 540, 0, 1440, 1080 | (−240..879, 1120) |
| 1366 × 768 | 0, 0, 1366, 768 | 171, 0, 1024, 768 | (−106..745, 852) |
| 1280 × 1024 | 0, 32, 1280, 960 | 0, 32, 1280, 960 | (0..639, 640) |
| 1000 × 1000 | 0, 125, 1000, 750 | 0, 125, 1000, 750 | 0..639, 640 (read) |

(Hatar values in brackets follow from §3's formula; they are only set once a 3D screen has been entered, so two of
the probes caught them.)

## 3. The 3D range: `ModifyViewPort` (exe 0x4AC4F0, ELF 0x4F6DF0)
`m = ((WIN_W·480)/WIN_H − 640)/2` virtual pixels per side; `HatarX1 = −m`, `HatarX2 = 639 + m`, `HatarY1..Y2 =
0..479`, `HatarXm = 640 + 2m`, `HatarYm = 480` (exe 0xB9A980..0xB9A994). Every 3D screen sets this range in its
init (`Starmap_Belep`, `Colony_Belep`, `Research3DInit` exe 0x51F7C0, `Infoing3DInit` 0x4EA8A0, `Messages3DInit`,
`Spying3DInit`, `Diplomacy3DInit`, `Design3DInit`, `Trade3DInit`, `Options3DInit`, `Help3DInit`, `SpwC::initgfx`)
and after drawing a sub-viewport (`DrawRadar`, `GFX_StarmapRadar`, `ColonyGfx2D_2`, `StarmapGfx2D_3`).
- The engine projects every vertex itself to virtual pixels around (320, 240) with its fixed focal length (600 on
  the 480 lines): **the vertical field of view never changes; a wider window shows more of the scene to the
  sides**, and every 3D object keeps its virtual position. `Hatar*` is the engine's clip and cull range; the D3D
  viewport call is a no-op in the port (`_IDirect3DViewport3::SetViewport2` ELF 0x4FAFE0 returns 0, the exe inlines
  it away) and GL clips at the full-width viewport (the captures have no 1–2 px seam where `m` rounds down).
- **Background pictures stretch:** `Copy3hatterkep` (ELF 0x534AC0) draws the screen's picture (`Kepek[n]`) over
  `HatarX1..X2 × 0..479`, stretched horizontally across the whole window. Called from `Research3DUpdate`,
  `Infoing3DUpdate`, `Messages3DUpdate`, `Spying3DUpdate`, `Diplomacy3DUpdate`, `Design3DUpdate`,
  `Trade3DUpdate`, `GFX_Options`, `GFX_Help`.

What the captures show (`_local/traces-widescreen/`, `k<kep>-<W>x<H>-0.png`): the 4:3 centre of every widescreen
capture matches the 800 × 600 capture at offset 0 (mean |diff| 0.7–4.3 grey levels, animation only). Starmap: the
3D map, the stars and the sky clouds fill the full width; the top bar, the control panel and the info box stay in
the frame. Colony: the terrain and buildings fill the full width. Research: the background box's grid reaches into
the sides (dark). Info: the sides stay black (nothing in the scene or picture there). Messages
(`_local/traces-widescreen/k10-1920x1080.png`, 2026-10-18): the frame models stop at the 4:3 frame's edge and the
side strips show a stretched picture (re/deviations.md 4).

## 4. Input and other users of the frame
- **Mouse:** `MainCallBackEventHandler` (ELF 0x579B70) maps the physical position through the virtual frame:
  `x = (px − VIRTUAL_POSX)·640 / VIRTUAL_WIDTH`, `y = (py − VIRTUAL_POSY)·480 / VIRTUAL_HEIGHT`, **unclamped**
  (negative or past 639 in the side strips; hot-spots there don't exist, so clicks hit nothing or the 3D scene).
  `ig2_get_real_mouse_x/y` (ELF 0x4283A0) are the same for touch.
- **Edge scrolling** uses the physical window: `Starmap_Autoscroll` (ELF 0x6FE890) scrolls while the pointer is
  within 8 window pixels of the window's edge (`sg3d::m_window_width/height`), not the frame's; the colony's and the
  space battle's autoscroll read the same globals.
- **The cursor** is drawn over the whole window (`ig2_render_mouse_pointer` ELF 0x550410: viewport `0, 0, W, H`,
  the same 4:3-frame mapping), so it can move into the side strips.
- **Videos** (`ig2screen::openAnim` ELF 0x425620) are fitted to the virtual frame (`IG2_VIRTUAL_*`).

## 5. The mobile toolbar (`toolbar=1`, off by default)
`IG2_TOOLBAR` (exe 0xFE6532) takes `H·52/768` pixels off each side of the window before the rule above
(`IG2_WINDOW_POSX` = that width) and draws the touch sidebars there (`ig2_toolbar_resize` ELF 0x5515B0,
`retina_*` bitmaps from 1680 px wide). A mobile leftover; not planned for the client.

## 6. The client
`ig2-client::kit` (the shared screen kit, 2026-10-18) holds the rule; every screen draws through it:
- `kit::Layout` = `ig2_resize` (§2, its f32 aspect test and integer rounding), `hatar()` = `ModifyViewPort` (§3),
  `mouse()` = `MainCallBackEventHandler`'s unclamped integer mapping (§4), `band()` = the GL viewport from the top
  (`glViewport` counts y from the bottom, so an odd remainder puts the extra pixel row above the band). Tested with
  §2's six observed sizes.
- `kit::Virtual` (re-exported as `font::Virtual`): `ig2_render_first`'s float mapping (`k = WIN_H/480`, the frame
  centred) of the viewport; `virtual_of` is the exe's integer mouse mapping; `x_range()` = `HatarX1..X2` (text
  flag 8). The cameras (`starmap`, `colony`) and every former `k = size[1]/480` site go through it.
- Narrower than 4:3: the screens draw in the band (`W × W·480/640`) as if it were the window; the window
  (`window::Gpu::present`) and the snapshots (`assets::write_snapshot`, `kit::layout::compose`) put it on the window
  with black bars above and below; the pointer is taken relative to `IG2_VIRTUAL_POSY`.
- `kit::Wide`: the offscreen 3D targets (`research.rs` models, `starmap_panels.rs`, `colony_ui.rs`,
  `diplomacy_scene.rs`) span the viewport, their clip matrices' x row divided by the width over the frame's. The
  info/messages frame models (`screen_frames.rs`) keep a 4:3 target (`Wide::frame`): ig2.exe cuts them at the
  frame's edge (§3, Messages).
- The "letterbox" option (`--letterbox`, off by default, ADR 0010 §2): black over the side strips at the end of the
  frame (`kit::layout::letterbox_bars`), and the `Wide` targets stay 4:3.
- Also in the kit: the faders (`kit::fader`: `FadeKerettelKerekit`, `KirakFader`, `fadertexture`), the hot-spot
  tooltip (`kit::Tooltip`), the row frame `DesignKeret1` (`kit::DesignFrame`), the hot-spot test `kit::inside`.
- Checked (2026-10-18): 800 × 600 snapshots of nine screens are pixel-identical to the client before the kit; at
  1920 × 1080 they differ by at most 1 grey level (the reformulated projections) and a 1280 × 1024 snapshot is the
  1280 × 960 one between black bars.
- Open: edge scrolling at the window's edges (§4) when the autoscroll is ported; the side-strip pictures
  (`Copy3hatterkep`, re/deviations.md 4).
