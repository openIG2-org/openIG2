# ADR 0080: Discord Rich Presence

Date: 2026-11-04. Status: accepted. Builds on ADR 0014 (our multiplayer screens and join codes), ADR 0042 (the
remake's texts) and ADR 0048 (the remake's options). Implemented by `ig2-client::discord` (the activity, the
driver), `ig2-client::discord::ipc` (Discord's local RPC), `ig2-client::discord::register` (the launch command),
`ig2-client::window::presence` (what the window reports, the joins) and `ig2-client::mp_lobby`
(`MpScreen::discord_room`, `MpScreen::join_from_discord`).

## Context
Players arrange multiplayer games on Discord and pass the join code there by hand. Discord's Rich Presence shows a
game's state on the player's profile and can carry a Join button whose secret reaches the friend's game. The
original has nothing of the kind; it changes nothing in the game itself.

## Decision
1. **One option, on by default.** "Discord status" (Options, Game; `DiscordPresence` in our settings file): Off,
   Status (the activity only) or With join (the activity and the room's Join button, the default). Off sends
   nothing and opens no pipe. Without a running Discord app nothing happens (the client retries every 5 s, up to
   every 60 s).
2. **Discord's local RPC, our own client.** The desktop app's IPC pipe (`\\.\pipe\discord-ipc-N` on Windows, the
   `discord-ipc-N` socket under `XDG_RUNTIME_DIR`, `TMPDIR` or `/tmp` and the Flatpak and Snap folders elsewhere):
   frames of `opcode u32 LE, length u32 LE, JSON`, the handshake with the application id `1567637511340036228`,
   `SET_ACTIVITY` and `SUBSCRIBE` to `ACTIVITY_JOIN` and `ACTIVITY_JOIN_REQUEST`. A worker thread owns the pipe;
   the window reports once a second and the worker sends only a changed activity, at most one per 4 s (Discord takes
   5 per 20 s; the latest wins). No new dependency: `serde_json` and the standard library (one `PeekNamedPipe`
   import on Windows). The browser build has no pipe and reports nothing.
3. **What is shown.** In the menu: the page (main menu, choosing an empire, a scenario, looking for a multiplayer
   game, the match history, replays, mods, settings). In a room: its name, the state (waiting for players, ready),
   this member's race and the party size (humans of the seats humans may take). In a game: the mode (campaign,
   scenario, single player, tutorial, multiplayer), the race and difficulty, the screen (starmap, colony, ground war,
   space battle, research, design, diplomacy, spy, trade, info, messages, settings, help; a video shows what was shown before it), paused or
   watching as a defeated player, the game's date, and the local empire's planets, population, ships, credits, score,
   techs and battles won (`ig2_sim::stats::sample`, the synced statistics). A replay says so. The elapsed time
   restarts when the player moves between the menu, a room and a game. The texts are the remake's (English, Hungarian
   for a Hungarian game); the race names are the game's own, read from the install.
4. **Join is the join code.** A host's room that is not private and has a free seat puts its join code (ADR 0014) in
   the activity's join secret, with a party id every member derives alike from the room's and the host's names. A
   friend's Join reaches their game as `ACTIVITY_JOIN`: over the main menu the connection page opens (as the
   Internet ring opens it) and joins the code as if pasted; a game running keeps the join until the main menu is up;
   a member already in a room drops it. A password room still asks for its password. A friend's "Ask to join" is
   accepted at once while the room offers Join (and turned down otherwise), and the room's chat says who is coming.
   A started game offers no Join (a room can't be joined once started). For Discord to start a friend's game, the
   client registers its executable as the `discord-<application>` URL scheme the first time it reports
   (`HKCU\Software\Classes` on Windows, a `.desktop` handler in `~/.local/share/applications` on Linux), as Discord's
   own RPC library does; nothing is written when it is already there.
5. **The pictures are ours.** The activity names art asset keys of the application on Discord's side: `logo`,
   `race_1` to `race_8` (a mod's race shows the logo) and the small icons `menu`, `lobby`, `starmap`, `colony`,
   `ground_war`, `space_battle`, `research`, `design`, `diplomacy`, `spy`, `trade`, `info`, `messages`, `options`,
   `replay`, `paused`. They are drawn for the project, uploaded in the Discord Developer Portal, and never
   taken from the game's install. Without an activity Join the activity has two link buttons (the releases, the
   website).

## Consequences
- What a player's Discord friends see: the game's state as above, the room's name and, with Join on, a way into an
  open room; a private room or Off shows none of it. The join code itself is only handed to the friend's Discord
  client, as a pasted code would be.
- The settings file gets one key; the launch command is the only thing written outside it.
- Tests: the frames, the events and a Discord stand-in on a Unix socket (`discord::ipc`), the launch entries
  (`discord::register`), the activity for every kind of state, the Join rules, the Hungarian texts and the limits
  (`discord`). What the profile looks like and a Join between two Discord accounts are checked by hand.
