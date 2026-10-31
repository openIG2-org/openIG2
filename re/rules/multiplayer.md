# Multiplayer: per-peer state in the simulation

Port status: the rule of ADR 0007 §5; the real-time battles run by session rules (§7, ADR 0011). Implemented in `ig2-sim::game` (`ViewMode`, `LocalView`,
`Game::cosmetic`, `Game::messages_for`) and `ig2-net` (`Peer::show`, snapshot format version 2); tested by
`ig2-net/tests/lockstep.rs::local_views_stay_out_of_the_synced_state`. Per-human contact lists, spy report queues
and the `st_Multi_include` effects (§5): `SaveState::humans`, snapshot version 3, tested by
`ig2-net/tests/lockstep.rs::each_human_keeps_its_own_contacts_and_reports`. Each human's open fleet-transfer panel
is synced state (`SaveState::transfer_panels`, snapshot version 6; changes travel as the `TransferPanel` command), so a
split's new fleet survives on every peer: `lockstep.rs::a_split_fleet_lives_while_its_owners_transfer_panel_is_open`.

The original runs the whole strategic sim on every machine, but some code paths read globals that differ per
machine: `EN` (0x912A3C, the local human's empire), the screen on show (`AktKep` 0xBAE0FC, `StarmapMode`
0xB9998C, `AktPlanet` 0xB9933C, the inbox message on screen `UAktUzenetFajta` 0xDABDBC), the selection
(`AktFleet` 0xB99664), and UI-paced timers (the 100 Hz colony tick, frame-paced scrolling and voice cooldowns).
Some libc `rand()` draws happen only for what is on screen. In single player they're game state like any other
(the byte-exact replays depend on them); in lockstep multiplayer, two peers whose players differ in any of them
would desync. (The original MP layer was host-relayed with full-state resyncs, U7 in `re/README.md`, which hid this.)

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
| `fleet::Starmap::upkeep` | `FRISSIT_Starmap` 0x594ae0 | `ContactLost` (SP) / `ContactLostNetwork` (MP) for `EN`'s fleets | P | every human's (`ContactLostNetwork`) |
| `fleet::Starmap` radar pass, local-human sightings | `FRISSIT_Starmap` 0x595422..0x595570 | tracked story fleet, revealed wormhole ends, "enemy approaching" for `EN` | G | every human gets its own (`fleet-movement.md` §6.4) |
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
| `StoryWorld::akt_planet` / `akt_fleet` | campaign stories (`campaign.md` §8: bomb, volcano on `AktPlanet`; new ships into `AktFleet`) | the selection | G | SP only. `Game`'s `StoryView` returns 0 (the trait default; reading the state diverges the save replay at story 405's start): open |
| `ground::War` with `on_screen` | `WF::Refresh` 0x490890, `FRISSIT_Grwar` 0x4747c0 | debris `AddRepesz` 0x45d400, ruins `Rom::Rom` 0x455570, bursts, dust, bomb textures, voices (libc draws; debris takes list-B slots that order later shots) | G | `Game::with_war` passes `on_screen: false`; a renderer makes its own debris from `Game::cosmetic` |
| the real-time ground war itself | `FRISSIT_Grwar` 0x4747c0: `WF::MI` 0x486d40 and `WF::Refresh` run for the planet on the colony screen only (SP) | `AktKep == 3`, `AktPlanet` | G | the war's planet every tick, the original's MP branch (§6) |

## 4. UI-paced timers and screen-dependent draws

| What | Exe | Class | MP |
|---|---|---|---|
| Scrolling line drain `FRISSIT_Scroll2` 0x556210 (frame-paced, `Ticker::pop`) | UI | P | per peer (the line isn't in the synced state in canonical mode) |
| Voice-line cooldowns (0xA3EA3C, UI-paced) | ground voices | P, but their draws are on the libc stream | off screen: no draws |
| Transporters `WF::Refresh2` 0x48ce10 every 5th `ColonyCounter` (f32 0x99823c) of the 100 Hz colony tick | ground war | G | once per sim tick (§6) |
| End-of-battle countdown `WF+0x148` (runs down on the colony screen only) | ground war | G | none in MP: `WF::Refresh` ends the war when a side holds the field (§6) |
| Real-time space battle `SpaceWarFrissit` 0x56dff0 (`IdoSpeedSzorzoBattle` calls per 20 Hz tick; the UI writes unit +0x184..+0x18f, selection/UI only) | battle screen | G | every slot every tick, the humans' orders as commands (§6) |
| `AktEtaDays` 0xF43428 libc draw per new month (`FRISSIT_Trade` 0x605ed0) | UI value | P, unconditional | stays on the synced stream (not view-dependent) |
| `FeldolgozValasz` 0x4c1480 text draw (every answer) | UI text | P, unconditional | stays on the synced stream |
| The spy screen's verdict roll (`Spy_Main` exe 0x58d410: `rndlog(0, …)` ×1–2) | UI | G (it picks the fate sent) | `spy::Order::Judge` carries the choice, every peer rolls (`re/screens/spy.md` §7) |
| The spy screen's chance estimate (`KemEselySzamolas1` exe 0x587b80, stream 0: 2 draws a round × 1000 + picks) | UI | G (stream 0) | `spy::Order::Estimate`: every peer draws; the issuer shows the figure from a copy |
| Closing a spy report (`KemHanyBeszolas −= 1`, `Spy_Main`) | UI | G (the queue, §5.2) | `spy::Order::Acknowledge` on the issuer's queue |

## 5. Per-human state (ours; implemented)

Where the original keeps something for "the local human" and runs the same code with a different `EN` on each
machine, every machine ends up with its own copy for its own empire. The canonical view alone would keep it for the
host's empire only, so canonical MP keeps **one copy per human empire** in the synced state (`SaveState::humans`,
snapshot version 3); `ViewMode::Classic` has none, so single player and every replay are unchanged.

