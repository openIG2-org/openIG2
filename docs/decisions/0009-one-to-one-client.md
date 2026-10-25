# ADR 0009: A 1:1 client

Date: 2026-10-17. Status: **accepted** (the project's goal of 2026-10-17). Builds on ADR 0006 (renderer and client
architecture) and refines its "classic fidelity" line for the client; ADR 0005/0008 (multiplayer) supply the one
area where the client has no original to copy.

## Context
The simulation (`ig2-sim`) matches the original byte for byte. The client (`ig2-client`) draws the starmap, the colony
screen, the HUD, the info charts and the inbox, checked by eye and by hand-measured offsets against ig2.exe captures
(`re/screens/hud.md`). Most screens, the input, the audio and the video are still missing, and nothing so far says
how close "close" has to be, how a screen is accepted, or what happens on displays the original never supported.

The project's goal (2026-10-17): `ig2-client` is a **1:1 remake of the original GOG client** in menu structure, screens,
layout, input, sounds, timing and feel. A player who starts it must not be able to tell it from the original, except
by the extras they opted into and by the new multiplayer.

## Decision

### 1. Fidelity is the default
- Every screen, menu, dialogue, transition, cursor, tooltip, sound cue, music track, voice line and cutscene of the
  original exists in the client, reached by the same clicks and keys, laid out at the same virtual coordinates
  (640 × 480 virtual screen, `re/screens/hud.md` §1), animated at the same pace (20 Hz sim, 100 Hz UI tick, the
  original's frame-paced counters) and drawn from the user's own assets.
- Behaviour comes from the exe (draw and input functions cited in `re/screens/*.md`), not from memory of the game.
  Original quirks visible to the player (odd tab orders, a button that needs a second click, text that overflows its
  box) are kept unless they crash or corrupt state; such fixes are listed in the screen's spec.
- The screen inventory `re/screens/README.md` lists every screen of the original with its exe draw and input functions,
  the flow between screens and the client's status. A screen is "done" only as described in §4.

### 2. Deliberate differences are opt-in
- **Enhanced mode** (one switch in our options screen, per feature where it makes sense): quality-of-life or visual
  changes (higher-resolution 3D, filtered textures, extra hotkeys, fixed original bugs that change what the player
  sees, …). Off by default. Each one is named in the screen's spec and, if it changes rules, gets its own ADR
  (as ADR 0003 does for libc `rand()`).
- **Multiplayer screens** (lobby, LAN browser, direct IP, password, rejoin, desync notice; ADR 0005/0008) have no
  original to copy. They are new, but **styled like the original's menus**: the same backgrounds, brushes, bitmap
  font, button frames, sounds and transitions, so they read as part of the same game. They are reached from the
  original's multiplayer entry in the main menu.
- Our own settings (display, enhanced features) live in one added options page styled the same way; the original's
  options keep their layout.
- Developer tools (egui overlays, `--snapshot`, `--info`) stay out of the player's way (ADR 0006 §5).

### 3. Widescreen, high DPI and 4K
*Amended by ADR 0010 (2026-10-18): the default is the remaster's own rule (2D in the centred 4:3 frame, the 3D
scenes and background pictures over the full width, narrow windows letterboxed top and bottom); letterboxing is the
option. The first two bullets below are superseded.*
- **Default: the original layout scaled up and letterboxed.** The 640 × 480 virtual screen is scaled by the largest
  factor that fits the window (non-integer allowed, aspect 4:3 kept) and centred; the bars are black. 2D brushes are
  sampled from the remaster's 2× pictures, the bitmap font and the 3D scenes are drawn at the output resolution (no
  upscaled low-resolution render), so the picture is sharp but the layout is the original's.
- **Option "fill the screen with the 3D maps"** (enhanced, off by default): on the starmap, the colony screen and the
  battle screens, the 3D view extends into the side bars (wider field of view at the same vertical fov), while every
  2D panel stays at its original virtual position inside the centred 4:3 area. 2D-only screens (menus, research,
  diplomacy, …) stay letterboxed.
- Refresh rate is free (interpolation between sim ticks, ADR 0006); everything the original paces by frame counters
  is paced by the 100 Hz UI clock instead, so timing matches the original's at its reference rate.

### 4. Acceptance: a capture comparison against ig2.exe
A screen is accepted only when it passes a **capture comparison**: `tools/trace/run_screendiff.py` puts ig2.exe
(through Frida, `tools/trace/screen.js`) and `ig2-client --snapshot` on the same save, screen and state, captures both
at the same size and reports the pixel difference (whole frame and per region, with a heat map). The comparison's
result is written into the screen's spec, and the screen's row in `re/screens/README.md` names the run.

- Pass: every 2D region (panels, text, buttons, top bar) matches within 1 virtual pixel of position and the
  per-region mean difference is under the threshold the tool reports as "match" (alignment search ≤ 2 px). 3D regions
  pass when the camera, the objects and their placement match (they may differ by sampling, lighting and particle
  noise; the spec names what is accepted and why).
- Animated or random parts (the message line, particles, the background noise) are compared at a pinned moment
  (same tick, same UI frame, same libc seed) or masked, and the mask is named in the result.
- Input and sound are accepted the same way where they can be captured: the same key or click on the same state
  gives the same next screen and the same command (Frida on the exe's input functions), the same sound id at the
  same moment.

### 5. What this changes
- The README and `CONTRIBUTING.md` state the rule. New screen work starts by reading the screen's row in the inventory,
  porting its draw and input functions, and ends with a capture comparison.
- Missing whole areas are tracked as tasks (`TASKS.md`): audio (music, voices, UI sounds), MP4 cutscene playback, the
  menus before the game, the tutorial interpreter.

## Consequences
- More work per screen (capture, diff, spec update), but "done" becomes measurable and regressions show up as diffs.
- The capture tool depends on Frida driving ig2.exe and on states the exe can be put into (a save + a screen
  request); screens that need a staged state (a battle, an open dialogue) need their own staging in `screen.js`.
- Widescreen is honest: the default looks like the original on a modern monitor; the fuller view is a choice.
