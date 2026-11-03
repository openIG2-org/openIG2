# ADR 0062: egui for the remake's own pages

Date: 2026-11-01. Status: accepted (part 1: the layer and the main-menu Options page; part 2, the multiplayer pages:
ADR 0066; part 3, the starmap HUD: ADR 0065; part 4, the in-game menu: ADR 0067). Amended by ADR 0073 (the style, the fonts and the shared widgets) and ADR 0078 (the Options page in the boards' five tabs of cards). Amends ADR 0006 §7 (egui was
for developer overlays only). Implemented by `ig2-client::gui` (the layer, the style, the shared widgets),
`options::modern` (the page) and the window's frame and input (`window.rs`: `Gpu::present_over`, the pointer
routing); the snapshot's `--script` drives the page too (`mainscr_scene::sequence`).

## Context
The decision of 2026-11-01 (`docs/ENHANCEMENTS.md` → Decisions): the remake's own screens move to a real Rust UI
framework, egui, with designed pages; first the main-menu Options screen (ADR 0053), then the multiplayer pages,
then a modern starmap HUD. Until now those screens were drawn with our own primitives (the kit's faders, filled
quads, the game's bitmap font) in the 640 × 480 virtual pixels: every switch, slider, list and scroll was hand-made,
and the result looked like the original's boxes with new colours. The game's own screens are not touched: they
keep matching the exe (ADR 0009).

## Decision
1. **egui 0.32** (`egui`, `egui-wgpu`): the versions on wgpu 25, the same the asset viewer already builds with
   (eframe 0.32), so the lock file gains no second wgpu. It builds for the browser (ADR 0047) unchanged: WebGPU or
   WebGL2 through our wgpu.
2. **In the frame**: a page runs once a frame (`Gui::run`, immediate mode) after the screen's 3D and 2D are built;
   `Gpu::present_over` draws our frame, then egui's triangles into the same target (`LoadOp::Load`), then the game's
   pointer as a separate quad frame (`Frame::keep`) on top. In a window narrower than 4:3 all three go into the
   band's target before it is put on the window, so the page sits in the band like every screen.
3. **Scale**: a page is laid out in points; at 100 % the window is 1080 points high (`gui::POINTS_HIGH`), so a
   1080-line window draws a point as one pixel, and the points grow with the window height as the 640 × 480
   screens do (1.33 px at 1440p, 2 px at 4K). The "UI scale" option (`UiScale`, Remake row under Interface, 50–200 % in tens, 100 % by
   default; `--ui-scale P`) multiplies it: `Gui::scale` = band height ÷ 1080 × per cent ÷ 100, read by every
   page's and the modern starmap HUD's next frame, so a change shows at once without a restart. On the modern
   Options page the row is a slider whose value previews when it is set (a drag resizes the page when the drag
   ends, so the track stays under the pointer); Back puts the applied value back, Apply stores it. The window's DPI
   factor plays no part (the band is in physical pixels). egui tessellates at the window's pixels, so text and edges
   stay sharp at 1440p and 4K without "Sharp UI". The original screens keep filling the window (their 640 × 480
   layout is the exe's); the option sizes only the remake's egui layer. Until 2026-11-01 a page was 540 points high
   (two pixels a point at 1080p, now 200 %), which made every page and the HUD too large at 1080p and above.
4. **Input**: while a page is up it owns the pointer: the window turns its moves, buttons and wheel (band pixels)
   into egui events (`Gui::pointer`, `button`, `wheel`), and the screen under it sees none of them; the right button
   also stays the menu's Back (ADR 0059). The keyboard stays with the page's own navigation (the Options screen's
   `Nav`: arrows, Tab and Page Up/Down, Enter, Space, Esc, the name entry, the key capture), so a key never acts
   twice through egui's focus moves. The events are ours, not egui-winit's: the same calls take a script's input
   in `--snapshot --script` and a test's. A page that needs egui's own text fields (the multiplayer chat) adds
   keyboard and text events the same way, only while the field has the focus (`Context::wants_keyboard_input`).
5. **Fonts**: the remaster's sans from the user's install (`fonts/jurabook.ttf`, an `ecGs` container, read
   through the install like every file) when it is there; egui's bundled fonts (compiled in from its crate, free
   licences, Latin, Latin Extended-A and more) behind it for any letter it lacks, and alone without it. Nothing is
   added to the repository; the game's bitmap font is not used on these pages (it has no sizes between its fixed
   ones and no smooth scaling).
6. **Style**: the UI theme (ADR 0056) is the egui style (`gui::style`): the panel colour as a dark glass window
   (slightly see-through, 16-point corners, a soft shadow over a dimmed screen), the tab and button colours for
   the widgets, the accent for what is on or chosen (switch tracks, sliders' fill, the selected tab's bar, card
   headings), the hover colour on what is under the pointer, the dimmed and disabled text colours, a type scale
   (title 26, text 15, descriptions 12.5 points). Classic and the mods' themes map the same way.
7. **Shared widgets** (`gui`): `page` (the dimmed backdrop and the centred glass panel), `card` (a titled group),
   `switch` (an animated on/off), `primary` and `secondary` (a page's footer buttons), `focus_ring` (the keyboard's
   mark). egui's own `ComboBox`, `Slider`, `Button`, `ScrollArea` and `Label` complete them.
8. **The Options page** (ADR 0053 on egui): the same eight tabs, settings, values, Apply/Back and storage; a sidebar
   of tabs, the settings in cards by group (the remake row's group, the exe's graphics and game rows, the keys by
   where they act), each with its label and its description under it and its control at the right: a switch, a
   drop-down of its values, a slider with its value, the name field, a key button. The list scrolls (wheel,
   scrollbar; no drag scrolling, so a quick move and press can't scroll it). Status messages (waiting for a key, a
   shared key, the language's restart) sit in the footer. The keyboard focus is marked and kept in view.

## Consequences
- The multiplayer pages and the starmap HUD (later parts) build on `gui::page`, `card`, `switch`, `primary`,
  `secondary`, `focus_ring` and `gui::style`, run through `Gui::run` and drawn by `Gpu::present_over`; a page
  over the game frames calls the same two.
- The in-game Options screen and every original screen are unchanged; their capture comparisons don't move.
- A page's look has no exe capture: screenshots go with the commit (`--snapshot --screen main --script`).
