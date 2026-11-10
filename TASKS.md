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
  (CampaignOver, seed 4242) and is won on seed 2, storyline 2 won on seed 4242, campaign 3 storyline 1 won (seed 8), storyline 2 ends (story 837) and is won on seeds 2 and 5.
- [x] **Campaign 3 storyline 2 to its end**: campaign 3 keeps empire 1 shielded from the AI while it isn't the
  strongest (`StoryAdat+0xb8`), so the driver sends every human fleet after the story goals while the human's only
  wars are with such empires, declares war on a small empire near the end (story 405), and keeps as many colony ships
  in production as story 840 still needs. Seeds 2 and 5 end storyline 720 and win; other seeds lose to empire 1,
  stall, or miss 840's colonies.
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
- [x] **Port mapping in the client**: wire `ig2-net::portmap` (UPnP/NAT-PMP) into hosting (on by default since ADR 0020).
- [x] **Join codes and private rooms**: in the MP room UI.
- [x] **Desync soak**: long loopback MP games with every order type; trace the untraced MP paths when a
  desync shows (U15 economy, U18 production, U22 AI, U23 ground war; `re/README.md` → Open unknowns).
- [x] **Rendezvous / relay service**. The service and the client side (ADR 0018, `docs/RENDEZVOUS.md`), its public game list (ADR 0060), and the public instance at `ig2relay.justdawee.hu:30000`, the game's default relay server.
- [x] **Multiplayer: join a running game as a spectator** (issue #37; ADR 0026).

### Player-visible deviations
- [ ] The rows under "Open: player-visible" in `re/deviations.md`.

## Maintainer decisions
- [x] The "Proposed" rows of `docs/ENHANCEMENTS.md`: accept or reject each (2026-11-01: all four accepted); an ADR and a task line here per
  accepted option, then its implementation.
- [x] **Auto research** (ADR 0037, `docs/ENHANCEMENTS.md` row 451):
  - [x] Part 1, sim: per-empire switches (ships, buildings, tanks), the lockstep order, the pick inside the research tick, saved in our saves.
  - [x] Part 2, UI: the three switches on the research screen (no Options rows: the switches are per empire and live in the game).
- [x] **Custom game** (ADR 0085, `docs/ENHANCEMENTS.md` row 546):
  - [x] Part 1, sim: `CustomGame` (galaxy, per-empire race, money, technology, start planets, level, start position), saved and in the start data; the original's skirmish starts unchanged.
  - [x] Part 2, UI: the modern Custom game page (galaxy settings, the empires table), without the map preview.
  - [x] Part 3: the starmap preview and the chosen start star.
  - [x] Part 4: aggressiveness and start relations, measured in the play-through harness (8 seeds, 10 years).
  - [x] Part 5: the multiplayer room's settings and the classic skirmish panel's "Advanced" label (enhanced ruleset).

## Remaster (as mods)
The plan and its rules: ADR 0088. Each part is engine work that does nothing until a mod switches it on.
- [x] **Look files** (562): a mod's `visuals.toml`, the planets' halo, the first mod `mods-src/remaster-look`.
- [ ] **HDR, tone mapping, bloom over the whole scene** (563).
- [ ] **Lights and shadows on every 3D screen** (564).
- [ ] **A richer starmap** (565).
- [ ] **Effects as data, drawn on the graphics card** (566).
- [ ] **Texture pack tool** (567).
- [ ] **Sprite animations as smooth effects** (568).
- [ ] **Model packs** (569).
- [ ] **Publish the remaster mods in the mod index** (needs the index repository, 549).

## After 1.0
- The minor and keep rows of `re/deviations.md`.
- The ideas in `docs/ENHANCEMENTS.md` (sharper text, upscaled textures, play in the browser, …).
