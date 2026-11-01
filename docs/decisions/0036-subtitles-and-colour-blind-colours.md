# ADR 0036: Subtitles and colour-blind empire colours

Date: 2026-11-01. Status: accepted. Two deliberate deviations from the classic client (ADR 0009), both off by
default (= the original). Spec: `re/screens/audio.md` §5 and §7, `re/screens/starmap.md` §7, `re/screens/options.md`
§14. Implemented by `ig2-client::ui` (`Hud::subtitle`, `wrap`), `ig2-client::window` (`audio_frame`,
`reload_palettes`), `ig2-client::starmap` (`COLOUR_BLIND`, `read_palette`, the fleet sprites) and
`ig2-client::display`.

## Context
Players who can't hear the voices or can't tell some empire colours apart lose information the original gives only
by sound or by hue.

What the install has for subtitles (searched: `anim/mpeg4/`, `dataraw/text/`, `dataraw/uzenetek/`, every
`localization/<lang>/` folder):
- **Videos** (story cut-scenes, intros, endings, the title): no transcript, no subtitle track, no timing. Nothing to
  show without shipping our own texts, which the engine never does (ADR 0004).
- **Campaign start voice** (`sounds/start/faj%dstart`): no transcript.
- **Unit voices** (the starmap's and colony's speakers): no transcript.
- **Message-screen voices** (`messagesounds/<EN>/%04d_%02d.ogg`): their words are the message part the screen
  already types (`txt%04d.txt`, the `TE` part).
- **Scroll-line voices** (`sounds/scroll/<human|brutal>/%04d`, `FRISSIT_Scroll2` exe 0x556210): their text is the
  message the top line scrolls through; it moves in from the right, stops for four seconds and leaves, in a 53-character
  window at the top of the screen.

The empire colours are `DDColor[e + 15]` (exe 0x8F8710): names, the minimap, the info lists and graphs, the top bar's
texts; the starmap's fleets are the `star_pl` sprite sheet's column `e − 1`, already coloured. Simulated with the
Machado, Oliveira and Fernandes (2009) dichromacy matrices, empires 2 (orange) and 6 (green) come to 0.056 apart in
sRGB under protanopia, 4 (magenta) and 5 (light blue) 0.061 under deuteranopia.

## Decision
1. **Option "Subtitles"** (`Subtitles` in the remake's settings file, Remake row on the Options screen's game page;
   off by default): when a scroll-line message speaks, its text (the line's own, with the names filled in) stays
   centred above the bottom panels, on a dark box, size 2 in the line's colour 0x21, broken at spaces into lines of
   at most 100 characters, until the voice ends. Messages without a voice file get none. The message screen's voices
   need nothing (their text is typed on the screen); videos, the start voice and unit voices get nothing (no text in
   the install).
2. **Option "Colour-blind colours"** (`ColourBlindColours`; off by default): empires 1..=8 take white (kept for 1)
   and the Okabe–Ito set (orange, vermilion, reddish purple, sky blue, bluish green, yellow) with a light violet-blue
   for 8, instead of `DDColor[16..=23]`; every pair stays more than 0.15 apart under all three dichromacies (test
   `colour_blind_pairs_stay_apart`). It changes the palette everything reads, so names, the minimap, the info screen,
   the top bar and the starmap's fleet sprites (column 1, white, tinted) follow; empires 9 and up keep the exe's
   colours. The 3D ship models keep their race textures.

## Consequences
- Both read only by the live window; captures, `--snapshot` and replays stay classic.
- The option rows' names are English (the game has no strings for them).
- If a later install or mod ships transcripts, the subtitle line can show them through the same `Hud::subtitle`.
