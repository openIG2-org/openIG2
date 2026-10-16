# Multiplayer: per-peer state in the simulation

Status: audit done 2026-10-16 (ADR 0007 §5). Implemented in `ig2-sim::game` (`ViewMode`, `LocalView`,
`Game::cosmetic`, `Game::messages_for`) and `ig2-net` (`Peer::show`, snapshot format version 2); tested by
`ig2-net/tests/lockstep.rs::local_views_stay_out_of_the_synced_state`.

The original runs the whole strategic sim on every machine, but some code paths read globals that differ per
machine: `EN` (0x912A3C, the local human's empire), the screen on show (`AktKep` 0xBAE0FC, `StarmapMode`
0xB9998C, `AktPlanet` 0xB9933C, the inbox message on screen `UAktUzenetFajta` 0xDABDBC), the selection
(`AktFleet` 0xB99664), and UI-paced timers (the 100 Hz colony tick, frame-paced scrolling and voice cooldowns).
Some libc `rand()` draws happen only for what is on screen. In single player they're game state like any other
(the byte-exact replays depend on them); in lockstep multiplayer, two peers whose players differ in any of them
would desync. (The original MP layer was host-relayed with full-state resyncs, STATUS U7, which hid this.)

## 1. The rule (ADR 0007 §5)

`Game::view` (synced, in the snapshot since `.ig2s` version 2):

- `ViewMode::Classic` (default): the sim reads the globals from the state, the UI writes them there
  (`Game::set_view`). Single player and every replay. Unchanged behaviour.
