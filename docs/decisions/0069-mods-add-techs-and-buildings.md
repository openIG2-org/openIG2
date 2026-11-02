# ADR 0069: mods add techs and building kinds

Status: accepted. Implemented by `ig2-rules::patch` (`add_new`, `Rules::added_techs`, `Rules::added_buildings`),
`ig2-sim` (`ColonyRules::role`/`kinds_like`, `ResearchRules::added_techs`, `Game::new`, `SaveState::unknown_kind`) and
`ig2-client` (`new_game::StartTexts`, `ui::UiTexts::load`, `research::tech_infos`, `world::check_kinds`). Part 1 of
"Everything moddable" (docs/ENHANCEMENTS.md); later parts add ship hulls, weapons, tanks, races, planet types.

## Context
Mods replace files and patch the values of the exe's rule tables (ADR 0044), but can't add entries. Two facts shape
how a new tech or building fits in:

- The live tech table is the exe's `Talalmanyok` (193 records of 276 bytes at 0xB9C0C8), held in the sim's memory
  image as the exe holds it; the next globals (`LaborSzintOsszes` 0xBA90E0, the building blocks) follow it. Saves name
  each tech's chunks (`Talalmanyok[t].status`, …). A tech's research-screen place is its number:
  category `(t − 1) / 16 + 1` (12 of them), field `(t − 1) % 16 + 1` (16 per category, the 16 orbit places of the
  ball); its name is `txt17` line `(17, 8 + (t − 1) / 48, (t − 1) % 48 + 1)`; the colony pick list offers techs
  81..=144 (categories 6..=9) and builds the kind in the tech's byte `+0x27`.
- 55 of the 192 tech numbers are empty: the exe's `-` records with every field and status zero. Building kinds are
  plain numbers (`u8` in the 32-byte instance); every per-kind table in `Rules` is a `Vec`, and the economy's effects
  are counted per kind and then read by fixed kind numbers (houses, food, hospitals, power plants, …).

Appending techs past 192 would move the globals after the table, break every save's chunk names and give the new
techs no category, field or name; filling the empty records keeps all of it.

## Decision
1. **A new tech takes a free field.** In a mod's `rules.toml`:
   ```toml
   [[techs.new]]
   key = "fusion_reactor"   # the mod's own id; other entries name it by this key
   category = 9             # research category 1..=12 (6..=9 the buildings')
   field = 8                # optional: 1..=16; else the category's first free field
   name = "Fusion reactor"  # shown where txt17's line for the slot is empty; a texts.<lang>.toml patch wins
   description = ["…", "…"] # up to 3 lines for the research screen's info box
   prerequisites = [135]    # up to 3 techs, by number or by an added tech's key
   ai_priority = 8          # optional: 0..=255 for every race (lower first); else the category's latest
   building = "fusion_reactor"  # optional: the kind it gives (number or added kind's key)
   model = "…"              # optional: the research screen's model (categories other than 6..=9)
   size = 1.0               # optional: the model's scale; else the category's first
   # and any `techs` field of ADR 0044 §6: price, build_time, hp, capacity, lab_levels, research_cost, research_time, firepower
   ```
   A field is free when its record is the exe's empty `-` (static fields and statuses zero) and no building prices
   itself with it (kinds 17, 18, 23, … point at tech 192). The record gets status 1 (unknown) for every race, so
   `Research_TalalmanyUjraSzamol` shows it once its prerequisites are owned, and the AI priority for races 1..=8.
2. **A new building kind is appended** after the originals (56, 57, …; at most 255):
   ```toml
   [[buildings.new]]
   key = "fusion_reactor"
   like = 55                # an original kind: its role, model, footprint, scaffolding and values to start from
   tech = "fusion_reactor"  # prices and names it; must be a building tech (81..=144)
   model = "…"              # optional: vector/colony/ model, preview, scale, preview_scale
   # and any `buildings` field of ADR 0044 §6: priority (0..=6), build_work, max_hp (> 0), energy, workers, output, flags
   ```
   A new tech that gives a new kind prices it, and a new kind's new tech gives it, when the other side doesn't say.
   Its name is its tech's name, as for every building.
