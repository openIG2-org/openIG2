# Tasks

The single work queue. **Features before polish**: a task is done when a player can do it in the live window
(not when a test or a headless capture passes). Deviation rows are fixed only when a player would notice them
(`re/deviations.md` → "Open: player-visible"). Tick `[x]`, add one line to `CHANGELOG.md`, and delete the line here
at the next cleanup.

## M7: playable 1.0

### Finish the game
- [x] **Starmap story actions**: the radial icons 0x10/0x11/0x12: the gigabomb (story 402), the story
  fleet's wormhole, the virus alert. 0x11 is offered but does nothing; 0x10/0x12 are never offered
  (`ig2-client/src/starmap_menus.rs` `rop_places`/`fire`, `re/screens/starmap.md`); the campaign episodes that need
  them must be completable.
- [x] **The story-planet ground war**: `Grwar8020hoz` (`ig2-sim/src/game/battles.rs`, `GroundScripted`),
  and `War::locate_tank`'s fallback without a colony map (fixed cell 100,100), `re/rules/ground-combat.md`.
- [x] **Play-through harness**: `ig2-client --playthrough` drives whole games headless with the AI
  playing the human's side, saving and loading on the way (`docs/BUILDING.md` → Play-through harness).
- [x] **Story-aware play-through driver**: in a campaign the harness plays the story's part of the player (episode
  answers, techs, spy missions, story specials, fleets to story targets, colony ships for story 840 through the
  production order, attacks on the empires the story guards from the AI, war on a much smaller empire when at war
  with nobody); `--seed` takes a list. On difficulty 1 every campaign reaches its end on some seeds: campaign 1 won
  on both storylines (storyline 1 seed 2, storyline 2 seed 4242), campaign 2 storyline 1 ends in story 863's own end
  (CampaignOver, seed 4242) and is won on seed 2, storyline 2 won on seed 4242, campaign 3 storyline 1 won (seed 8).
- [ ] **Campaign 3 storyline 2 to its end**: no seed tried (2..11, 4242) wins storyline 720 on difficulty 1; on some
  the human still misses story 840's five colonies and can't retake the two planets empire 17 takes within 900 days;
  on the others empire 1 wins the war.
- [x] **Story spy missions in the sim**: the spy pass asks the story (`Story::spy_question`), so story 863's end (CampaignOver) resolves
  (`re/rules/campaign/elem.md` 861/863). Campaign 2 storyline 2 seed 2 reaches the end videos; seed 1 runs out of ticks
  because the AI-played human never conquers empire 5's last planets.
- [x] **Diplomacy specials reach the stories**: a special the AI receives reaches stories 857/858/860
  (`Game::diplomacy_message`); the play-through sends them. Still blocked: 857's purchase needs 2,000,000 credits
  (the AI-played human never has them) and 858 waits for empire 5's death.
- [x] **Fleet panic in story 856**: the raider ordered at fleet `OsszHanyFlotta + 1` read a slot past the last fleet;
  it now reads as a zeroed record, as in the exe.
- [x] **MP loopback play-through**: the same harness for a 2-peer loopback MP game (desync check): `ig2-client --playthrough mp`.
- [x] **Tutorial input locks**: the F-key and screen blocks, greyed entries, gate-flag locks and the
  recorded input replacing the live mouse (deviations 141, 174, 198).
- [x] **Single-player real-time ground war**: `Game::ground_war`/`grwar_sp` (`ig2-sim/src/game/battles.rs`) run an
  invasion routed to real time on its colony screen; `re/rules/ground-combat.md` §2, §11.
- [x] **Diplomacy and spy leftovers**: diplomacy's MP-only menus (0x2e) and lip sync; the spy screen's
  2D video path and the tutorial's spy limits.

### Ship it
- [x] **Install detection**: find the GOG install (registry, default paths, GOG Galaxy) when
  `IG2_INSTALL_DIR` is unset; a clear message when none is found; check the `ig2.exe` hash.
- [x] **Display settings**: window/fullscreen/borderless, resolution, vsync and the letterbox option in
  the Options screen (enhanced-styled), saved in our settings file.
- [x] **Release bundle**: a `--release` build script per platform (Windows first), version from a
  `VERSION` file, a zip with the exe and a readme; no game data.
- [x] **Install check tool**: `ig2-check`, like openOMSI's `omsi-check`: loads every asset, table and
  script of the user's install and reports what fails to parse.

### Multiplayer over the internet
- [x] **Port mapping in the client**: wire `ig2-net::portmap` (UPnP/NAT-PMP) into hosting, opt-in.
- [x] **Join codes and private rooms**: in the MP room UI.
- [x] **Desync soak**: long loopback MP games with every order type; trace the untraced MP paths when a
  desync shows (U15 economy, U18 production, U22 AI, U23 ground war; `re/README.md` → Open unknowns).
- [ ] **Rendezvous / relay service**. The service and the client side are done (ADR 0018, `docs/RENDEZVOUS.md`); left: whether and where to run a public instance (maintainers' decision).

### Player-visible deviations
- [ ] The rows under "Open: player-visible" in `re/deviations.md`.

## Maintainer decisions
- [ ] The enhanced proposals in `re/deviations.md` → "Enhanced proposals": accept or reject each; an ADR per
  accepted option, then its implementation.

## After 1.0
- Sharper text, upscaled textures, graphics upgrades (each an enhanced option with an ADR).
- The minor and keep rows of `re/deviations.md`.
