# ADR 0057: enhanced battle effects

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::battle_fx` (the layers, the procedural sprites, the
option), `ig2-client::spacewar_fx` / `spacewar_screen` (the space battle's feeds and draws), `ig2-client::colony_fx`
/ `colony` (the ground war's) and `ig2-client::display` (the "Battle effects" row).

## Context
The classic effects of both battles are the exe's (re/screens/spacewar.md §6.3–§6.8, re/screens/colony.md
§5.2–§5.11): white 5 × 5-sheet sprites for blasts and hits, puffs for trails, the shield ring model, debris models,
tracers and lightning ribbons. They are flat by today's standards: a ship's death is a few still sprites, a hit a
single flash, nothing lingers. The project ships no graphics (ADR 0001), so richer effects can only come from the
install's own textures or from pictures the client draws itself.

## Decision
1. **A "Battle effects" row** (`BattleEffects`, Graphics group; Classic / Enhanced; `--battle-effects` for one run).
   Classic by default. Classic draws and computes nothing new: the classic effects keep their own random streams
   and draw order, so the captures stay byte-identical.
2. **Extra layers on top, same triggers.** Enhanced keeps every classic effect and adds particles where and when
   the classic ones start, fed from the same sites (no sim change, no new random draws in the classic streams; the
   layers draw from their own xorshift stream):
   - a **blast** (a ship dying, a planet gun destroyed, a fighter group lost; the ground war's `Flash`): a hot flash,
     sparks flying out and slowing, a few embers (falling in the ground war), an expanding shock ring, and smoke;
   - a **hit** (a non-beam `Hit` on a ship; the ground war's `HitFlash`): a short flash and a spray of sparks;
   - a **shield hit** (`ShieldHit` on the planet's shield; a hit on a ship whose shields still hold): a blue-white
     flash and a quick ring;
   - **smoke columns**: a dying ship and a falling building (`BuildingDestroyed`'s ruin) leave an emitter that puffs
     smoke for a few seconds, rising in the ground war (−height), drifting in space;
   - **beam glow**: each projectile in flight and each laser bolt gets a soft additive halo in its own colour.
3. **Our own particle code.** One list per battle, stepped once per battle tick (the space battle's `Fx::tick`, the
   ground war's sim tick), capped at 4096 particles (the oldest go). Each particle has a place, velocity, drag, fall,
   age and life, a start and end size and a colour; it fades out over its life. Sizes are in the battle's units, set
   from the classic effect's size at the site.
4. **Sprites**: the glow is the install's `flare.bmp`, the smoke the install's smoke picture (`smoke.bmp`'s first
   cell in space, `rcloud.bmp` on the ground); the sparks' soft dot and the shock ring are drawn procedurally at load
   (64 × 64). Glows, sparks and rings are added (`ONE`/`ONE`, depth tested, no depth write, like the classic
   particles). On the ground the smoke darkens what lies below (the blob shadow's `ZERO`/`ONE_MINUS_SRC_COLOR`), so
   it reads as smoke over the bright terrain; in space dark smoke would vanish against the sky, so it is a faint dust,
   added.
5. **Classic stays the reference**: the replay tests and captures run classic; the layers are tested on their own
   (lifetimes, caps, determinism).

## Consequences
- Big battles draw more quads: the cost is measured in `docs/perf.md` (Measurements).
- The layers are additive glass mesh billboards, the kind the modern lighting's bloom (ADR 0055) takes: with both
  options on they glow; neither depends on the other.
- On snowy planets the added glows saturate against the white ground and show little; the smoke still shows.
- Shield hits on the ground: the ground war has no shields, so only the space battle shows them.
