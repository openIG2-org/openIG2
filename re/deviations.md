# Deviations ledger

Every known difference between `ig2-client`/`ig2-sim` and the original GOG build, and every original bug we come
across. During the prototype (roadmap M0–M5) this list only grows: fix a difference only when it blocks play. In
M6 every entry gets a decision: **fix** (match the original), **enhanced** (an opt-in option, with an ADR), or
**keep** (the original's behaviour stays, documented).

Tags: `input` (keys, mouse, gestures), `layout` (position, size, widescreen), `render` (colour, filtering,
effects), `timing` (animation, pacing), `sound`, `feature` (missing or extra behaviour), `bug` (an original bug,
kept for fidelity unless decided otherwise). Original bugs the sim keeps on purpose for bit-exactness are already
listed in the specs ("quirks kept"); add one here only when a player would notice it.

| # | Tag | Where | What differs (ours vs the original) | Evidence | Decision |
|---|---|---|---|---|---|
| 1 | layout | all screens | the original remaster anchors/stretches some elements at 16:9; we letterbox the 800 × 600 layout | owner, 2026-10-18; roadmap §4; `re/screens/widescreen.md` | **fix** (ADR 0010, 2026-10-18): the remaster's rule is the default; at 16:9/21:9 the client already matched (2D in the 4:3 frame, 3D full width). The screen kit (`ig2-client::kit`, 2026-10-18) closed the rest: the exe-exact layout (`kit::Layout`), windows narrower than 4:3 letterboxed top and bottom, the offscreen 3D targets as wide as the window, the `--letterbox` option. **Closed** |
| 2 | input | all screens | inputs differ from the original in places (not yet listed per screen) | owner, 2026-10-18 | open (M4 audit; add entries as found) |
| 3 | feature | client start | the client starts from a save; the original starts at the title and main menu | `re/screens/README.md` §4 | open (M2 MainScr) |
| 4 | render | info, messages at 16:9 | the side strips beside the 4:3 frame stay black; ig2.exe shows a picture there (`Copy3hatterkep(0x34)`'s, stretched) | `_local/traces-widescreen/k10-1920x1080.png` (2026-10-18) | open (with the screens' `Copy3hatterkep` pictures) |
| 5 | bug | campaign story 834 | the original's (and ours, kept) siege fleets of empire 1 appear at planet −1's record, (0, 0), and fly from there to planet 8340 (`st_fNewEx(…, 2, −1)`) | `re/rules/campaign/elem.md` §8.14 part 2e | open |
| 6 | input | research screen | PgDn: ours goes to the next category in `Terulet_Melyik` order (turrets 11 → radars 3), the exe to the previous one (turrets 11 → tanks 10); PgUp likely the reverse too | sequence diff `research.json` t 400→420 (exe `ResearchTerulet` 11 → 10), 2026-10-22; re/screens/sound.md §4 | open |
| 7 | input | research screen | Esc: ours closes the screen with the closing animation; the exe opens screen 13 at once (no animation, no sound) | script run 2026-10-22 (`AktKep` 4 → 13 within 3 ticks of Esc) | open |
| 8 | input | research screen | the top bar's research spot (600, 0, 40 × 20) closes ours; a press there does nothing in the exe | script run 2026-10-22 (press at (620, 10): the exe stays open) | open |
| 9 | timing | research screen | the opening runs one UI tick behind: ours starts `resframe` after the first tick (`started` set by the first drawn frame's step), the exe from the first tick (4.0 at tick 10 vs our 3.6; open at tick 150 vs 151) | sequence diff 2026-10-22, re/screens/sound.md §4 | open |
| 10 | timing | research screen | the urgent button's tooltip shows 20 ticks after the pointer arrives in the exe (t 320), later in ours (the kit's 400 ms rest) | sequence diff 2026-10-22 (t 320, 340) | open |
| 11 | render | all screens (snapshots) | the exe draws the pointer in its frame; our snapshots draw none | sequence diff 2026-10-22 (t 320) | open |
| 12 | sound | all screens | no sounds yet in ours (the exe's per-action ids: re/screens/sound.md §1, §4) | sound tracer 2026-10-22 | open |