3. **Roles.** The sim counts a new kind's effect, active and idle counts under the kind it is `like`
   (`ColonyRules::role`), and every fixed kind check goes by role: power plants and the hub produce energy,
   factories stop and pollute, solar and magma yields, trade ports, the hub's presence, diplomacy's planet value.
   Autobuild and the enhanced AI's building pick rank a new kind right after the original it is like
   (`kinds_like`): an empire that owns its tech builds it where it would build the original. The defences
   (28..=43) can't be copied: the ground war drives them by kind (later part). Story scripts keep their fixed kinds.
4. **New games and saves.** The new game copies `Talalmanyok0` and `EpuletekLeiro` from the exe image; with rule
   patches the client now overlays the patched tables on that image, so tech and building patches (ADR 0044) and
   added techs reach a new game (before, only loaded saves saw tech patches). A scenario's `rprojectallapot` line
   doesn't reset an added tech's statuses. A save keeps each tech's `+0x2c..+0x3c` (prerequisites, labs, research
   cost and time) besides the per-empire state; an added tech whose slot a save left empty (every status 0: saved
   without the mod) gets the mod's fields and status 1 when the game is made (`Game::new`). A save holding a kind
   the active rules lack refuses to load with a message ("enable the mods it was played with").
5. **Order, errors, check.** Added entries apply before the patch's `[<table>.<id>]` records, which may then set an
   added entry by its number; mods apply lowest priority first (ADR 0044 §7), so a later mod can name an earlier
   mod's keys. A missing key, a used key, a taken or out-of-range field, a full category, an unknown key reference,
   a `like` that is a defence or unused kind, a non-building tech, a priority over 6 or a `max_hp` of 0 fails the load
   naming the entry and field. `ig2-check` lists where each entry landed and checks every `rules.toml` alone and,
   with several, together.
6. **Multiplayer**: the `rules.toml` is in the mods fingerprint (ADR 0044 §9) and the added techs' state lives in the
   tech table, so the lockstep hash covers them.

## Consequences
- Classic play is byte-exact: without rule patches nothing is overlaid, `role` is the kind itself and no kind is
  `like` another.
- Capacity is the free fields: none in ships (1) and tanks (10); 1 in weapons, 6 radars, 3 shields, 6 bombs,
  3 civil buildings, 6 research, 2 military, 9 power plants, 3 turrets, 7 special. A tech outside the building
  categories is researchable but gives nothing yet (ships, weapons and tanks are a later part).
- ADR 0044 §7's "saves keep only the per-empire state" was wrong for `+0x2c..+0x3c`: a loaded save keeps its own
  prerequisites, labs, research cost and time; patches of those reach new games.

## Amendment (part 2): ship hulls, ship parts, weapons and tanks
Implemented by `ig2-rules::patch` (`like` on `[[techs.new]]`, `[[hulls.new]]`, `[[space_weapons.new]]`;
`Rules::added_hulls`, `Rules::added_weapons`, `Tech::like`, `Hull::like`), `ig2-sim` (`DesignRules::role`,
`DesignRules::hull_role`, `SpaceRules::added_weapons`, `World::slot_hull`) and `ig2-client` (the design screen's hull
list, `world::check_kinds`).

