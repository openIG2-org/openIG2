# ADR 0073: the design kit of the modern UI

Date: 2026-11-03. Status: accepted. Amends ADR 0062 (style, fonts, shared widgets) and ADR 0056 (the built-in Modern
theme's colours). Implemented by `ig2-client::gui::ds` (`gui/ds.rs`: tokens, type scale, painter helpers;
`gui/ds/fit.rs`: text that fits; `gui/ds/widgets.rs`: the widgets; `gui/ds/cursor.rs`: the pointer set;
`gui/ds/gallery.rs`: the gallery), `gui.rs` (the egui style and the shared page widgets on the kit), `ui.rs`
(`Hud::draw_pointer_phase`: the modern pointer) and the fonts in `engine/crates/ig2-client/fonts/`.
`ig2-client --ui-gallery <out.png> [--size WxH]` draws every widget in every state.

## Context
Every original screen is getting a modern egui version (ADR 0062, 0065-0072). Each screen so far styled its own
panels from the theme's few colours, so the pages differed in corners, spacing, type and states, and long translated
labels could push a panel wider or overflow it. A design system was drawn for the modern UI (screen boards for every
screen at 1920 × 1080, one token set and component sheet: glass panels with a rim and a lit top edge, bevelled plates,
LCD readouts, metal keys, the Oxanium and Share Tech Mono faces). The screen pages need one kit that draws those parts
with all their states, so that every screen built on it looks the same and fits its text.

## Decision
1. **Tokens** (`ds::Tokens`): the design's colours by role (glass, rims, plate shades, metal, rust, LCD, inks, info,
   own, enemy, signal red, focus), spacing steps `SPACE` (4, 8, 16, 24, 40), radii (`RADIUS_PANEL` 8, `RADIUS_PLATE`
   4). `Tokens::DEEP_SPACE` is the palette; `Tokens::from_theme` maps the UI theme onto the roles it has keys for
   (panel → glass, panel edge → bright rim, button edge → rim, button / tab / selected tab → the plate's shades,
   text / dim / off → the inks, accent → info, hover → focus) and derives deep and raised glass and the silver ink by
   the palette's ratios, so a mod's theme recolours the kit; metal, rust, LCD and the empire colours stay the
   design's. The built-in Modern theme (`themes/modern.toml`) is now the deep-space palette, exactly
   (`from_theme(modern) == DEEP_SPACE`). `gui::style(theme)` sets the tokens for the frame (`ds::tokens()`).