Evidence that the original never split them: the MP state blob `GetSavedGame(player)` (exe 0x543670,
`formats/save.md` §6) is the save stream with only `player` written into the `EN` slot, so a joining client gets
the **host's** `AktPlayerek` and `KemBeszolasok` and from then on its own machine updates them for its own `EN`. The
initial copy is therefore ours (below), the updates follow the original's `EN` code paths with `EN` = the copy's
owner.

### 5.1 Contact lists (`AktPlayerek` 0xA41E98, `diplomacy.md` §10.1)
- `Diplomacy::players`/`ghost`/`shown_players` stay the canonical `EN`'s list (the exe's memory); every other human
  `h` has a `ContactList { empire: h, players, ghost, shown }` in `Diplomacy::others` (`SaveState::humans[h]`, raw
  0xc4-byte slots, so the dialogue bytes the sim doesn't decode survive).
- Created by `Game::canonical_view` for every human-controlled empire (`PlayerKi == 1`) other than `en` that has
  none yet: `FelfedezBirodalom(e)` (`discover`) for each `e` it has met (`BirodalomIsmeri[h][e]`), `e = 1..=8` in
  order (empires ≥ 9 and race 5 get no slot, as in the original).
- Each `EN`-keyed site of `diplomacy.md` applies to **the list of the human concerned** (`Diplomacy::keeps(x)` = `x`
  is `EN` or has a list):
  - `FelfedezBirodalom` (`discover_by(owner, e)`): from a message to `owner` (`FeldolgozAjanlat`), a war of
    `owner`'s (`DiplomaciaMegtamadas`), a radar meeting (`StarmapEvent::EmpireMet { by: owner }`: story special
    0x0B stays `EN`'s only), a caught spy of an unknown empire (`SpyEvent::Discovered { by: owner }`: special 0x0B
    `EN`'s only), a planet changing hands in a deal (`BolygoElfoglal`'s `SideEvent::Met`, reported for every
    human; diplomacy keeps those whose `who` has a list).
  - `FeldolgozAjanlat` (exe 0x4c1480 area, §5): a message to `b` with a list waits in `b`'s slot of the sender
    (`a != b`; the dialogue-text draw comes from `Game::cosmetic` in canonical mode, as for `EN`).
  - `FeldolgozValasz` (§7): an answer or counter-answer to proposer `a` with a list lands in `a`'s slot of `b`.
  - `FRISSIT_Diplomacy`'s auto-cancel (§10): every list, `EN`'s first, then the others by empire;
    `DiploEvent::AutoCancelled { from, to }` names the list's owner.
  - `DiploEvent::Discovered { empire, by }` names the owner (each peer posts message 0x1A for its own empire).

### 5.2 Spy report queues (`KemBeszolasok` 0xBAF338, `info-messages.md` §3.3)
- `KemJelentesAdd` is guarded by "the receiver is `EN`". Canonical MP: a report to `EN` is `SpyEvent::Report`
  (queue in the exe's memory, `KemEsemeny`, `Story_KemEsemeny`); a report to another human `h` with a list is
  `SpyEvent::ReportTo { to: h, report }`, which only appends to `h`'s queue (`SaveState::humans[h].reports`, the
  same 50-entry shift as step 2). `Story_KemEsemeny` is single-player only in the original (`campaign.md`), so the
  queue is all there is in MP.
