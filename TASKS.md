# Tasks

What is left to reach a playable 1:1 client, then polish. Tick `[x]` and move the line to **Done** when a task is
pushed; the detail (specs, agents, acceptance) lives in [`docs/roadmap.md`](docs/roadmap.md) §6.
Updated 2026-10-23.

## M2: playable start to finish
- [x] New game: skirmish start (mode 3; `Game::start_skirmish`, `new-game.md` §13.1)
- [ ] New game leftovers: the AI's design names, the campaign stories in `Game`'s story tick
- [x] Client: build a `World` from a new `Game` (`World::new_game`, `--new-campaign`; `mainscr.md` §9.1.1)
- [x] MainScr part 2: race and difficulty, scenario/skirmish setup, tutorial start, multiplayer name entry, exit animation (skirmish start through `Game::start_skirmish`; MP lobby M5)
- [x] Colony war part 2: landing clicks, landing zone and shadows, unit picture, self-destruct window, cursors, camera keys
- [x] Colony: the planet info mode
- [x] SpaceWar: formation re-placement (sim, deviation 137; `Battle::pick_formation`, 29 exe picks bit-exact)
- [ ] SpaceWar: the rest of spec §9.7 (planet stations' models dev 144, the info view, zoom names dev 138, uncaptured effects)
- [x] Design screen: hull look (Destroyer body and windows, tank hulls)
- [x] Trade part 2: hull model, info card, pod list, return from Options
- [x] Info/messages: the answers' orders
- [x] Diplomacy: opening animation, the special (story) menu, sounds
- [x] Research: the button lines
- [x] Video: campaign intros verified (hook after `new_game`, no code change), `anim` is the debug cheat console's (not the tutorial's), message clips captured against the exe
- [x] Game end: victory/defeat flow and videos, high scores (`rules/game-end.md`; the scenario panel's table draw: MainScr part 2)

## M3: every screen exact
- [ ] Help (AktKep 1), credits, demo mode
- [ ] Tutorial overlay (the interpreter is done)
- [x] U25's rest: battle posts, UI line, untraced ids, inbox expiry (left: the AI's messages to the human in the save replay, diplomacy §11 reason codes)
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
Simulation (all systems), campaign scripts, tutorial interpreter, campaign, scenario, MP-galaxy and skirmish new game; MainScr part 1, Options,
screen kit (widescreen rule, faders), starmap panels, colony ground-war UI, SpaceWar parts 1–5 (spec §9.8), Design parts 1–3,
Trade part 1, audio parts 1–3 and ground-war voices, video playback; MP engine, real-time battles over the
network, session layer, rejoin admission, keep-alive, host migration; U16, U19, U25 scripts' messages.
