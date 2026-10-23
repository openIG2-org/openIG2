# Tasks

What is left to reach a playable 1:1 client, then polish. Tick `[x]` and move the line to **Done** when a task is
pushed; the detail (specs, agents, acceptance) lives in [`docs/roadmap.md`](docs/roadmap.md) §6.
Updated 2026-10-23.

## M2: playable start to finish
- [ ] New game: skirmish start (mode 3)
- [ ] New game leftovers: the AI's design names, the campaign stories in `Game`'s story tick
- [ ] Client: build a `World` from a new `Game` (today only from a save)
- [ ] MainScr part 2: race and difficulty, scenario/skirmish setup, tutorial start, multiplayer entry, exit animation
- [ ] Colony war part 2: landing clicks, landing zone and shadows, unit picture, self-destruct window, cursors, camera keys
- [ ] Colony: the planet info mode
- [ ] SpaceWar part 5: lightning, debris, shield rings, fighters, planet-gun markers, cursors and tooltips, voice lines, big info box, formation picking
- [x] Design screen: hull look (Destroyer body and windows, tank hulls)
- [ ] Trade part 2: hull model, info card, pod list, return from Options
- [ ] Info/messages: the answers' orders
- [x] Diplomacy: opening animation, the special (story) menu, sounds
- [ ] Research: the button lines
- [ ] Video: the campaign intros (now unblocked), tutorial `anim`, message-clip capture check
- [ ] Game end: victory/defeat flow and videos, high scores

## M3: every screen exact
- [ ] Help (AktKep 1), credits, demo mode
- [ ] Tutorial overlay (the interpreter is done)
- [ ] U25's rest: battle posts, UI line, untraced ids, inbox expiry
- [ ] Sweep: re-run every screen's acceptance, close the open states in `re/screens/diff-results.md`

## M4: input and feel
- [ ] Input audit per screen against the keyboard handlers
- [ ] 16:9, 21:9 and 4K checks

## M5: multiplayer playable
- [ ] Lobby and connection screens on top of `ig2_net::session`

## M6: fixes and polish (after the prototype)
- [ ] Triage `re/deviations.md` (fix / enhanced option / keep)
- [ ] Sharper text, upscaled textures, graphics upgrades

## Done
Simulation (all systems), campaign scripts, tutorial interpreter, campaign, scenario and MP-galaxy new game; MainScr part 1, Options,
screen kit (widescreen rule, faders), starmap panels, colony ground-war UI, SpaceWar parts 1–4, Design parts 1–3,
Trade part 1, audio parts 1–3 and ground-war voices, video playback; MP engine, real-time battles over the
network, session layer, rejoin admission, keep-alive, host migration; U16, U19, U25 scripts' messages.