- The other `EN`-guarded spy events (rank-ups, the victim's `Struck`, `Exposed`) are presentation only and stay
  `EN`'s; per-peer messages for them are the client's (`Game::messages_for`).

### 5.3 `st_Multi_include` (`campaign.md` §8.10)
- 1003 (three planets): the radar purchase comes back as special 15 with the buyer; the original starts the radar
  counter only on the buyer's machine (`buyer == EN`). Canonical MP: every buyer is recorded in story variable
  `RADAR_BUYERS` (10039, a bit per empire; an id the scripts never use) and the radar pass runs for each recorded
  buyer in empire order (its planets, the enemies of that buyer), plus `EN` when it bought; the fleet visibility it
  sets is the canonical view's; every human gets its own machine's sightings (resolved, `fleet-movement.md` §6.4).
- 1005 (capture the flag): the original writes `EN` into the Flag Carrier design's race field (`flag_carrier(EN)`),
  so the two machines' designs differ. Canonical MP: the design for empire 1 carries 1, for empire 2 carries 2.

- 1003's radar offer per human (ours). Each machine of the original runs its own offer
  (counter 10034, 50 days; `NewMessage(0x2347, 300)` with `PlayerNev[EN]`; answer 2 buys with `Multi_Send(0x66, EN, …)`,
  4 asks again in 50 days, the entry's expiry 100) from its own inbox. Canonical MP:
  - **A per-human inbox.** The inbox (only story and tutorial messages reach it) and the messages screen's dialogue
    globals (`INBOX`, `INBOX_RANDOM`, `INBOX_PLAYBACK`, `INBOX_COUNT`, 0xDABDA0..0xDABE40, 0xDA65E7, 0xDA65FF,
    0xDA6607, 0xD90DC0..0xD90DD8, 0x91494C, `Valasztasok` 0xD8E160 + 4·0x5c, 0xD909BA..BB) are the canonical `EN`'s in the
    state; every other human keeps its own copy in `HumanSide::inbox` (created with an empty inbox and the start's
    dialogue state by `per_human_copies`; in the snapshot and the hash). `Game::with_inbox_of(h, …)` swaps it in and out
    (symmetric, so the state is the canonical one outside it).
  - **Orders.** A messages-screen order from human `h ≠ EN` runs inside `h`'s inbox (classic: other issuers' orders do
    nothing); a story hand-off from it (an answer) runs with `StoryWorld::answering()` = `h`. The expiry pass
    (`FRISSIT_Messages`) runs for each human's inbox in turn, its promotion answered by that human.
  - **1003.** At the start each other human `h` gets counter `10060 + h` (50 days; 10061..10068 unused by every story);
    when it fires `StoryEvent::HumanMessage { empire: h, id: 0x2347, arg: 300 }` puts the offer into `h`'s inbox with
    `PlayerNev[h]`. Answer 2 raises `SendStorySpecial { id: 10031, sender: answering }` (the special's sender is the
    answering human: `Command::StoryFrom`, sim-originated, never taken from a peer; the 0x66 lists above it name its
    sender); answer 4 and the promotion re-arm
    the answering human's counter (10034 for `EN`). The purchase and the radar pass were per buyer already
    (`RADAR_BUYERS`).
  - **Client.** `world::local_en` also swaps the local human's inbox in while the world holds the game, so a joiner's
    messages screen shows its own inbox; its orders travel as commands.
  - The slow-down of `NewMessage` stays synced (the original slowed only the receiving machine).
### 5.4 EN in the client (ours)

The shared state's EN is the host's empire. `ig2-client::world::local_en` writes the local empire into EN while the
world holds the game and puts the canonical one back before the game goes into the peer, so hashes and snapshots are unchanged.

### 5.5 The sim's own messages (deviation 231)

`Multi_Send` (exe 0x513840) only appends to the outgoing composer; `MultiSendPackets` (exe 0x5126b0), called by
`Fake_FRISSITES` (exe 0x4de720) after the whole tick body, stamps it into the turn ring and, once the slot is ready (at
once in single player; every machine's ack in MP, 4 turns ahead), runs the `MULTIGET_*` handlers on it. So a message a
tick raises reaches the handlers before the next tick's body, on every machine with the sender's `EN` from the message.
Three come from the sim itself, and every peer raises them identically, so the driver delivers each before the next
tick (`Command::from_event` → `pending` in `ig2_net::lockstep::Peer::run_turn`; `World::tick` in single player) for
`Game::sender` (the state's `EN`, the canonical one in lockstep; a message raised while a bundle's order runs, such as an
answer's 0x66 on the messages screen, counts too: the original's UI loop sends it and it arrives after the next tick):
- 0x1b, an emptied fleet → `Game::fleet_emptied`;
- 0x66 (`st_Multi_include`, mode 1003, exe 0x5ee4d0: answer 2 to message 0x2347, payload 10031) →
  `MULTIGET_Story` (exe 0x5c2f00, ELF 0x740670) → `Story_Special(15, sender, id)` = `Command::StoryChoice`;
- 0x49 (the elem stories, exe 0x5d4b70: `Multi_Send(0x49, EN, spy, st_GetKuldetesID(id), 0, 0, 0, 10)`) →
  `MULTIGET_Spy` (exe 0x5883c0) = the spy screen's send: `SpyOrder::Send { target 0, 0, 0, days 10 }`. Campaign only.

In canonical MP the story's human is the canonical `EN` (`Game::message_order`); another human's message answers
reach the story through that human's inbox (§5.3).

### 5.6 Renames (ours; deviation 395)

The original renames a fleet or a planet on its own machine only (`Starmap_Main` / `Colony_Main` with `IOMode`, no
message): the machines' names differ. In canonical MP the names are synced state (in the snapshot and the hash), so a
rename travels as `Command::Rename { fleet, index, name, custom }` when it ends (Enter, Esc with the old name, a press
elsewhere); `Game::rename` writes the name (a fleet: at most 0x13 characters, only its owner's; a planet: at most 0x17,
any one, as the original's rename allows) and a fleet's custom-name bit (`+0x52 |= 0x800`, kept for good, also after
Esc). While the rename runs and until the lockstep has carried it the client shows the typed name and the bit from
`Game::local_names` (`edit_local_name`), put in while it holds the game and taken out before the game goes into the
lockstep (`show_local_names`, with `world::local_en`), so every peer's hash sees only the shared names.

### 5.7 Order owner checks (ours; deviation 414)
Issue #24 (`Game::foreign_planet`/`foreign_fleet`): in multiplayer every peer refuses starmap and colony orders on another empire's fleet or planet (the exe's `MULTIGET_Starmap`/`MULTIGET_Colony` check no owner); satellite, colonise, watch, queue, production, transfer and ground orders keep their own checks. Test: `ig2-net/tests/lockstep.rs`.

## 6. Open

- The unowned planets' richness by the host's race (`GetBolygoGazdagsag`) and radar row 0: a fidelity question
  only; the original machines likely disagreed here.
- Per-peer fleet visibility: resolved (U16, `fleet-movement.md` §6.4).

## 7. The real-time battles (ours; implemented)

The original already runs MP battles by session rules, not by a screen: a real-time battle (space: two human sides;
ground: two human sides) sets an in-progress flag on every machine (0xBAD114 space, 0xBAD13C ground with the planet
at 0xBAD140), every machine stops its clock and strategic systems while it is set, `FRISSIT_SpaceWar` ticks every
battle slot and `FRISSIT_Grwar` runs `WF::MI`/`WF::Refresh` for the flagged planet on the 20 Hz loop, and the humans'
input reaches the battle through the lockstep queue (`space-combat.md` §7.12–§7.13, `ground-combat.md` §11.17). The
canonical view (`game::realtime`; `ViewMode::Classic` runs none of it, so SP and every replay are unchanged) keeps
that and fixes the per-machine parts:

- Synced: the flags, the planet, the vegetert list, the battle slots, the live ship-type table and `AktSpaceWar`
  (`Game::realtime`, snapshot version 4).
- While a flag is set, `Game::advance_clock` doesn't step the clock and `Game::run_tick` runs the battle branch
  (`GroundWarSetBattleStatus`, `IdoMultiPlayerBattle`, `FRISSIT_Grwar` unless a space battle runs,
  `FRISSIT_SpaceWar`); otherwise the normal tick ends with `FRISSIT_SpaceWar`.
- The space battle is shown (every human is sent to its screen); no screen fade; the battle and the ground war see
  `MultiPlayerMode` = 1 (MP landing, retreat, end and story rules) whatever the state says.
- `IdoSpeedSzorzoBattle` (unsaved) is set by `Game::canonical_view` when the state has none, as MP `SetIdoSpeed`
  would for the game speed.
- The orders: `Command::SpaceOrder { battle, order }` (messages 0x70..0x77, 0x44e, 0x44f, `ig2_net::BattleOrder`) and
  `Command::SpaceBattleLeft` (0x44d), applied at the tick boundary; `MULTIGET_SpaceWar`'s tail
  (`Game::space_battles_settle`) runs before every clock step. Ours: an order naming units or guns that don't exist,
  or another empire's (the screen only sends the player's own), is refused on every peer alike, and a move target
  gets the screen's clamp (`Battle::screen_point`).
- The ground war's orders: `Command::Colony` with `ColonyOrder::Ground` (0x22..0x25, 0x12d, 0x12f, 0x130, 0x134,
  `War::order`). Ours: refused unless the planet is the flagged war's, the unit is alive and the issuer's, and for
  retreat and air strikes the issuer is a participant (owns the planet or a live unit there) (`Game::colony_order`).
- The ground war's dropships fly once per sim tick (the 100 Hz UI's every fifth pass); `WF::Refresh` ends the war
  as soon as one side holds the field (MP has no countdown); the capture follows (`Game::with_wars`).

