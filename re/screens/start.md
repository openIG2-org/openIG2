# The multiplayer game room (`Start`, `AktKep` 14) and our MP room

The original's room is the place where the host and the joiners of a generated MP galaxy pick slots, races and the
galaxy options before `InitNewGame(seed, 2, 0, 1)`. Our room replaces it (ADR 0009 §2, ADR 0014): it keeps the
original's **rules** (§1–§4, normative for `ig2_net::lobby` in galaxy mode) on our lobby protocol (ADR 0008 §3), and
is drawn in the style of the original's menus (§6), not as a copy of `StartGfx2D_1..3`. From the exe decompiles in
`_local/decomp/newgame-mp/exe/` and the ELF's in `.../elf/`; the exe wins. Addresses are exe unless marked ELF.
Strings are `GetString(14, section, n)` (`txt14`) unless another file is named.

## 0. Functions

| Part | exe | ELF |
|---|---|---|
| `Start_Belep` | 0x5bf4b0 | |
| `Start_Init` (defaults) | 0x5bfbf0 | |
| Click handler | 0x5c0120 | 0x723140 |
| AI pick | 0x5bd6c0 | |
| Join (host) | 0x5bf7d0 | |
| `Start_ClientDeleted` | 0x5bf940 | |
| `FRISSIT_Start` (host start) | 0x5bc590 | 0x728630 |
| `ReceiveStartData` | 0x5bc9d0 | 0x726f20 |
| Rejoin helper (host) | 0x5bc970 | |
| `Start_SendAktStatusz` (message 2) | 0x5c14e0 | 0x723c20 |
| `Newgame_Send(type, flag, len, data, to)` | 0x5141d0 | |
| `MultiStartGame` | | 0x5c4310 |
| Draw (not decompiled; our room doesn't copy it) | `StartGfx2D_2` 0x5be820 | 0x71ee80, 0x7208e0, 0x721610 |

## 1. Options (click handler 0x5c0120; defaults from `Start_Init` 0x5bfbf0)

The host owns every option; each change is broadcast as message 2 (§3). The handler's option block (ids
0x1F..0x32) runs only while `StartNowStarting` 0xF214A6 is 0 and checks no host flag (a joiner's edit would be
overwritten by the next message 2; ours: host only).

