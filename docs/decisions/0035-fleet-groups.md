# ADR 0035: fleet groups on Ctrl+1…9

Date: 2026-11-01. Status: accepted. A deliberate deviation from the classic client (ADR 0009), with the original's
keys kept as an option. Spec: `re/screens/starmap.md` §10.6, `re/screens/input.md` §1. Implemented by
`ig2-client::fleet_groups` and `ig2-client::window` (`fleet_group_key`).

## Context
The original selects a fleet with the mouse or steps through the own fleets with PgUp/PgDn (`StarmapKeyboardHandler`
exe 0x5aeab0). Long games have dozens of fleets; strategy players expect to bind them to number keys.

What the top row's digits do today (`remapKeyCodes` exe 0x4e4670 queues their key down, input.md §1):
- Every screen (`DoKeyboard` exe 0x4db1a0): 1..3 set the game speed and unpause (sound 10); 4..9 go on to the
  screen's handler. Neither tests Ctrl, Shift or Alt, so Ctrl+1 is speed 1 too.
- Starmap: the handler takes no digit (key down or character).

## Decision
1. **Option "Fleet groups"** (Remake row on the Options screen's game page, `FleetGroups` in the remake's settings
   file): on (1, the default) or off (0 = the original: Ctrl/Alt+1..3 set the speed, the rest do nothing).
2. On the starmap, by the key's place (the top row, any layout): **Ctrl+1..9** stores the selected fleet in that group
   (only a fleet of the human's; with a planet selected nothing changes). **Alt+1..9** selects the group's fleet as a
   click on it would (fleet mode, the info box); pressed again while it is selected, the camera centres on it
   (`StarmapKozepreAllit`, as PgUp/PgDn's step).
3. The plain digits keep the original's meaning (1..3 the speed): a group is recalled with Alt, not the bare digit.
4. A group whose fleet is gone (destroyed, merged, or no longer the human's) recalls nothing. Groups belong to the
   client of one machine, never to the sim: no lockstep order, nothing in the shared state, so multiplayer peers and
   recorded replays are unaffected.
5. Kept beside the save: every save (a slot, the quick save, the autosave) also writes `<save>.groups` next to it
   (one line per group, `group fleet`; `save<n>.groups` for slot `n`), loading that save reads them back; a new game, or a slot without the file, starts with none.
6. Not while a text entry has focus, a select menu, the fleet's panel or a transfer is up, under an overlay screen,
   or while the tutorial locks the starmap.

## Rejected
- A repeating production queue as a second enhancement: the original already has it (the production view's repeat
  lamp, order 0x1d op 3, `+0x90`; ported in `ig2-sim::production`), so there is nothing to add.

## Consequences
- A freed fleet slot can be reused by a new fleet of the human's; the group then recalls that fleet.