### 7.1 The session around a battle (ours, ADR 0011)

- **Start**: `Game::start_multiplayer(en, humans)`: `MultiPlayerMode` = 1, `PlayerKi` = 1 for exactly the session's
  humans (a former human not in the list: 0), canonical view of `en`. The original's MP routing then sends only
  human-vs-human battles to real time; every human is on the screen, the participants order, the others watch.
- **Cadence**: one battle step per sim tick (two per turn), orders `delay` turns after they're issued like every
  command; the strategic clock waits, the turn clock doesn't.
- **Hold**: `Host::hold(bool)` stops and resumes the bundles (every peer stalls at the same turn). The session
  layer holds while a dropped human may come back during a battle (`Game::realtime_running`) and for a pause.
- **Away/back**: `Command::PlayerAway` / `PlayerBack`, sent only by the host (`Host::inject`, stamped with the
  dropped empire; a peer's copy is dropped by `Host::pump`). Away = the empire has left every running space
  battle's screen (`SpaceWarVege` waits only for the others); back = that mark is cleared while the battle runs.
- **Leaving**: the strategic clock waits until every human's 0x44d (`Game::space_battle_waits_on`). Ours: a machine
  without the battle's screen up while it waits on its empire (a rejoin, a resumed save, a missed start, a battle
  decided under Options) opens the screen, or sends 0x44d at once when the battle is decided (the client's
  `mp_battle_check`); the screen's end sends no pause in MP (`SetIdoPause` changes nothing there; sent as a command
  it would pause every machine). A turn's events reach the screens even when a drop, kick or save ran it. Tested by
  `lockstep.rs::a_rejoined_player_without_the_screen_lets_the_battle_end`.
