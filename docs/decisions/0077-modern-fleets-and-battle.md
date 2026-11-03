# ADR 0077: the fleet panels, the planet settings menus and the space battle on the design kit

Date: 2026-11-03. Status: accepted (the fleet info and transfer panels, the settings menus). Amends ADR 0065 and ADR
0079 (the modern starmap HUD) on the design kit of ADR 0073. Implemented by `ig2-client::fleet_modern` and the
window's pointer (`window.rs`: `starmap_virtual`, `fleet_modern_up`), `starmap_modern` (the settings menus).

## Context
The design system's fleet boards ("Fleet info over the starmap", "Transfer between two fleets", the four "Select
menu" boards, 1920 × 1080) draw the starmap's fleet info panel (re/screens/starmap.md §19), its transfer panel
(§13) and the long info's settings menus in the kit's language. Under the modern starmap HUD the fleet and transfer
panels were still the original's 640 × 480 panels with their faders, texts and camera-fixed unit models; the
settings menus dropped under their field with no mark of the value set.

## Decision
1. **Fleet info panel** (the board's `.ig-panel`, 1320 × 700, centred, 28 under the bar): a lit header (the fleet's
   name, the owner glowing in its colour, the status line of the selection card upper case and muted), the units as
   cards three across (a recessed well with the unit's model, its count in an LCD window, the design's name; the
   original's highlighted row in the list's selected look, the pointer's with the bright rim), the selected design on
   the right (its model on the void, the name, the original's data lines as a table, the pod slots as a recessed list
   with the empty ones muted), the vehicle capacity as a bar and value in the footer, the exit plate. The original's
   eight places stay: two rows of the board's 196-point cards while six or fewer are shown, three sharing the height
   for seven or eight, a thin scroll bar when the fleet has more designs.
2. **Transfer panel**: the two holders as the board's two 620-wide panels 80 apart (left `f1`, right `f2`), the same
   list of designs on both, each card with its count in an LCD window and a round button pointing to the other side,
   the count in the warning colour where the original's is (vehicles over a full fleet), each side's capacity in its
   footer, the exit plate on the right one.
3. **The models are the original's.** The camera-fixed render target the original panels draw
   (`starmap_panels::PanelModels::render`: the cells' turning units, the fleet's turning preview, the transfer's two
   sides, an empty side's model three quarters transparent) is still made; its regions are cut into the cards' wells
   and the preview area (`fleet_modern::pictures`, in the window's top frame after the panels, as ADR 0074's wells).
   While the sale menu is open the wells under it stay empty (the models are drawn after the panels).
4. **Same functions.** The panels take no input: the window hands the pointer to the original's panel code moved
   where the panels drew their controls (`fleet_modern::State::pointer`, used by `starmap_virtual` for the press, the
   right press, the 100 Hz hold frames, the hover and the tooltips): over a card the middle of the original's cell,
   over the exit plate hot-spot 9, along the scroll bar the original's bar at that height, over the sale menu the
   original's row, over the rest of the panels (and the original panels' old place) the panel's empty part (hot-spot
   0x58 takes the press). So the press-and-hold sale menu (its rows, its prices, 0x34 and 0x35 on the release), the
   held transfer cells (one more each step with the left button, all with the right), the scroll in row pairs, the
   arrow keys, the exit and the multiplayer orders are the original's. The sale menu is drawn at the pointer of the
   press as a deep glass list, the row under the pointer lit, a row that can't be had muted.
5. **The settings menus** (tax, autobuild, priority): the list opens over its field with the field's value on the
   field (a native list box), the value in the list's selected look, the row under the pointer with the bright rim
   fading in; the long info is the board's 500 wide, the ALL plates 64. The planet list keeps lighting the pointer's
   row. The hot-spot remap of ADR 0079 §4 follows the moved list.
6. **Text**: every label through the kit's fitted text in a fixed box (cut with "…" and its whole text as a tooltip,
   or shrunk to a floor); long or translated names never widen a card, a panel or a row. The game's strings
   throughout; the one new label, "Equipment" over the pod list, is ours (ADR 0042).

## Consequences
- The vehicles' info panel (`StarmapTanksInfo`, §20) stays the original's under the modern HUD.
- The minimap and the console stay off under the fleet and transfer panels, as in the original; the boards show the
  navigation plate.
- The board's "1 tank doesn't fit" line and the long info's colony plate have no original function and are not
  drawn.
- Classic and every capture and replay are unchanged: the panels are the modern HUD's (its option and `--modern-hud`;
  `--panel fleet --select-fleet F` and `--panel transfer:F1,F2` stage them in snapshots).
