# ADR 0027: The space battle's defend/attack slider sits under its label

Date: 2026-10-31. Status: accepted. A deliberate deviation from the classic client (ADR 0009), with the original's
look kept as an option. Spec: `re/screens/spacewar.md` §5.2 (small box, case C). Implemented by
`ig2-client::spacewar_hud`, `ig2-client::spacewar_ui` and `ig2-client::display`.

## Context
With nothing selected, the space battle's small info box (`kisinfokiir` exe 0x577c00) ends with the fighters'
defend/attack slider: the bar `DDFillToScreen3dtRGB(x, y + 0x2a, 0x71, 3, ..)`, its marker (3 × 7 at y + 0x28) and
then the label `GetString(0xb, 1, 0x14)` at y + 0x29, drawn last. The label is one 19-character string with the two
words at its ends ("defend       attack", Hungarian "védekezés   támadás"; the text file says the second word is
right-aligned to the 19th position), 114 px wide at size 1, so the bar runs through both words [obs: the exe's
`spacewar_slider` capture, Hungarian, stance 0 and 100; the client matches it]. Players read this as a drawing error.

## Decision
1. **Option "Slider under label", on by default.** The label moves up 3 px (y + 38) and the bar to y + 46, under it;
   the marker keeps its size and its place around the bar. The slider's hot-spot (`[50, 461, 113, 9]`) grows to
   14 rows so it reaches the bar.
2. **Classic mode** (`SliderUnderLabel` 0 in the remake's settings file, or the Remake row on the Options screen's
   game page): the exe's drawing and hot-spot.
3. Nothing else changes: the bar's colours, the marker, the label's text, font and colour are the original's.

## Consequences
- Capture comparisons and `--snapshot` draw classic (the option is read by the window only).
- The label touches the fighters' row above it (no blank row between them), and the marker's top row overlaps the
  label's bottom row where it stands; the label is drawn last, as in the original.