| Option | VA | Range | Default | Hot-spot ids |
|---|---|---|---|---|
| `HanyBolygo` (planets) | 0x912AC8 | 20/40/60/80 = (id − 0x21)·20 | 40 | 0x22..0x25 |
| `HanyGepiJatekos` (AI count) | 0xF21488 | 0..6 = id − 0x2A, then at most the free slots | 2 | 0x2A..0x30 |
| `ProgramGalaxyShape` | 0xBAE554 | 1..2 = id − 0x30 | 1 | 0x31, 0x32 |
| `MultiplayerBattleStatusz` | 0xBAE558 | 0..3, +1 mod 4 per click | 0 | click with `AktIkon2` 0x27 |
| `OnlyServerCanModifyTime` | 0xBAE55C | 0/1 = id − 0x28 | 0 | 0x28, 0x29 |
| `ProgramMode` | 0xBAE544 | 1..3 = id − 0x1E (connection kind, not a galaxy option) | MainScr's | 0x1F..0x21 |
| `ProgramSebesseg` | 0xBAE54C | fixed 4 (the host's in-game speed buttons act) | 4 | none |
| `Nehezseg` | 0x92827C | forced 1 by `InitNewGame(seed, 2, 0, 1)` | 1 | none |
| `ProgramPlayers` | 0x912ACC | fixed 8 | 8 | none |

- Battle status (the sim's readers, `ig2_sim::game::battles`, `ground.rs` `route`): 0 = space and ground battles
  in real time, 1 = space only, 2 = ground only, 3 = neither (both computed).
- Galaxy shape: stored, sent and saved; no reader in the generated galaxy (§3 of `rules/new-game.md` ignores it).
- No money, tech, difficulty or diplomacy options: the MP start forces them (`rules/new-game.md` §13).
- Strings: 14/2/1..4 "20/40/60/80 planets", 14/1/1 "AI players:", 14/1/3..10 "Player n race", 14/1/11 "Cancel",
  14/1/12 "Start game", 14/5/4..6 "Start", "Cancel", "Ready", 14/5/2 "Server IP address", 14/5/3 "Disconnected"
  (the line a joiner gets when the host is gone), 14/5/1 "Starting story set to" (the `mpstory` config line).
  14/3/1..2 "Normal game", "Fast game" and 14/4/1..5 "Level 1..5" have no reader in the decompiled room code.

## 2. Slots

Eight slots, slot = empire 1..8. `Start_Init`: `PlayerKi[i]` = 2 and `PlayerFaj[i]` = i for i = 1..18 (15..18:
races 1, 3, 1, 1), names "computer", no client; then `PlayerKi[1]` = 1 (the host is the human in slot 1, named
`MultiplayerNev`), then the AI pick.

- **AI pick** (0x5bd6c0, at the start and on every AI-count click): every non-human slot's `BirodalomEl`
  (0xBAE6AC + slot) = 0; `HanyGepiJatekos` = min(count, free slots); while fewer are placed:
  `rndlog(1, 1, 8)` (stream 1, start.cpp line 0x1BE), redrawn while that slot is human or already an AI; that slot's
  `BirodalomEl` = 1. Every call re-rolls the whole set. A slot neither human nor AI is empty (not in the game).
- **Join** (0x5bf7d0): refused (code 4) once starting; else the first slot that is neither human nor AI, else the
  first non-human slot (an AI slot taken over); none, or the 8-entry client list full: refused (code 5). The slot
  becomes human, not ready.
- **Move** (ids 0x33..0x3A, target slot = id − 0x32; joiner: message 8): only to a non-human slot; the two slots'
  per-slot fields (`PlayerKi`, `BirodalomEl`, name, client) swap, so an AI there moves to the mover's old slot. `EN`
  = the target. The race stays with the slot.
- **Race** (ids 0x3B..0x42, a popup of the 8 races; joiner: message 6 `slot << 16 | race`): `PlayerFaj[slot]`; the
  host applies a joiner's request with **no ownership check** (anyone sets any slot's race).
- **Leave** (`Start_ClientDeleted` 0x5bf940): the slot goes back to the computer (`PlayerKi` 2, "computer"); its
  `BirodalomEl`, race and ready stay. A joiner that lost the host shows 14/5/3 and leaves.

## 3. Messages and the start

`Newgame_Send` header: u16 type, u32 length, u16 flag. Message 2 (0x29C bytes, `Start_SendAktStatusz`) is the whole
room: `ProgramMode`, `Nehezseg`, `ProgramSebesseg`, `ProgramPlayers`, `GalaxyShape`, `ServerBirodalom`,
`HanyGepiJatekos` (recounted: non-human slots with `BirodalomEl`), `HanyBolygo`, `GameStarted`, battle status,
`OnlyServerCanModifyTime`, `PlayerKi[0..8]`, `BirodalomEl[0..18]`, `PlayerFaj[0..8]`, `PlayerNev` 9 × 11,
the client ids and IP strings. Other types: 1 chat, 3 the seed, 4 "start now", 5 kick/leave, 6 race, 7 start ack,
8 move, 9 name, 10 picture, 0x1E/0x1F rejoin save and ack, 0x20/0x21 ready, 0x8D IP table, 0x65 `mpstory`.

- **Ready** (button 0x44): a joiner sends 0x21 once (host chat line "Player %s ready."). The host's press is its
  ready and the start attempt: all humans ready and (humans + AIs) ≥ 2 → `StartNowStarting` = 1, message 2,
  message 4; each joiner acks with 7. Fewer than 2 empires: nothing (a lone host without AIs can't start).
- **Start** (`FRISSIT_Start` 0x5bc590, every frame once every human acked): `srand(SDL_GetTicks())`, seed =
  `rand() % 0x7fff` (the ELF has `random(0x7fffffff)`), message 2, message 3 (the seed), `MultiStartGame(0)`,
  `ProgramPlayers2` = `ProgramPlayers`, `InitNewGame(seed, 2, 0, 1)`, then the starmap. A joiner on message 3: the
  same `InitNewGame(seed, 2, 0, 1)` on the last message 2's room, `SetIdoSpeed(1, 1)`, `SetIdoPause(0, 1)`. So every
  machine generates the galaxy itself; only the room and the seed travel. `NEWGAME_Diplomacy` reseeds libc from each
  machine's own `SDL_GetTicks()`.
- **Start story** (`MultiplayerStartStory` exe 0xF2148C; the MP game modes, `rules/campaign/multi.md`): `Start_Belep`
  (0x5bf4b0) sets it from the config line `mpstory` (default "0") and, when non-zero, shows 14/5/1 + the id as a
  chat line. In the room (`AktKep` 0xE) `DoKeyboard` (0x4db1a0, the write at 0x4db7d2) takes a typed line that starts
  `story ` or `s ` (`_strnicmp`, case-insensitive) with `atoi(rest) != 0` as the new value and shows "Starting story
  set to " + the id (`Start_NewChatText(.., 2)`, this machine only; not sent); any other line is chat. Any id is
  taken, and on any machine, but only the host's `FRISSIT_Start` sends it: right after `InitNewGame`,
  `Multi_Send(0x65, EN, 4, &story)` when non-zero (read at 0x5bc66a); every peer's `MULTIGET_Story` runs
  `StartStory(id, 0)`. There is no menu option for it.

## 4. Our room's rules (ADR 0014; `ig2_net::lobby` with `LobbyState::galaxy`)

- Eight slots for empires 1..8, races 1..8 (slot n race n), all open; the host sits in slot 1; options as §1's
  defaults (`GalaxyOptions::default`). Options and slot kinds are the host's; any change clears every ready flag.
- AI count: the host's `set_options` re-rolls the AI set like 0x5bd6c0 (random free slots, at most the free slots;
  the host's own RNG, not the sim's stream 1: the lobby runs before `InitNewGame` reseeds it).
- A joiner is seated at once as §2's join (first empty slot, else an AI's); a full room refuses (`Full`).
- Move: a member may move to an empty or AI slot (the AI moves to its old slot). Race: a member sets its own slot's
  race, the host any slot's (ours: the original lets anyone set any).
