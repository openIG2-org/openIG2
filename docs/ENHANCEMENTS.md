# Enhancements

The plan list of *enhanced* options: deliberate, opt-in improvements over the original. The classic ruleset and
client stay as the GOG release; every enhancement can be switched off (`CONTRIBUTING.md`). Port differences that
should match the original are not here: they live in `re/deviations.md`.

How an idea moves:
1. **Ideas**: anyone adds a row (what it changes, why a player would want it, the spec it touches).
2. **Proposed**: the row is specific enough to decide (the exact behaviour, the classic setting, the cost).
3. The maintainers accept or reject it. Accepted: an ADR in `docs/decisions/`, a task line in `TASKS.md`, the
   implementation. Rejected: the row is deleted (git history keeps it).
4. **Shipped**: the option exists; the row names its setting and ADR.

Ideas have no number; a row gets one when it is proposed. Numbers are shared with `re/deviations.md` (one
counter): a proposed row takes the next free number there (440).

## Ideas

Grouped by area; sizes are rough guesses for one developer.

| Area | Idea | Why | Rough size |
|---|---|---|---|
| Menus | A reworked menu system: clearer navigation, fewer clicks to start a game, a back button everywhere | the original menus are slow to move through | 1-2 weeks |
| Menus | Settings reachable from the main menu without starting or loading a game (display, sound, language, remake options) | today the Options screen opens only in a game | days |
| Menus | A language selector in the menu (today only `--lang`), applied without a restart where possible | players can't switch language in the game | days |
| Menus | A quick game: one screen with sensible defaults (galaxy, opponents, difficulty) and a Start button | a skirmish takes many screens to set up | days |
| Menus | More options: key rebinding, UI scale, separate music/voice/effect volumes, game speed steps, autosave interval | today's options are the original's few plus the remake rows | 1-2 weeks |
| UI | A modernised, themeable UI skin (colours, frames, fonts), the classic look kept as the default theme | the 2D panels look dated at high resolutions | weeks |
| UI | High-DPI / UI scaling for 4K screens | the 800x600 layout is small or blurry on big screens | 1 week |
| Text | Fill in the missing Hungarian and English texts (the remake rows, mode labels and lines the install lacks; `re/README.md` U13), from a translation file we write ourselves | some texts show in the wrong language or not at all | days |
| Graphics | Sharper text (vector or high-resolution font rendering at any window size) | the 800x600 bitmap font blurs when scaled up | weeks |
| Graphics | Upscaled textures and other graphics upgrades, each its own option | the original textures are low-resolution on modern screens | weeks |
| Graphics | Better effects: particle explosions, smoke, beams, shields in the space and ground battles (after the classic effects are complete, `re/deviations.md` 420/421) | the battles look flat | 2-4 weeks |
| Graphics | Higher-detail building models and textures on the colony screen, loaded from a pack beside the install | the buildings are low-poly | weeks, plus art |
| Graphics | A modern lighting option: per-pixel lighting, soft shadows, bloom, anti-aliasing | the original lighting is per-vertex | 2-3 weeks |
| Speed | Optimisation: profile the frame and the sim tick (`docs/perf.md`), faster loading, fewer draw calls | smooth play at high refresh rates and on weak machines | ongoing |
| AI | Smarter computer empires: better economy, research order, fleet use and diplomacy, as an "enhanced AI" ruleset beside the classic one | the original AI is easy to beat | weeks to months |
| AI | Per-bot difficulty and personality in a multiplayer room | today every computer player in the room plays alike | 1 week |
| Multiplayer | A nicer lobby: player cards, empire colours and pictures, ready states, chat | the room is functional, not pretty | 1-2 weeks |
| Multiplayer | Our own public rendezvous/relay instance (the service is done, ADR 0018, `docs/RENDEZVOUS.md`) | players behind strict NATs can join each other | days, plus a server |
| Multiplayer | A public lobby browser on the rendezvous service: list open games, join with one click | today a join code has to be shared | 1-2 weeks |
| Multiplayer | Steamworks (lobbies, invites, relay, achievements) | an easy way to find players | needs the rights holder's agreement and a legal check first |
| Modding | Mod support: a mods folder whose files override the install's, then rule tables and scripts that mods can change | new content and balance changes from players | weeks |
| Content | More races, as a mod or an enhanced ruleset (rules, diplomacy lines, pictures) | more variety | months, plus art |
| Platform | Play in the browser: open a URL, the main menu loads; the player picks their own install folder, the browser keeps the files (OPFS), the game data is never hosted. Work: a wasm32 build (wgpu WebGPU/WebGL2, winit, an async main loop), a virtual file layer for the install and the saves, WebAudio via cpal, videos through WebCodecs instead of openh264, the web clipboard instead of arboard; multiplayer over WebRTC or WebTransport through the rendezvous/relay service (ADR 0018). First step: `cargo check --target wasm32-unknown-unknown -p ig2-formats -p ig2-sim` | no install step, plays anywhere | main menu 1-2 weeks, single player 2-3 more, multiplayer 2-4 more |
| Platform | Linux and macOS builds (and the Steam Deck) | more players | 1-2 weeks |
| Comfort | Quick save and quick load keys, and autosaves kept in turns | lost progress after a crash or a bad battle | days |
| Comfort | Replays: the lockstep order stream saved beside the save, played back at any speed | review a battle or a whole multiplayer game | 1-2 weeks |
| Comfort | Fleet groups on Ctrl+1…9, and a colony build queue that repeats | less clicking in long games | days |
| Comfort | A searchable event log with filters (battles, research, diplomacy) | messages scroll away | days |
| Accessibility | Colour-blind-friendly empire colours, and subtitles for the videos and spoken messages | players who can't tell the colours apart or can't hear the voices | days |

