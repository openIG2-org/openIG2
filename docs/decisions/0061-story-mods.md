# ADR 0061: Story mods: a story description format and its interpreter

Date: 2026-11-01. Status: accepted (part 1). Builds on ADR 0044 (mods; §14 says why the campaign's own stories can't
be modded: they are program code). Implemented by `ig2_sim::story_mods` (`ModStory`, `parse`, `ModStoryState`,
`ModScripts`), `ig2_formats::vfs::Install::story_files`, the client's rules (`story.mods`), the snapshot's
`mod_stories` (save format 14) and `ig2-check`'s mods group.

## Context
A new campaign or a race's story needs story logic: "when the human owns planet 12 and has met the Zorgons, send
message 9001; when it is answered with part 2, give them planet 40 and spawn a fleet". The campaign's stories are
C code in the original and Rust in the port (`story.rs`, `storyline.rs`, `elem.rs`, …): there is nothing a mod can
replace. What a mod can already add is the data around them: message files (`uzenetek/txtNNNN.txt`,
`msgNNNN.txt`, ADR 0044 §1), text lines by key (`texts.<lang>.toml`, §10) and rule fields (`rules.toml`, §6). So the
story logic gets a declarative description that the sim interprets.

## Decision
1. **The file**: `stories.toml` at the mod folder's top (`mods/<name>/stories.toml`), TOML in UTF-8; not a game path
   (the file index skips it, like `rules.toml`). It holds `[[story]]` tables, each a story:
   ```toml
   [[story]]
   name = "greeting"                     # unique in the file; letters, digits, '_' and '-'
   when = [{ day = 30 }, { planet_owner = { planet = 12, empire = "human" } }]
   do = [{ message = 9001 }, { set_flag = "greeted" }]

   [[story]]
   name = "accepted"
   when = [{ answered = { message = 9001, answer = 2 } }]
   do = [{ give_planet = { planet = 40, empire = "human" } },
         { spawn_fleet = { id = 20001, owner = 3, planet = 40, design = 3, ships = 4 } },
         { add_counter = { name = "favours", value = 1 } }]
   ```
   A story fires **once**: the first story check in which every `when` entry holds (an empty `when` holds at once),
   its `do` entries run in order. Every `when` and `do` entry is a table with exactly one key, the trigger's or
   action's name.
2. **Triggers** (the story engine's reads, `StoryWorld` and the check's event flags):

   | Trigger | Value | Holds when |
   |---|---|---|
   | `day` | n ≥ 0 | the game time (`Ido`) is at least n days (`n · 50` ticks) |
   | `planet_owner` | `{ planet, empire }` | planet `planet` (1-based, as in `.scn` files) belongs to `empire` |
   | `empire_dead` | empire | `BirodalomEl[e]` is clear |
   | `flag` / `not_flag` | a name | the mod flag is set / clear |
   | `counter` | `{ name, at_least?, at_most? }` | the mod counter (0 when never set) is in the range (at least one bound) |
   | `fleet_at` | `{ fleet, planet, within? }` | a live fleet with story id `fleet` is within `within` (default 20) map units of the planet |
   | `answered` | `{ message, answer }` | the check runs for the human's answer `answer` to message block `message` (`st_Message`: `StoryEsemeny[6]` and `StoryMessageBlokk`/`Resz`) |

   An empire is 0..=18 or `"human"` (`EN`, the local human; each human in canonical multiplayer is
   `StoryWorld::human` on its machine's check, the same rule the built-in stories follow).
3. **Actions** (the story engine's events, `StoryEvent`, and its fleet helpers):

   | Action | Value | Does |
   |---|---|---|
   | `message` | block | `NewMessage(block, 0)` for the human (and `HumanMessage` for the other humans in canonical multiplayer); the message needs its row in `uzenetek/uzenetek.txt` (a mod's copy, ADR 0044 §1) and its `uzenetek/txtNNNN.txt` (and `msgNNNN.txt` for a conversation), its lines by key (§10) |
   | `ticker` | text key | `ScrollUzenet(_s(5, key))`: the scrolling ticker line `s_story.txt` key (a mod adds it by key, §10) |
   | `video` | text key | `st_ShowAnim(_s(6, key))`: the video the `s_anims.txt` key names |
   | `give_planet` | `{ planet, empire }` | `st_SetBolygoTulaj`: colonised when free, conquered otherwise, the empire's race |
   | `spawn_fleet` | `{ id, owner, planet, design, ships?, name? }` | `st_fNewEx(name, owner, id, 0, 2, planet)` then `st_fAddShip(id, design, ships)` (default 1): a fleet on the planet with `ships` of the owner's design slot; `name` a story text key, else unnamed |
   | `money` | `{ empire, amount }` | `st_AddMoney` |
   | `set_flag` / `clear_flag` | a name | the mod flag |
   | `set_counter` / `add_counter` | `{ name, value }` | the mod counter := / += value (wrapping i32) |
   | `win` | `true` | `MEGNYERES(1)`: the human wins |
   | `lose` | `true` | `MEGHALAS(EN)`: the human's empire dies |

   Fleet story ids are 10000..=32767, outside the campaign's (1..=868, 2301, 2311), so a mod's fleet is never one a
   built-in story searches for.
4. **Variables**: flags and counters are named (same character rule as story names) and shared by every enabled
   mod's stories, so one mod can react to another's. They, and the set of fired stories (`<mod>/<name>`), are the
   whole interpreter state.
5. **When it runs**: `Story_Check` runs the built-in scripts in the exe's order (`game::story_chain`, `multi` last);
   the interpreter, `ModScripts`, runs after them in every check (nested ones too, so the event flags an answer, an
   arrival or a battle sets are seen the same way the built-in stories see them). Mods run lowest priority first
   (`enabled.txt`'s last mod first, as patches apply, ADR 0044 §7), stories in file order. A story's fired mark is
   set before its actions run, so a nested check its actions cause can't fire it again. Everything it reads is
   synced state and it draws no random numbers except through the story helpers (`st_fNewEx`'s placement, stream 5,
   as the built-in stories): deterministic and lockstep-safe.
6. **Save and hash**: the state is ours, not the exe's: `SaveState::mod_stories`, in our save format from version 14
   and in the lockstep state hash (part `mod_stories`). Older saves load with an empty state. The original's `.sav`
   export doesn't carry it. Each active `stories.toml` is hashed into the mods fingerprint (ADR 0044 §4) as
   `stories.toml#<k>`, `k` its place in the apply order, so players with other stories don't share a room.
7. **Errors**: a file that doesn't parse, a `[[story]]` without a name or with a duplicate one, an unknown key at
   any level (trigger, action, field), a missing field, a value of the wrong kind or out of range (empire, planet
   ≥ 1, fleet id, day, ships 1..=100) fails the load naming the mod, the story and the entry; the client stops with
   the message (as for `rules.toml`). `ig2-check` checks each mod's file on its own, says how many stories it holds,
   and checks that every `message` block has its `uzenetek.txt` row and its `txtNNNN.txt`.
8. **Classic games**: without a `stories.toml` the interpreter has no stories and touches nothing: the sim runs as
   before, byte for byte.

## Later parts
More triggers (a battle's start/end and winner, a tech researched, money, relations, a spy report, a story's
counter-days timer) and actions (diplomacy, relation changes, planet info lines, buildings, techs, a repeating
story or a timer), and a worked example mod in `docs/USER_GUIDE.md`.

## Consequences
- Mods can add story logic without code; the campaign's own stories stay as they are.
- The engine ships no story: the tests use a tiny made-up story over a synthetic world.
- Save format 14; MP rooms compare the story files through the mods fingerprint.