- `ViewMode::Canonical { en }` (lockstep MP; `Game::canonical_view(host_empire)` on every peer before the first tick):
  - `EN` = `en`, the host's empire, the same on every peer.
  - Nothing is on screen: no shown planet (`StarmapMode`/`AktPlanet` read as none/0), no selected fleet, no
    inbox message shown (`UAktUzenetFajta` = 0); the ground war's `on_screen` is false.
  - The sim never writes view state: `FlottaOsszevon` doesn't move `AktFleet` (the peer's UI follows
    `StarmapEvent::Merge`), and the scrolling line (`ScrollUzenetId`) isn't posted into the state; each peer
    formats its own with `Game::messages_for(events, its empire)`.
  - A `Multi_Send(.., EN, ..)` the original sends from the owner's own machine goes out for **every** human
    empire (each one's peer would have sent it): the 0x1b "fleet emptied" message of `HajoDelete`.
  - Draws that only pick what the local human sees come from `Game::cosmetic`, a per-peer libc stream outside the
    snapshot and the hash (the dialogue text pick when `EN` receives a message).
  - Each peer's own view is `Game::local` (`LocalView { empire, starmap_mode, planet, fleet }`), set by
    `Peer::new` (its empire, nothing shown) and `Peer::show`; the sim never reads it.

Classes in the table: **P** = presentation only (what the local human is told or shown); **G** = changes synced
gameplay state (including the libc stream's position); **SP** = the read is already gated by `!MultiPlayerMode`
in the original, so it never runs in MP. "MP" says what the canonical view does.

## 2. `EN` readers

| Port site | Exe function | Reads | Class | MP (canonical) |
|---|---|---|---|---|
| `Game::post_messages` / `messages_for` | `ScrollUzenetId` 0x556dc0 at the sites of `info-messages.md` §3.2 | `EN` filters the lines; writes the lists 0xE40CB0/0xE40F70 | P | not posted to the state; each peer: `messages_for(events, own empire)` |
| `colony::economy_head` | `FRISSIT_Planet` 0x474930 | unrest warning only for `EN`'s planets | P | canonical `EN`'s (events are reported for it only) |
| `colony::production_base` | `GetBolygoGazdagsag` 0x4aca20 | an unowned planet's richness uses `EN`'s race | G | host's race (the original would differ per machine) |
| `colony_side::transfer` | `BolygoElfoglal` 0x53b720 | `SideEvent::Met` for `EN` → `FelfedezBirodalom` (contact slot) | G (contact list) | canonical `EN`'s list; see §5 |
| `colony_side::transfer`, epidemic | `BolygoElfoglal` 0x53b720 | `new != EN` | SP | — |
| `colony_side` labs start / story lab bonus | `FRISSIT_Planet` 0x474930 | `EN`'s labs | SP | — |
| `fleet::Starmap::upkeep` | `FRISSIT_Starmap` 0x594ae0 | `ContactLost` (SP) / `ContactLostNetwork` (MP) for `EN`'s fleets | P | canonical `EN`'s |
| `radar::Radar::refresh` | `GetENRadarKifejlesztve` 0x53e020 | row 0 (unowned observer) = `EN`'s range; story radar bonus (SP) | G | row 0 = host's |
| `radar` planet circles | `FRISSIT_Starmap` 0x594ae0 (visibility pass) | the human sees inside radar planets' circles | SP | — |
| `research::tick` | `FRISSIT_Research` 0x51ddd0 | `EN`'s completion → `SetIdoLelassit` 0x541270 (MP: back to the program speed); `EN`'s cheat; story bonus (SP) | G | canonical `EN`'s completions reset the speed |
| `spy::Spies` reports, rank-ups, exposure, return, struck, discovered | `KemJelentesAdd` 0x58d000, `FRISSIT_Spy` 0x584b30 | `EN`'s spies / victims | P (queue `KemBeszolasok` 0xBAF338) | canonical `EN`'s |
| `spy` forced success | `FRISSIT_Spy` 0x584b30 | debug switch 0xB77250 for `EN` | SP | — |
| `ai::design::auto_design` | `TervAutoDesign` 0x541920 | tutorial event 0xb for `EN` | P | — |
| `ai::units::delete_ship`, `Game::combat_aftermath` | `HajoDelete` 0x53e5d0 (from `FRISSIT_MI`, `SimulateSpBattle` 0x5a5e60) | `Multi_Send(0x1b, EN, f)` when `EN`'s fleet is emptied | G (the fleet is deleted a frame later) | **every human's** emptied fleet (`SpyContext::senders`, `Globals::sends`) |
| `ai::resale` | design resale (`ai-empire.md` §5.3) | story resale ×0.5 for `EN`'s story designs | G | host's (campaign flag; SP in practice) |
| `ai::fleets::war_target`, `Diplomacy::target` | `MI_KitAkarTamadni` 0x50f040 | spare the human | SP | — |
| `Diplomacy::discover`, `war`, `receive_message`, `receive_answer` | `FelfedezBirodalom` (inlined 0x585531), `FeldolgozAjanlat` 0x4c1240, `FeldolgozValasz` 0x4c1480 | `EN`'s contact slots (`AktPlayerek` 0xA41E98): discovery, waiting/answered/countered letters | G (contact list) | canonical `EN`'s; see §5 |
| `Diplomacy::receive_message` text pick | `FeldolgozAjanlat` 0x4c1240 | one libc `rand()` when `EN` receives | P on the synced stream | `Game::cosmetic` (implemented) |
| `Diplomacy::consider` | `MI_DIPLOMACY` 0x4c7060 | the human's waiting letter | SP | — |
| `Game::starmap` (`EmpireMet`) | `FRISSIT_Starmap` radar pass | `EN` met an empire → story special 0x0B + `FelfedezBirodalom` | G | canonical `EN`'s |
| `Game::space_war` (`BattleEvent::human`) | `SpaceWarIndit` 0x5a7670 | messages 0x0F/0x10 for `EN` | P | routing uses `PlayerKi` (every human), not `EN` |
| `ground::War` end countdown | `FRISSIT_Grwar` 0x4747c0 | starts with `EN` involved | SP | — |
| `ground::War` voices, kind 7/9 death line | `WF::PlaySpeechGrWar` 0x48c670 | `EN`'s units | P (libc draws, on screen only) | off screen: none |
| `story` (scenario 360–375, storylines, `BRnd`, campaign) `w.human()` | `Story_Check` 0x5c5610 (SP scripts) | money, designs, fleets for `EN` | G | SP only (`Story_Check` runs them in SP) |
| `multi::MultiStories` | `st_Multi_include` 0x5ee4d0, `st_bShow` 0x603f20 | king of the hill info lines (P); the radar offer's message, `buyer == EN` starts the radar that hides enemy fleets, the purchase price falls on `EN` when the special has no sender (G); the flag carrier's race field (G) | G | canonical `EN`'s; see §5 |

## 3. Shown-screen and selection readers

| Port site | Exe function | Reads | Class | MP (canonical) |
|---|---|---|---|---|
| `Game::battlefield_weather` | `FRISSIT_Planet(true, 0)` prologue 0x4749b9..0x474ab0 | skips the planet on screen (`StarmapMode == 1`, `AktPlanet`): 0–2 libc draws fewer | G (libc stream) | nothing shown: every planet draws |
| `Game::inbox` | `FRISSIT_Messages` 0x5062d0 | the message on screen (`UAktUzenetFajta` 0xDABDBC) doesn't expire | G (`Story_MessageElevules`) | none shown (0) |
| `Game::starmap_pass` | `FlottaOsszevon` exe 0x53d860 | writes `AktFleet` = the merged fleet | UI state in the state | not written; UI follows `StarmapEvent::Merge` |
| `StoryWorld::akt_planet` / `akt_fleet` | campaign stories (`campaign.md` §8: bomb, volcano on `AktPlanet`; new ships into `AktFleet`) | the selection | G | SP only. `Game`'s `StoryView` still returns 0 (the trait default); reading the state broke the save replay (story 405 start at tick 2): open |
| `ground::War` with `on_screen` | `WF::Refresh` 0x490890, `FRISSIT_Grwar` 0x4747c0 | debris `AddRepesz` 0x45d400, ruins `Rom::Rom` 0x455570, bursts, dust, bomb textures, voices (libc draws; debris takes list-B slots that order later shots) | G | `Game::with_war` passes `on_screen: false`; a renderer makes its own debris from `Game::cosmetic` |
| the real-time ground war itself | `FRISSIT_Grwar` 0x4747c0: `WF::MI` 0x486d40 and `WF::Refresh` run for the planet on the colony screen only | `AktKep == 3`, `AktPlanet` | G | open (§5): must run by session rules, not by a peer's screen |

## 4. UI-paced timers and screen-dependent draws

| What | Exe | Class | MP |
|---|---|---|---|
| Scrolling line drain `FRISSIT_Scroll2` 0x556210 (frame-paced, `Ticker::pop`) | UI | P | per peer (the line isn't in the synced state in canonical mode) |
| Voice-line cooldowns (0xA3EA3C, UI-paced) | ground voices | P, but their draws are on the libc stream | off screen: no draws |
| Transporters `WF::Refresh2` 0x48ce10 every 5th `ColonyCounter` (f32 0x99823c) of the 100 Hz colony tick | ground war | G | open (§5): needs a fixed sim cadence |
| End-of-battle countdown `WF+0x148` (runs down on the colony screen only) | ground war | G | open (§5) |
| Real-time space battle `SpaceWarFrissit` 0x56dff0 (`IdoSpeedSzorzoBattle` calls per frame; the UI writes unit +0x184..+0x190) | battle screen | G | open: `MULTISEND_SpWarSynch` as commands (ADR 0007 open items) |
| `AktEtaDays` 0xF43428 libc draw per new month (`FRISSIT_Trade` 0x605ed0) | UI value | P, unconditional | stays on the synced stream (not view-dependent) |
| `FeldolgozValasz` 0x4c1480 text draw (every answer) | UI text | P, unconditional | stays on the synced stream |

## 5. Open (per-human state the canonical view can't express)

The canonical view makes every peer agree, but where the original kept something per human (it ran the same code
with a different `EN` on each machine), it now exists only for the host's empire:

- The human's diplomacy contact list (`AktPlayerek` 0xA41E98: discovered empires, waiting letters, answers,
  counter-proposals) and the spy report queue (`KemBeszolasok`). A non-host human's letters to AI empires still
  work (the AI answers through `FeldolgozValasz`), but the slots that pace the human's side are the host's.
  Needs: these as per-empire synced state (one list per human slot) and the MP blob's layout (`GetSavedGame`
  0x543670 stores the player in the `EN` slot) as evidence of how the original split them.
- `st_Multi_include`'s per-player effects (the radar purchase of mode 1003, the flag carrier record of 1005):
  should run once per human empire.
- The unowned planets' richness by the host's race (`GetBolygoGazdagsag`) and radar row 0: a fidelity question
  only; the original machines likely disagreed here.
- The real-time battles' screen-paced parts (§3, §4): the ground war, its transporters and countdown, and the
  space battle must run on the sim clock in MP, with player input as commands.
