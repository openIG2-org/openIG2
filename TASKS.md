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
- [ ] **Story-aware play-through driver**: the AI-played human loses all three campaigns, so later
  episodes, CampaignOver and the campaign end videos are never reached headless; drive the campaigns to their end.
- [ ] **MP loopback play-through**: the same harness for a 2-peer loopback MP game (desync check).
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