## Proposed (maintainers decide)

| # | Area | What differs (ours vs exe) | Player-visible? | Spec |
|---|---|---|---|---|
| 5 | campaign story 834 | proposal: empire 1's siege fleets start at planet 8340, not at planet −1's (0, 0) (enhanced ruleset only; classic stays bit-exact) | minor | campaign/elem.md §8.14 |
| 146 | game end, high scores | proposal: a working high-score entry after a won scenario (dead code in the exe; score = Ido − scenario_kezdesi_ido [guess]) | minor | game-end.md §6 |
| 169 | starmap, wheel | proposal: wheel zoom (and right-drag pan) on the starmap; classic has neither | minor | starmap.md |
| 322 | title video | proposal: music only with the intro, not over the logos (ours matches the exe today) | minor | video.md §4 |

## Shipped

| # | Area | What differs (ours vs exe) | Player-visible? | Spec |
|---|---|---|---|---|
| 133 | colony camera | ours zooms with the mouse wheel; the original has no wheel zoom (classic: none) | minor | colony.md §11.9 |
| 159 | all screens, V key | a V hotkey for vsync (the original has none); also a "Vertical sync" row in Options (row 402) | minor | input.md |
| 197 | videos, skip | ours skips a video in any window mode; the exe only in fullscreen (`anyEvent` 0x7329d0) | minor | video.md §4 |
| 263 | overlay screens, background | the default redraws the starmap/colony live under the overlay (`LiveBackground`); classic = the exe's frozen `SaveScreen` picture (ADR 0015) | minor | overlay-background.md |
| 402 | options, game page | twelve "Remake" rows under the original's seven (display mode, resolution, vsync, 4:3 letterbox, live background, one-language names, clean name entry, models inside boxes, slider under label, Hungarian articles, centred Exit label, WASD camera), saved in `ig2-rework-options.dat`; their names and the mode/resolution labels are English only (the game has no strings for them); captures and `--snapshot` stay classic | minor | options.md §14 |
| 433 | options, save name entry | by default the edited slot's old name (or dashes) isn't drawn under the name entry, so a short name doesn't show the old tail; the exe draws the slot list first (`OptionsGfx2D_2` ELF 0x5d71c0); classic (`CleanNameEntry` 0) = the exe's drawing (ADR 0024) | minor | options.md `Gfx2D_2` step 6 |
| 434 | starmap and space battle, info box model | by default the bottom-left info box's turning model (planet globe, fleet or battle ship) is cut to the box `[5, 423, 167, 475]`; the exe clips it only to the screen, so a long hull at x −545 reaches past the box's left edge; classic (`BoxModelsInside` 0) = the exe's drawing (ADR 0025) | minor | starmap.md §11, spacewar.md §5.2 |
| 437 | Hungarian text, articles | by default a Hungarian game converts the article markers 0x95/0xB0 to "a"/"az" (`a2az`) on the message line and its history, the message screen, the diplomacy menus and dialogue lines, the space battle's "A vs. B" banner and the ground war's attack banner (which names the owner first, as the exe's `Lang` 4 branch does); the GOG exe keeps `Lang` 1 and shows the raw marker byte; classic (`HungarianArticles` 0) = the exe (ADR 0028) | minor | locale.md §2 |
| 438 | starmap, Exit label | by default the production view's, transfer panel's and fleet panel's Exit label is centred on its button; the exe draws it 11 virtual px left of centre in the production view (margins 69/97 px at 800x600, 83/84 with the option); classic (`CentredExitLabel` 0) = the exe (ADR 0029) | minor | starmap.md §12.2 |
| 439 | starmap and colony, W/A/S/D | held W/A/S/D (by place) pan the camera like the pointer at that edge, same speed; the exe pans only at the edges and its letters do nothing on the starmap; `WasdCamera` 1 (default) = starmap only, 2 = also the colony screen, where they replace the exe's `a` (select all) and `s` (stop), 0 = the exe; never while a text entry has focus (ADR 0030) | minor | starmap.md §10.6, colony.md §11.2 |
| 410 | Options, tutorial | while the tutorial runs the "Ground battle simulate" and "Space battle simulate" rows read off and are greyed (stored values kept); the exe leaves them live while `DISABLE OVERRUN` sets the flags behind them (ADR 0019); default on | minor | options.md §14, tutorial.md §5 |
| 422 | MP, hosting | the room asks the router for a UPnP / NAT-PMP port mapping when it opens; one press in the room switches it off (ADR 0020); the exe has no port mapping | minor | start.md, ADR 0008 §2 |
| 423 | MP, desyncs | a desync's resync puts a line on the host's and the resynced joiner's message line and a log line (ADR 0021); the exe's `Desync` flag (0xB771E8) is never shown | minor | multiplayer.md §7.2 |
| 428 | MP, resuming a save | a saved game resumes with only some of its players in the room; the computer plays each saved human nobody holds (and keeps the empire); the exe has no multiplayer save | minor | multiplayer.md §7.2, ADR 0022 |
| 425 | MP, space battle | with the room's "Space battle formations" option on, every human side with ≥ 6 ships picks a formation and starts before the battle runs (at most 1200 ticks; ADR 0023); the exe skips the formation phase in MP (off = the original) | minor | multiplayer.md §7.1.2, space-combat.md §7.10 |
| 435 | MP, defeated player | with the room's "Defeated players: stay and watch" option on, a machine whose empire dies plays the defeat videos, then returns to the starmap and stays in the session as a spectator, and anyone can join the running game under a new name to watch it, at most 8 (the exe has no joining after the start) (sees every planet, fleet and name; its orders are dropped, and every peer ignores orders of a dead empire; ADR 0025); the exe leaves the game (`ExitToMainScreen` 0x5117f0; off = the original) | minor | multiplayer.md §7.1.3, game-end.md §3 |
| 436 | space battle, defend/attack slider | by default the slider bar sits 8 px under its label (label y+38, bar y+46) and its hot-spot 0x19 grows to 14 rows; the exe draws the label "VEDEKEZES … TAMADAS" (19 chars, `GetString(0xb,1,0x14)`) over the bar; classic (`SliderUnderLabel` 0) = the exe's drawing (ADR 0027) | minor | spacewar.md §5.2 |