**Context.** Every ship and tank item is a tech: a design names its hull by `Alapok` index (`+0x44`, i32; the hull
names its tech) and its slots by tech number (`+0x51..`, u8). The exe recognises items by fixed tech numbers: weapons
0x11..=0x20 and 0x41..=0x48 (`t − 0x11`, `t − 0x34` into `fegyverek`), shields 0x31..=0x36, engines 0x39..=0x3d,
radars 0x21..=0x24, ground hull techs 0x91..=0x95 (`TankAlapSpeed[t − 0x91]`), tank turrets 0xa1..=0xa9, ground hulls
`Alapok` 0x13..=0x17 (`hull − 0x13` indexes the ground war's tables). Categories 1 (ships) and 10 (tanks) have no free
research field.

**Decision.**
7. **A part plays an original's role.** `[[techs.new]]` takes `like = <tech>`: an original tech of the ship or tank
   categories (1..=80, 145..=176). Every fixed tech-number check (the design stats' shields, engines, radars and
   stealth, the battle's pod effects and weapon, the ground war's pods, the AI's design rules, the research picker's
   category) reads the tech's role (`DesignRules::role`); prices, time, hit points, firepower and capacity come from
   the tech's own record. The tech is appended to every pod type (`PodTipusok`) that lists the original, so it fits
   wherever the original fits (at most 16 per pod type). Its research-screen model and size start as the original's.
   Without `like`, a new tech in categories 1..=5 and 10..=11 gives nothing (as in part 1).
8. **A new hull is appended** (`Alapok` 25, 26, …) as a copy of the original it is `like` (1..=24):
   ```toml
   [[hulls.new]]
   key = "star_galleon"
   like = 9                 # an original hull: class, flags, slots, models, battle ship type
   tech = "star_galleon"    # its tech (price, time, hp, firepower, capacity); the tech becomes `like` the hull's tech
   hp = 260                 # optional, ground hulls: a member's hit points in the ground war (`TankAlapHP`)
   flags = [0, 0, 0, 0, 0, 0]  # optional, as `[hulls.<id>]`
   ```
   Every hull-indexed table gets the original's entry at the new index: the slot layouts of all pod tables
   (`PodFajok`), the design screen's model (`Hull.id` stays the original's model index), the ground hit points, the
   starmap's transfer model. Tables the exe indexes by the hull's tech or ground index (`TankAlapSpeed`, turning
   speeds, armour, the tank models, the battle's ship type) are read through the role (`DesignRules::hull_role`).
   A ground hull (`like` 0x13..=0x17) is a new tank. The design screen lists every hull whose tech the empire owns.
9. **A new space weapon is appended** (`fegyverek` 40, 41, …) as a copy of the weapon it is `like`:
   ```toml
   [[space_weapons.new]]
   key = "plasma_lance"
   like = 3                 # an original weapon record (0..=39): graphics, kind, flags
   tech = "plasma_lance"    # the pod tech that mounts it; the tech becomes `like` the weapon's tech
   reload = 30              # optional: any `[space_weapons.<id>]` field
   range = 900
   ```
   The battle mounts a tech's added weapon first, else the role's (`AddUjUrhajo`); `fegyverszamol` gives the new
   weapon the damage formula of the original's tech with the new tech's firepower. A part `like` a weapon tech
   without a `[[space_weapons.new]]` fires the original weapon with its own firepower.
10. **The AI** designs hull slot `s` with the last added hull `like s` whose tech it owns (else `s`; a redesign
    upgrades the hull), and fits the best part per slot over every tech, new ones by their role's rules.
11. **Saves and multiplayer.** Designs are game state; hull and weapon records are rules (the `rules.toml` is in the
    mods fingerprint, ADR 0044 §9; the lockstep hash covers the designs and the tech table). A save whose design
    names a hull the active rules lack refuses to load ("enable the mods it was played with").
12. Errors: a `like` outside the ranges above (or a defence/building tech), an unknown hull or weapon, a tech
    that is not an added tech's or is already a hull's, a pod type over 16 techs: the load fails naming the entry.

**Consequences.** Classic play is byte-exact: without `like` the role is the tech itself and no table grows. A new
item looks like its original (models, sounds, battle type); its texts are its tech's (`name`, `description`, a
`texts.<lang>.toml` patch). Capacity is the free research fields (part 1).