2. **Type** (`ds::Font`): the design's scale as constants with size, line box, weight, letter spacing (em, egui's
   `extra_letter_spacing`) and upper case: `TITLE_PLATE` 28/32 500 0.12 em, `HEADING` 20/26 600 0.08, `BODY` 17/26
   400 0.06, `LABEL` 13/18 600 0.14 upper, `DATA` 15/20 500 0.10, `READOUT` 24/28 Share Tech Mono 0.12 slanted,
   `PAGE_TITLE` 30/36 600 0.30 upper; `sized`, `weight`, `tracking`, `upper` derive the boards' variants. **Fonts**:
   Oxanium (Regular, Medium, SemiBold) and Share Tech Mono, SIL Open Font License 1.1, bundled in the client with
   their licence files (`fonts/OFL-*.txt`); each weight is its own egui family and Oxanium Regular is egui's
   proportional face, so egui's own labels use it. The install's `jurabook.ttf` and egui's bundled fonts stay behind
   them for letters they lack (Hungarian ő and ű are in Oxanium; the test checks the languages' letters). The design's
   italic LCD face is Share Tech Mono drawn sheared (egui has no italic: the galley is tessellated and its vertices
   slanted, `ds::sheared_galley`).
3. **Text that fits** (`ds::Text`, `ds::text`): a label in a fixed box never sizes the box. One line: on the box's
   baseline (its capitals centred; rows of one height and style share a baseline, `fit::baseline`, and a row of mixed
   styles passes one baseline to all), cut with "…" (egui's elision) or shrunk in half points down to a floor first
   (`shrink(min)`), the whole text as a tooltip where it is cut; left, centred or right. Wrapped body text: `lines(n)`
   caps the lines, the last cut with "…". `Text::show` allocates exactly the given size; `paint` draws into a rectangle
   without allocating. Every kit widget draws its labels this way.
4. **Widgets** (`ds::widgets`, re-exported by `ds`), each with idle, hover, pressed, selected, disabled and keyboard
   focus (`focused(true)` for a page with its own navigation, ADR 0062 §4): `Plate` (the command plate; `primary`,
   `sub`), `menu` with `MenuStyle::MENU` / `RAIL` (main menu items, a page's tab rail; the selected plate glides),
   `tabs` (a tab strip; too many share the width), `IconKey` with `Icon` or `Glyph` and `key_group` (the metal keys,
   speed keys lit in LCD ink), `round_button`, `ListRow` and `row_back` (list rows, a page's own rows), `option_row`
   (a settings row: label left, control right, the label cut to the room left), `toggle` (two words), `segmented`,
   `stepper`, `slider`, `key_cap`, `lcd`, `bar` (progress and charge), `badge` with `Tone`, `kv` (a data table line),
   `title_plate`, `separator`, `heading_label`, `panel` / `paint_panel` with `PanelKind` (glass, deep, raised, frame),
   `header_strip`, `card`, `tip` (a wrapped tooltip), `dropdown`, `confirm` / `confirm_box` (a modal). The painter
   helpers draw the CSS effects for a page's own parts: `vgrad`, `hgrad`, `radial` (gradients as textured rounded
   rectangles, made once per gradient), `rim`, `edge_line` (the lit or dark one-point insets), `shadow` (soft outer
   shadows and glows), `relief`, `plate_fill`, `glass`, `lcd_window`, `focus_mark`, `glowing_galley`, `behind` (paint a
   container's background after its content, under it).
5. **Motion**: hover fades over 120 ms (`hover_t`), selections glide over 160 ms (menus, tabs, segments, switches,
   bars), panels open with a fade and a 12-point slide over 180 ms (`open_t`; `gui::page` and `confirm`). `set_motion
   (false)` makes every change instant: `--snapshot` and the gallery do, so stills stay deterministic; an
   accessibility "reduce motion" setting can use the same switch (Options has none today).
6. **Pointers** (`ds::cursor`): the original's pointer types (`MOUSE.BMP`'s eight columns, re/screens/input.md §8:
   arrow, clickable, move, build, destination, select, attack, target) and the pages' extras (busy, text, forbidden,
   drag) as one set drawn from geometry (signed distances, anti-aliased, a dark outline and a soft shade, the
   design's colours), the animated ones (move, select, busy) in 20 frames over the original's 2-second cycle, 64
   pixels across, 32 points at 100 % UI scale. No picture files. They replace the game's pointer quad (drawn last,
   where `MOUSE.BMP` was) when a modern UI theme is chosen and the new Remake option "Modern cursors"
   (`ModernCursors`, Interface, on by default) is on; Classic keeps `MOUSE.BMP`. An egui page's cursor icon becomes
   the overlay's pointer type (`Gui::pointer_kind`: the game's 0..7, 100 on for the extras, which Classic shows as the
   arrow or the original's clickable pointer).
7. **The shared page widgets on the kit**: `gui::style` (deep glass windows, rims, plate-blue buttons, the lit plate
   for what is on, the bright rim under the pointer, 4- and 8-point corners, the design's faces at the pages' existing
   sizes), `page` (glass with the circuit grid, opening fade and slide), `card` (= `ds::card`), `switch` (a well with a
   gliding lit plate), `primary` / `secondary` (plates at the pages' 32-point height), `focus_ring` (the design's
   focus outline). The already modern pages (Options, the multiplayer pages, the starmap HUD, colony, production and
   research, info) pick the look up without layout changes.
8. **egui reaches the boards.** Every effect the boards use has an egui form: linear and radial gradients (a small
   gradient texture filling a rounded rectangle: anti-aliased corners), inset rims and lit edges (strokes), soft
   outer shadows and glows (epaint's shadow), text glow (offset copies), letter spacing, the slanted LCD face, opacity
   for fades. The boards use no backdrop blur; CSS `filter: brightness` on a picture is a tint. Nothing calls for
   another framework.

## API guide for the screen pages
| Board part | Kit call |
|---|---|
| Panel over a scene (`.ig-panel`, `.deep`) | `ds::panel(ui, PanelKind::Deep, 0.88, 16.0, …)`, or `paint_panel` in a placed rectangle |
| Section heading (`h3`, `.label`) | `ds::heading_label`, or `Text::new(s, Font::LABEL)` |
| Command button (`.ig-plate`) | `ui.add(Plate::new(s).width(180.0))`, `.primary(true)` for the main action |
| Main menu, side rail (`.mm-item`, `.rail`) | `ds::menu(ui, id, MenuStyle::MENU, &entries, selected, focus)` |
| Tab strip (`.tab`) | `ds::tabs(ui, id, &labels, selected)` |
| Settings row (`.orow`) and its controls | `ds::option_row(ui, label, focused, enabled, \|ui\| toggle / segmented / stepper / slider / key_cap)` |
| Top bar keys, speed keys (`.ig-key`, `.ig-speed`) | `ds::key_group(ui, \|ui\| ui.add(IconKey::icon(Icon::Pause).on(true)))` |
| LCD readout (`.ig-lcd`) | `ds::lcd(ui, "$109090", 22.0, 7)` |
| List row (`.ig-list li`), a ship card's selected look | `ui.add(ListRow::new(s).value(v).selected(b))`, `ds::row_back` |
| Data table (`.ig-table`) | `ds::kv(ui, label, value, Tone::Info, 130.0)` |
| Empire label (`.ig-star`) | `ds::badge(ui, name, Tone::Own, false, max_width)` |
| Progress bar | `ds::bar(ui, w, 6.0, fraction, tokens().lcd_ink)` |
| Title plate (`.ig-title-plate`) | `ds::title_plate(ui, s, width)` |
| Round rust button (`.ig-round`) | `ds::round_button(ui, Glyph::Text(s), small, active)` |
| Confirm dialogue | `ds::confirm(ctx, id, open, title, body, yes, no)` |
| Any label in a fixed box | `Text::new(s, font).right().shrink(12.0).paint(ui, rect)`; never size a box by its text |
| Pointer over a page part | set egui's cursor icon (`ui.ctx().set_cursor_icon(CursorIcon::Crosshair)`): the kit shows its pointer |

## Consequences
- The Modern theme's colours change on every screen that draws with it (the kit's faders on the remake's 640 × 480
  screens included); Classic and the original screens are unchanged, so capture comparisons and replay tests don't
  move. A mod's `theme.toml` keeps working and now also recolours the kit.
- The client carries about 130 KB of fonts under the SIL OFL (allowed to be bundled with software; the licence files
  go with them).
- Screen pages build on `ds` and run `Text` for every label; a board's measurements are points one for one at 100 %
  UI scale.

## Amendment: the hand pointer (2026-11-03)
`look()` makes every enabled kit control (plates, keys, round buttons, list rows) show the hand pointer on hover, as the original does over its hot-spots (re/screens/input.md §8). The round −/+ buttons are not red-tinted like the board's.