- **Desync, rejoin, MP save**: the battle is in the snapshot and the per-turn hash (version 4), so all three work
  mid-battle through the ordinary paths (ADR 0007 §3–§4).
- **Parity**: the host's bundle log replayed into one `Game` reproduces every turn's hash the peers reported.

Tested by `ig2-net/tests/lockstep.rs::two_humans_fight_a_real_time_battle_in_lockstep` (two humans fight with
orders, the clock waits, both leave, `SpaceWarVege` ends it, 600+ turns hash-identical, a mid-battle snapshot
restores to the same hash) and `two_humans_fight_a_ground_war_in_lockstep` (with orders: the attacker's tanks take the planet; another
empire's units refused), `a_player_drops_mid_battle_and_rejoins` (hold, away, rejoin from a snapshot and the log,
back, the battle waits for the returning player; the host's log replayed into one `Game` gives every turn's hash),
`a_desync_inside_a_battle_is_reported_and_resynced` and `recorded_ground_orders_cross_the_wire` (the 407 ground-war
messages of the `grt-orders` recordings survive the input and bundle encoding).

### 7.1.1 Desync soak (ours; `ig2-net/tests/soak.rs`)

Generated-galaxy MP games of 2-4 humans plus AIs on loopback links (`HostSession`/`ClientSession`); every peer issues
random orders of every kind from its own state (fleets, planets, production, colony buildings, research, designs,
info shares, trade, diplomacy, spies, message-screen orders, renames), fights the real-time space battles and ground
wars, with an MP save (restored and re-hashed) at a third and a drop + rejoin at a half. Every pair of peers must agree
on every common turn's hash and the host must report no desync. Tests: `two_humans_and_two_ais_stay_in_step` (3000
turns, "even wars": debug switch 0xB7723C plus a staged invasion so a human-vs-human ground war runs in real time),
`four_humans_stay_in_step` (1500), `the_seed_one_game_stays_in_step` (6000), `a_long_soak` (`#[ignore]`).
Env: `IG2_SOAK_TURNS`, `IG2_SOAK_SEED`, `IG2_SOAK_SEEDS` (games of the long soak), `IG2_SOAK_STOP=T` (every peer stops
at turn T and prints the differing state parts and memory bytes, via `ig2_net::part_hashes`), `SOAK_EVENTS=1`
(event-kind histogram). Result: 6 long seeds x 6000 turns plus the defaults, no desync after the two fixes below; the
games contained real-time and auto-resolved battles, ground wars, conquests, empire deaths, trade, spies, designs.
It compares our peers with each other, not with the exe, so it does not close the untraced MP paths (U15, U18, U22, U23).
`ig2-client --playthrough mp` (`docs/BUILDING.md` → Play-through harness) repeats this on the client's MP path: a whole AI-played 2-machine game to its winner (~59,000 ticks, 98 save/resumes) with no desync.

