# ADR 0061: Story mods: a story description format and its interpreter

Date: 2026-11-01. Status: accepted (parts 1 and 2). Builds on ADR 0044 (mods; §14 says why the campaign's own stories can't
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

   Part 2 adds (`empire` fields default to `"human"`):

   | Trigger | Value | Holds when |
   |---|---|---|
   | `all` / `any` | `[ trigger, … ]` | every / some listed trigger holds (they nest) |
   | `not` | `{ trigger }` | the trigger doesn't hold |
   | `battle` | `{ result = "won" \| "lost", kind? = "space" \| "ground", empire?, planet? }` | the check runs for a battle's end (`Story_SpaceBattleFinished`: `StoryEsemeny[3]`; `Story_GroundBattleFinishedCheck`: `[4]`) and `empire` won / lost it. Space: `empire` owns a participant (`SpwFlottak`) and is / isn't `SpWarKiNyert`. Ground: `empire` is the landing fleet's owner (as `Story_GroundBattleFinishedSave` saved it, exe 0xF26538) or the planet's owner when the battle started (`StoryAdat+0xf4`), and its side won (the won bytes, exe 0xF26532); `planet` limits it to a ground battle on that planet |
   | `tech` | `{ tech, empire? }` | `Talalmany[tech].+0x45 + e == 5` (researched); `tech` 0..=192 |
   | `money` | `{ empire?, at_least?, at_most? }` | `MoneyMennyi(e, 0)` is in the range |
   | `relation` | `{ a, b, at_least?, at_most? }` | `Viszony[a][b].+0x25` (1 war, 3 neutral, 4 non-aggression, 5 alliance) is in the range |
   | `spy_report` | `{ mission?, target?, success?, died? }` | the check runs for a spy report (`Story_KemEsemeny`: `StoryEsemeny[15]`, `KemkedesTipus == 3`; single player only, as in the original) whose `KemEsemeny` mission (`+0x14`), target empire (`+0x18`), success (`+0xc`) and death (`+8`) match the given fields |
   | `building` | `{ kind, planet?, empire? }` | a planet of `empire` (only `planet`, when given) has a finished building of kind `kind` (`Epuletek[p] + 0x20·k`: `+0 == kind`, `+8 == 0`) |
   | `timer` | a name | the mod timer runs and its day has come (it stays expired until stopped or restarted) |

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

   Part 2 adds:

   | Action | Value | Does |
   |---|---|---|
   | `give_tech` | `{ tech, empire? }` | `st_SetTalalmanyAllapot(tech, 5, e)` (exe 0x602920): researched, its research time from the race, the recompute; the stories' specials queue gets `(e, tech)` as for the built-in stories |
   | `remove_tech` | `{ tech, empire? }` | `st_SetTalalmanyAllapot(tech, 3, e)`: back to researchable (what was built with it stays) |
   | `change_relation` | `{ a, b, by }` | `Viszony[b][a]` value 2 (`+4`, i16) `+= by` (−10000..=10000; `st_ViszonyValt(a, b, x)` moves it by 100·x) |
   | `diplomacy` | `{ a, b, state }` | `"war"` `st_SetHaborusViszony` (0x602820), `"neutral"` `st_SetSemlegesViszony` (0x6028a0), `"non_aggression"` `st_SetNonagressionViszony` (0x602860), `"alliance"` `st_SetSzovetsegesViszony` (0x6028e0), `"trade"` story 9's trade agreement (`Viszony[a][b].+0x24 := 1` both ways) |
   | `add_building` | `{ planet, kind, level? }` | `EpuletNewSearchLocation(p, kind, true, level, −1, −1)` (`AutoAddBuilding`, 0x53cca0): a finished building placed for free (level default 0) |
   | `ground_units` | `{ planet \| fleet, design, count? }` | `TankNew(place, design, count)` (0x541750): `count` (default 1, ≤ 1000) units of the holder's owner's design slot into planet `planet`'s garrison (`place = −planet`) or into the live mod fleet with story id `fleet`; nothing when there's no such planet or fleet |
   | `start_timer` | `{ name, days }` | the mod timer runs out `days` days from now (restarting a running one) |
   | `stop_timer` | a name | the timer is gone |
   | `chain` | a story name | that story of the same file fires at once, its triggers and `every` ignored, unless it may no longer fire (a once-story that fired, a repeating one at its `times`) |

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

9. **Repeating stories, timers and chains** (part 2): a story with `repeat = true` fires in every check where its
   triggers hold, at least `every` days (default 1) after its last firing, at most `times` times (no limit when not
   given); `every` and `times` without `repeat` are errors. A timer is a named day: `start_timer` sets it, the
   `timer` trigger holds once it has come, `stop_timer` removes it; timers, like flags and counters, are shared by
   every enabled mod. A `chain` names a story of the same file (an unknown name is an error) and the file's chains
   may not lead back to the story they start from (an error naming it), so a chain always ends. Like a check, a chain
   sets the story's mark (once-stories) or its last firing and count (repeating ones) before its actions run.
10. **Part 2's state and the original's battle winner**: `ModStoryState` gains `repeats` (`(Ido of the last firing,
    times fired)` per repeating story) and `timers` (the `Ido` each runs out at); save format 15 (a version 14 state
    loads with both empty). The story engine reads `SpWarKiNyert` (exe 0xBAA1EC), which the battle code writes
    (`SpaceWarVege`, `SimulateSpBattle`), at each check: before part 2 the port's story engine kept 0 there, so the
    built-in stories that compare a battle's participants with the winner (`st_SpWarPart…`, `rnd` 118, `fable`,
    `begin`) now see the real winner, as in the original.

## Later parts
Planet info lines, a story starting a video by file name, and story texts as message parameters, as players ask.

## Consequences
- Mods can add story logic without code; the campaign's own stories stay as they are.
- The engine ships no story: the tests use a tiny made-up story over a synthetic world.
- Save format 14 (part 1), 15 (part 2); MP rooms compare the story files through the mods fingerprint.
- `docs/USER_GUIDE.md` has a worked example mod; a test parses it, so it stays valid.
