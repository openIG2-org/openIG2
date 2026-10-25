# Changelog

One line per finished feature, newest milestone first. Detail lives in the specs (`re/`) and in git history.

## M7: playable 1.0 (in progress)
- The host of an internet game can switch on UPnP / NAT-PMP port mapping in the room and sees the address players
  can reach it at.
- A host can make the room private and share a join code (copy and paste); a joiner pastes it into the address
  field and connects to the host's LAN or mapped address.
- Docs restructured: one task queue (`TASKS.md`), this changelog, `CONTRIBUTING.md`, `docs/ARCHITECTURE.md`,
  `docs/BUILDING.md`, `docs/USER_GUIDE.md`, `re/README.md` as the knowledge index; `re/deviations.md` keeps only
  the open, keep and enhanced rows (closed rows: git history).
- The client finds the GOG install itself (registry, GOG Galaxy, default folders) and checks `ig2.exe`; when none
  is found a window lists the places tried and takes the game's folder dropped onto it; the menu needs no save.

## M6: fixes against the original (2026-10-24 … 2026-10-25)
- Deviations triaged and about 480 rows closed: global input (`DoKeyboard` F-keys, gates, key repeat, the top bar
  on every screen), the game pointer and tooltips everywhere, the right button acting as the left, starmap, colony,
  ground-war, SpaceWar, design, options, trade, research, diplomacy, info, messages and spy input; render phases
  and looks (material transparency, LOD chains, environment mapping, panel lighting, shield rings, cloud shells,
  construction scaffolding, weather, smoke); the tutorial wiring (every event site, gate flags, effects).
- MP game loop: chat, `/kick` `/kill`, host-gated pause and speed, MP saves resumed from the room, host migration
  in the client, per-human story offers, renames through the lockstep.
- ADR 0015 (live overlay background), 0016 (deliberate keeps), 0017 (one language).

## M5: multiplayer playable
- The MP room (ADR 0014) on the session layer: lobby, connection, start, drops, rejoin and admission, resync,
  pause, keep-alive, host migration (ADR 0012), multiplayer saves; real-time battles in lockstep (ADR 0011).

## M4: input and feel
- Input audit of every screen against the exe's handlers; 16:9, 21:9 and 4K checks of every screen.

## M3: every screen
- Help, credits (with the remaster's text overlay), the tutorial overlay, high scores; a comparison sweep of every
  screen against captures of the original.

## M2: playable start to finish
- New game: campaign, scenario, MP galaxy, skirmish and tutorial starts, byte-exact against the original.
- Main menu (title, rings, race and difficulty, scenario/skirmish setup, MP name entry, exit animation), Options
  (settings, save and load slots, quit).
- Starmap (camera, orders, panels, radial icons, long info, select menus, fleet and vehicle panels), colony and the
  real-time ground war UI, SpaceWar (the live battle with effects, formations, sounds), research, design, trade,
  diplomacy, spy, info, messages.
- Audio (mixer, music selection and crossfade, every screen's sounds, speech and ducking, ground-war voices) and
  video (MP4 via OpenH264 + Symphonia: title, intros, story, endings, message heads).
- Game end: victory and defeat flow, end videos, high scores.

## M1: campaign
- All campaign scripts (`st_Elem_include`) and the story engine: storylines, random stories, fables, the
  multiplayer game modes; the tutorial interpreter (all five chapters).

## M0: foundations
- Asset formats (containers, PVR, `.xv` models, colony maps, scenarios, text, dialogue, story data), the rules
  tables read from the user's `ig2.exe`, original saves.
- The whole simulation, verified tick for tick against recordings: colonies, fleets and radar, space and ground
  battles (auto and real-time), research and production, diplomacy, espionage, the AI empires, trade, info and
  messages.
- Lockstep, QUIC transport, LAN discovery, port mapping; the screen kit, widescreen rule (ADR 0010), the capture
  comparison and sound tracer tooling.