- Events go through `Game::take_events()`: `Peer::run_turn` used to drain `game.events` with `append`, leaving
  `Game::hand_offs` (the battlefield hand-offs' place in the event list, `battlefield_hand_offs`) unreset, so the next
  tick skipped as many garrison hand-offs as the previous tick had events, which differ between peers (view-dependent
  events): planet records and a tank slot differed on one machine. Also fixes MP garrison hand-offs skipped on all peers.
- `Command::apply` drops a `DiplomacyMessage`/`DiplomacyAnswer` whose `to` is not 1..=8 or whose terms fail
  `Terms::in_range(planets)` (money >= 0; radar/war/peace empire 1..=8; planet 1..=planet count; tech 0..193; hostage
  1..=5; negotiate sub 0..=8 with third empire -1..19; talk sub 0..=9; special sub 0..0x100; other categories refused).
  Out-of-range terms crashed `Diplomacy::deal_value` and `make_peace`; the original only receives what its own screen
  built, so the drop is the same on every machine (unit test `command::tests::terms_out_of_range_are_refused`).

### 7.2 The session layer (ours, ADR 0011 §4–§6; `ig2_net::session`)

No UI; the lobby screens sit on top. Time is the caller's (`now`, any monotonic origin), so the policy is
deterministic under test; nothing in it touches the sim except the injected commands, which travel in the bundles.

- **Start** (`HostSession::start(lobby, game, rules, settings)`): `Game::start_multiplayer(host's empire, the
  members' empires)`, `Game::libc` = the session id (ADR 0007 §5), `Snapshot::capture(.., 0, 0).to_file(rules, true)`,
  `LobbyHost::start`. The host's own player is an ordinary `Peer` on a loopback link. A joiner:
  `ClientSession::new(&start, link, rules)` from `LobbyClient::into_game`.
- **Drop** (a link reports `closed()`, or `HostSession::drop_player`): the host's peer first runs every bundle sent,
  then: a real-time battle runs (`Game::realtime_running`) → *waiting*: `Host::hold(true)` until the rejoin or the
  grace (`SessionSettings::grace`, default 60 s) → *away*: `Host::inject(empire, PlayerAway)`, the hold ends. No
  battle → *away* at once. Whenever a battle starts while an empire is away, `PlayerAway` goes out for it again
  (`Game::player_away` only marks the running battle). A dropped player's pause ends with it.
- **Rejoin** (`HostSession::rejoin(empire, link, now)`, the link admitted by the glue): the host's state at
  `Host::next_turn` as a fresh `Start` (`Msg::Lobby(LobbyMsg::Start)`) on the link, `Host::add_peer`, `send_log`
  from the snapshot's turn, the hold status, and `PlayerBack` if the empire was away. The rejoiner reads it with
  `ClientSession::wait_start`.
- **Desync** (ADR 0007 §3): on a new report the host's peer catches up; every peer whose hash for a reported turn
  differs from the host's own is restarted on its own link through the rejoin path (the host's state wins).
  `Host` ignores a slot's hashes for turns before it (re)joined, so the old peer's in-flight reports don't count;
  `ClientSession` swaps its peer when a `Start` arrives (`restarts`).
- **Pause**: `HostSession::pause` / `ClientSession::pause` (`Msg::Session(SessionMsg::Pause)`); the session holds
  while any human's pause stands. Every change of who is paused, awaited (with the grace left) or away goes to every
  peer as `SessionMsg::Status(HoldStatus)`.
- **Under Options, Help and full-screen videos**: the time runs on, as in the original (its Options pause,
  `SetIdoPause` 0x541300, changes nothing in MP): the machine keeps pumping its session and running the ticks due
  (`World::run_on`); the turns' events wait for the game screens. Ours for the videos (the original's `st_ShowAnim`
  0x602bb0 blocks, its network side unobserved): a video no longer silences the machine past the keep-alive. In MP
  the only videos are the machine's own (the story scripts run in SP only; the game-over clips are per
  `local_view().empire`). Ours: tested by `mp_lobby.rs::a_joiner_under_options_keeps_the_session_running`
  (a joiner under the screen past the keep-alive: turns go on, nobody is dropped).