- Leave: the slot becomes empty (ours: not the computer's; the host can give it to the AI).
- Start: every human ready and humans + AIs ≥ 2. The host's start sends each member `Start` with `galaxy:
  GalaxyStart { seed, ticks, options, races, ais, hash }`; every machine runs `Game::before_start` +
  `Game::start_mp_galaxy` (the host's empire as `EN`), `Game::start_multiplayer`, the session id as libc, and checks
  the start state's hash against the host's (`session::Error::StartMismatch`). The seed is `rand() % 0x7fff` after
  `srand(ticks)` on the host. `ticks` (the `NEWGAME_Diplomacy` reseed) is the host's and shared (ours: lockstep
  needs one state; deviation 223).

## 5. Our connection screen

Reached from the main menu's multiplayer rings (`mainscr.md` §6.5: "local" 0x23 and "inet" 0x22, after the name
entry). Lists the LAN sessions (`ig2_net::discovery::Browser`), hosts a session (`quic::QuicHost` + the LAN
`Announcer`), or joins one from the list or by address (`quic::connect_pinned`, `Pin::Fingerprint` from the
announcement, `Pin::Any` by address). A password field serves both: hosting with one makes a password session;
joining sends it to the SPAKE2 check. "inet" opens the same screen: no public matchmaking server ships (ADR 0005, 0018); a direct address join works across
the internet where the host's port is reachable. The status line (y 377) reads "No internet matchmaking: join by
address or join code".

**Relay server** (ADR 0018, `ig2_net::rendezvous`, `quic::{RelaySocket, QuicHost::bind_relayed, connect_via}`): the
optional field "Relay server (optional): " (box 130, 354, 380 × 18; kept in `relay.txt` in the save folder). Hosting
with it opens the room at the service; the join code then also carries the address the service sees. Joining by code
also tries the service (hole punch, then relay) in parallel with the code's addresses.

**Join by code** (`ig2_net::joincode`, byte layout in its module doc): the address field also takes a join code
(`JoinCode { session, pin, addrs }`: the session id, the first 12 bytes of the session certificate's SHA-256 as
`Pin::Prefix`, up to 4 IPv4 socket addresses; Crockford base 32 in dash-separated groups of five plus one SHA-256
check byte; case, dashes and blanks ignored, O reads 0, I/L read 1). The field takes up to 96 characters, shows 60
and scrolls. Join decodes the code, connects to every address in parallel, the first link up wins, and the hello
carries the session id (the private door's check). A long string without dots or colons that fails to decode shows
the decode error instead of a host lookup. Ctrl+V pastes (first line only) into the focused field. Codes are direct
addresses only (the host's LAN IPv4 and, when port mapping worked, the router's external one); no directory service.

## 6. Layout (ours, 640 × 480 virtual; the scenario panel's style: `mainscr.md` §9.3)

Panels are `kirak_fader` boxes over the main menu's 3D scene (the rings flown out, `Cel` 6), text in the charset
font (size 3 headings and buttons, 2 rows, 1 small values), palette colours 1 (normal), 2 (hovered/selected), 0x22
(values), 0x25 (labels); every press plays the menu click (`audio::CLICK`).

| Box | Rect (x, y, w, h) | Holds |
|---|---|---|
| Room: title | 16, 8, 608, 26 | the session name (size 3) |
| Room: slots | 16, 40, 380, 272 | 8 rows of 32 px from y 56: empire n, race (click: next, right click: previous), holder (human name, "computer", "-"; click: move there; host right click: empty → AI → closed), "Ready" |
| Room: options | 404, 40, 220, 272 | planets, AI players, galaxy shape, battles, speed control: label (0x25) and value (0x22) rows of 48 px with the host's ◄ ► arrows |
| Room: port mapping | 16, 316, 608, 40 | host only: row "Port mapping (UPnP / NAT-PMP)" (hot spot 0x72, 20, 318, 400, 16, click toggles; on when the room opens, ADR 0020): "asking the router...", "on, reachable at <ip:port>" or the failure text; right of it at x 476 "Private game: no/yes" (host-only hot spot 0x73, 430, 318, 190, 16, click toggles `LobbyHost::set_private`: no LAN announcement, the door asks for the session id; a joiner sees "Private game: yes" in colour 0x25); second row at y 338 "Join code: <code>  (click: copy)" (hot spot 0x74, 20, 336, 600, 16; click or Ctrl+C copies) or "Join code: none (no network address)"; with a relay set, a line at y 351 "Relay server: room open, seen at <addr>" (or opening / full / refused / no answer) |
| Room: chat | 16, 360, 608, 80 | the last 4 lines (y 364 + 14j), the entry line at y 424 (Enter sends) |
| Room: buttons | y 450 | Cancel (14/1/11), Ready (14/5/6) or Start game (14/1/12) |
| Connect | 120, 40, 400, 400 | title (15/1/29 "Local network" or 15/1/30 "Internet game"), the session list (10 rows), Server IP address (14/5/2) entry, password entry, Host / Join / Cancel |

## 7. Client status
`ig2-client::mp_lobby` (`MpScreen`, on `ig2_net::lobby`): §4–§6 done — the connection page (LAN sessions, host, join
from the list or by address, password), the galaxy room (slots, races, holders, the host's options, chat, ready,
start, the host's port mapping (on when the room opens, ADR 0020) through `Listener::map_port` / `port_map` and `ig2_net::portmap::open_live`, kept
renewed until the room or game ends; `launch` builds the game via `HostSession::start_galaxy` / `ClientSession::new_galaxy`), and a room for an MP
save (`MpScreen::host_save`). `mp_wait.rs` is the in-game wait overlay (`MOUSE_DrawOver`, input.md §9). The
original's own room drawing (`StartGfx2D_1..3`) is deliberately not copied (ADR 0009 §2, ADR 0014); the shared start
`ticks` is row 223; private rooms, join codes and the optional relay server (§5, §6) are done (clipboard through `arboard`, text only); the main menu's remaining MP-ring leftovers are row 157; the host's `story N` chat line picks the start story (§3, rows 418, 419).
