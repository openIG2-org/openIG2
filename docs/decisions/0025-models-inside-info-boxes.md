# ADR 0025: The info boxes' turning models stay inside their box

Date: 2026-10-31. Status: accepted. A deliberate deviation from the classic client (ADR 0009), with the original's
look kept as an option. Spec: `re/screens/starmap.md` §11 and `re/screens/spacewar.md` §5.2. Implemented by
`ig2-client::starmap_hud`, `ig2-client::spacewar_screen` and `ig2-client::display`.

## Context
The bottom-left info box (`[5, 423, 162, 52]`) shows a turning model: on the starmap the selected planet's globe or
the selected fleet's flag ship (`StarmapFrissitBolygoUrhajok` ELF 0x6d3d40), in a space battle the selection's ship
(`kisinfogfxfrissit` exe 0x5778c0). Each is a camera-fixed 3D object at scale 0.03 turned about the vertical axis
(`matrixforgat(m, m, 0.05, 0)`); the engine clips it only to the whole screen (`HatarX1..Y2`,
`re/screens/widescreen.md` §3), never to the box. The battle's model sits at x −545, so a long hull reaches past the
box's left edge as it turns [obs: the exe's battle grabs `grab3-t600`, `grab4-t800`; the starmap's at −535 stays
inside]. Players read this as a drawing error.

## Decision
1. **Option "Models inside boxes", on by default.** The info box's turning model is cut to the box's rectangle
   `[5, 423, 167, 475]` (virtual) when it is composited: on the starmap (globe and fleet) and in the space battle.
2. **Classic mode** (`BoxModelsInside` 0 in the remake's settings file, or the Remake row on the Options screen's game
   page): the exe's drawing, clipped only by the screen.
3. Nothing else changes: the models' size, position, turn and light are the original's in both modes.

## Consequences
- Capture comparisons and `--snapshot` draw classic (the option is read by the window only).
- The colony screen's unit and building picture is already cut to its panel corner (`HandleKisInfoObject`'s part of
  the panel pass), so it needs no change.
