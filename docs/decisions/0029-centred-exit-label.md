# ADR 0029: The starmap panels' Exit label is centred on its button

Date: 2026-10-31. Status: accepted. A deliberate deviation from the classic client (ADR 0009), with the original's
look kept as an option. Spec: `re/screens/starmap.md` §12.2. Implemented by `ig2-client::starmap_panels`.

## Context
The production view, the transfer panel and the fleet panel share an Exit button at the bottom right: the box
`StarmapGfx2D_2` draws at x, 380, `addo + 0xb0` wide (`addo` +11 production, −10 transfer, 0 fleet), and the label
`GetString(2, 4, 4)` that `StarmapGfx2D_3` (exe 0x5aaba0) draws at x + 30, 390 in a 13-cell field (size 3,
field-centred). The field (13 × 8 = 104) is centred on x + 82, not on the box's middle, so the label sits left of
centre: 11 virtual pixels in the production view (box 187 wide) [obs: Hungarian capture of ig2.exe at 800 × 600,
"Kilépés" ink 69 px from the button's left edge, 97 px from its right; the client draws the same pixels]. Players
read this as a layout error.

## Decision
1. **Option "Centred Exit label", on by default.** The label's field starts at `x + (addo + 0xb0 − 13·CharXMeret[3])
   / 2`, so it is centred on the box in all three panels.
2. **Classic mode** (`CentredExitLabel` 0 in the remake's settings file, or the Remake row on the Options screen's
   game page): the exe's x + 30.

## Consequences
- Capture comparisons and `--snapshot` draw classic (the option is read by the window only).