- **MP save** (`HostSession::save`): the host's snapshot at the next turn as `Snapshot::to_file(rules, true)`, then
  `IG2RSESS`, a `u32` length and the `SavedSession` (name, scenario, delay, grace, the humans' empires and names;
  `wire` encoding). `MpSave::read` checks the MP flag and the rules fingerprint; `HostSession::resume(lobby, &save,
  rules)` needs the lobby's members to hold exactly the saved empires and starts the `Host` at the saved turn
  (`Host::at`; `LobbyHost::start` takes the turn from the snapshot's header).
- **Rejoin admission** (`HostSession::admit(link, now)`, after the start): the same door as the lobby's (ADR 0008
  §3.2, `lobby::Door`, kept from `LobbyHost::start` in `Started::door` with the lobby's `Settings`, session id and
  rules fingerprint): `Hello{version, rules, name, code}` → `Refused{Version | Rules | Code}`, then for a password
  session SPAKE2 and the key confirmation bound to the link (wrong → `Refused{WrongPassword}` after
  `WRONG_PASSWORD_DELAY`), and `Refused{TimedOut}` once `ADMISSION_TIMEOUT` passes or the link closes. Who comes
  back is the hello's name (truncated like the lobby's) matched against the session's humans (the names the
  members had in the lobby that started it, a resumed save's too): no human of that name → `Refused{Started}` (it is
  closed to newcomers); every human of that name still connected → `Refused{Connected}`. Both are checked at the
  hello and again once the password checks out. Admitted: `Welcome{member: the human's slot}`, then the ordinary
  rejoin above (`Start`, `send_log`, status, `PlayerBack`). The door runs on the session's clock (`now`), so the
  delay and the timeout are deterministic under test; `LobbyHost` feeds it `Instant` time since its creation. The
  joiner: `LobbyClient::rejoin(link, name, rules, password, code)` (the lobby's hello and password handling),
  `Status::Admitted` at the welcome, then `LobbyClient::into_link` and `ClientSession::wait_start`. The name is the
  only claim to an empire, as in the lobby anyone admitted may pick any open slot.
- **Keep-alive** (`SessionSettings::keep_alive`, default `DEFAULT_KEEP_ALIVE` = 10 s; `Duration::ZERO` turns it
  off): a silent peer counts as dropped even on a link that doesn't report `closed()`. Lockstep traffic alone can't
  tell: while the session is held (a pause, an awaited player) no bundles go out, so no peer sends inputs or hashes
  either. So both ends send `Msg::Session(SessionMsg::Alive)` at least every `keep_alive / 4` of the caller's clock
  (`ClientSession::pump(now, max_turns)`, `HostSession::pump(now, ..)` to every peer), held or not. The host counts
  every message a slot sends (`Host::heard(slot)`, the lockstep's own messages included; the relay keeps no clock);
  the session notes when a human's count last changed, and a human connected but silent for `keep_alive` takes the
  drop path (`HostSession::drop_player`: the link is let go, then *waiting* in a battle, else *away*). A (re)start
  counts as hearing from it. The host's own peer (the loopback in the same process) is never timed out. The joiner
  counts every message from the host the same way (its own `ClientSession::keep_alive`, default the same): `ClientSession::closed()` is the link's `closed()` or the host
  silent for `keep_alive` at the last pump. The keep-alive isn't saved (`SavedSession`); a resumed session takes the
  default. QUIC's own idle timeout and keep-alive pings (`quic.rs`) stay underneath; the admission's door has its
  own `ADMISSION_TIMEOUT`.
- **Host migration** (ADR 0012; ADR 0005 §5). Every `Start` carries `SessionInfo` (session id, name, scenario,
  private/password flags, the host's slot, every human's `(slot, name)`, grace and keep-alive in ms; lobby protocol
  version 2); the password stays with each joiner's glue. When `ClientSession::closed()` reports the host gone:
  - *Election*: `ClientSession::successors()` = the session's humans in slot order without the host's slot and
    without the empires the last `HoldStatus` lists away, waiting or migrating. The glue connects to the first that
    answers; the one that is first itself calls `HostSession::migrate(client, fingerprint, password, now)`.
  - *New host*: keeps its peer and state (no snapshot). Its `Host` (`Host::resumed(delay, log)`) continues at its own
    frontier (the next turn it holds no bundle for) with the bundles its peer kept (the last `KEPT_BUNDLES` = 128 run
    and every one buffered) as the log; its own peer moves to a loopback link and resends its unbundled inputs
    (`Peer::resume(from)`). Every other surviving human is *migrating* (since `now`), the old host's is *lost*; the
    session holds while any is either. Its door is the lobby's (`Door` from `SessionInfo`, the fingerprint, the
    password).
  - *Survivor*: `ClientSession::migrate(link, name, password, fingerprint)`: the rejoiner's hello (join code = session
    id) and password; at `Welcome` it sends `SessionMsg::Resume{turn, bundles}` (its peer's kept and buffered bundles).
    The new host adopts every bundle from its `Host::next_turn` on in turn order (`Host::adopt`: logged and sent to
    every peer, its own included), then, if that migrating human's frontier is reached by the log, `Host::add_peer`,
    `SessionMsg::Resumed{from}` (the peer resends its inputs for turns `>= from` no bundle covered and goes on from
    there), `send_log` from the survivor's frontier and the status. Otherwise (a gap, or no longer migrating) the rejoin
    path (`Start` from the new host's state). A survivor refused at the door hears `Refused` (the glue tries the next
    candidate on `Refused` or a connect timeout).
  - *End*: no human migrating (all resumed, or the grace since the migration began has passed: those take the drop
    path). Then each *lost* human (the old host's) takes the drop path on the state at `Host::next_turn`: *waiting* in
    a battle (the grace again), else *away*. From here the session is an ordinary one: rejoins and resyncs, pause, the
    keep-alive and the MP save (`SessionInfo`'s names and settings).
  - In-flight state: bundles the old host sent to some survivors only are adopted from whoever has them, so every
    survivor runs the same turns; inputs sent to the old host but never bundled are resent; the old host's own
    injected commands not bundled are lost (the drop path injects anew). Hashes from before a resumed peer's re-add
    aren't compared (`Host`'s `since`).

Tested by `ig2-net/tests/session.rs` (loopback, `quick-1.sav`): lobby → start → a real-time space battle with both
humans' orders, an MP save mid-battle resumed in a new lobby gives the same 60 turns' hashes; a pause holds every
peer; a mid-battle drop holds, the grace runs out on the test clock, `PlayerAway`, rejoin with `PlayerBack`; a drop
outside a battle is away at once; a drifted battle is resynced by the host; in a private password session a
mid-battle drop is turned away at the door for wrong rules, no join code, a wrong password (after the delay), an
unknown name and a connected one, a silent connection times out on the test clock, then the right name, code and
password come back (`Welcome`, `Start`) and a second claim is `Connected`; a joiner whose link stays up but goes
silent mid-battle is dropped (awaited) after the keep-alive and not before, then comes back through the door; a
paused session three keep-alives long with every peer pumping drops nobody; a joiner whose host stops pumping
reports `closed()` after the keep-alive; host migration with three humans: the host drops mid-battle while one
survivor (the successor) had stopped hearing it, both elect slot 1, it adopts the bundle only the other held, the other
resumes from its own state (no restart), 6 is awaited for the grace then away, 70 turns on with every turn's hash equal
to the old host's and each other's, and an MP save afterwards has all three humans; outside a battle with a survivor
a few turns behind, 6 is away at once and its player comes back through the successor's door. Every turn's hashes
agree.

### 7.3 The client glue (ours, ADR 0012 §2 and §6, ADR 0014 §4; `ig2-client::mp_lobby`)

- **Where a survivor can be reached.** Every joiner's client opens its own listener when its game starts
  (`Net::listen`: QUIC on `GAME_PORT`, else any port; no LAN announcement) and tells the host
  `SessionMsg::Reach { port, cert }` (`ClientSession::reach`; the port and the listener certificate's fingerprint).
  The host pairs it with the address it sees the joiner's link come from (`Transport::remote`, QUIC's remote address;
  a loopback link is `127.0.0.1`) and sends every connected human's `Candidate { slot, addr, cert }` to every peer as
  `SessionMsg::Reachable` whenever the list changes, and to a rejoiner, a resynced or a resumed peer. A survivor sends
  its `Reach` again to each new host (after `Resumed` or a `Start`). The old host's own address is never needed. The
  certificate travels from the trusted host, so a survivor pins the successor's listener by it.
- **Election and connect** (`Session::pump` when `ClientSession::closed()` and not kicked): the candidates are
  `ClientSession::successors()`, tried in order. This machine's own slot first → `HostSession::migrate(client, rules
  fingerprint, the password typed for the room, now)` and the joiner's listener becomes the new host's (it accepts the
  survivors through the door). Else `Net::connect(addr, pin = cert)` to the first candidate with an address; on a
  link, `ClientSession::migrate(slot, link, name, password, fingerprint)`; a connect error, `MIGRATE_CONNECT`
  (5 s) without a link or the door's `Refused` → the next candidate. No candidate left → the game ends for this machine
  ("The host has left the game.", main menu; `MultiCloseServer`). The message line says
  "The host has left the game. <name> hosts it now." on every survivor (ours, English).
- **Resuming an MP save from the room.** The connection page has a fourth button, "Saved game" (ours): the list shows
  the save folder's slots (`options::read_slots`) that read as MP saves with these rules (`MpSave::read`), titled by the
  slot name and date. Pressing a row selects it; Host (or a second press) opens a scenario-style room
  (`LobbyHost::new`, ADR 0008's rules: no galaxy options, every member must hold an empire and be ready) for the saved
  humans' empires, named after the saved session, with the typed password. The host sits in the saved empire of its
  name (else the first); a joiner whose name is a saved human's is seated in that empire as it joins (it may move to
  another open one). Start (the host's ready) needs the members to hold exactly the saved empires
  (`HostSession::resume`), else the line says "Every saved empire needs its player". Joiners start from the host's
  snapshot (`Start::snapshot`, as a rejoin), so they need no save file.
