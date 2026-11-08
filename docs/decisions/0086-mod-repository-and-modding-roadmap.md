# ADR 0086: a mod repository, modding tutorials and what to open to mods next

Date: 2026-11-08. Status: **proposed** (draft for the maintainers' decision; nothing outward-facing is built until it
is accepted). Extends ADR 0044 (file overrides, rule and text patches), ADR 0061 (story mods), ADR 0068 (zip mods,
`mod.toml`, the Mods page) and ADR 0069 (new techs, buildings, hulls, weapons, tanks, races, planet types). The
packages are the proposal rows 547–559 in `docs/ENHANCEMENTS.md`.

## Context

What a mod can do today (each with its ADR and the user guide's Mods section):

| Capability | How | ADR |
|---|---|---|
| Replace or add any install file (textures `.pvr`, models `.xv`, texts, sounds, videos, `.scn` maps, message scripts, `building.txt`, `designs.txt`, `values.txt`, `fleet.txt`) | same path in the mod; plain or packed form | 0044 §1–5 |
| Change rule values | `rules.toml`: `[<table>.<id>] field = value` for 10 tables (techs, buildings, hulls, space and ground weapons, ship types, spy missions, problem curves) and 25 lists (race, difficulty, radar, spy) | 0044 §6–9 |
| Add techs, buildings, hulls, space weapons, parts, tanks, races, planet types | `[[<table>.new]]` with `like` = an original to copy | 0069 |
| Change or add text lines | `texts.<lang>.toml` (6 languages, English as the fallback) | 0044 §10–13 |
| Add stories (triggers → actions, repeating, timers, flags) | `stories.toml` | 0061 |
| Sharper or replacement textures and models (look only) | `hires/<game path>` | 0054 §5–7 |

Packaging: a folder or a `.zip` (read in place) in the `mods` folder beside the executable or the settings folder;
`mod.toml` (name, author, version, description, picture, game); `enabled.txt` (order = priority); the Mods page
(switches, order arrows, problems, overridden files, Apply, Restart); a broken mod is skipped and the game starts.
Multiplayer hashes every active override and patch into the room's fingerprint (`hires/` excluded) and refuses a
joiner with other mods. Nothing in a mod executes: the formats are TOML, PNG and game data; zips are checked (no
absolute or `..` paths, no encryption or ZIP64, 65 536 entries and 1 GiB per file at most). `openIG2-check --mod`
validates mods against the install.

What is missing:

1. **Finding mods.** A player finds a mod somewhere on the web, downloads a zip and drops it in a folder; updates are
   by hand; nothing says which mods a multiplayer room needs (the room only refuses).
2. **Making mods.** The reference is complete but dense; there was no step-by-step start. The rule ids and current
   values come from `ig2-rules-dump`, which only a source build has; text keys must be looked up in the specs. A mod
   has no stable id, no dependency or compatibility range and no licence field.
3. **Reach.** Defences, ship and tank research fields, scenarios as data, sounds and models for new entries, the AI's
   weights, new languages and the browser build are still closed; logic beyond the story rules needs code.

## Decision (proposed)

### 1. The mod repository: a GitHub index, read in the client

**Options compared**

| | GitHub index repository + release assets | mod.io | Self-hosted on the project's server |
|---|---|---|---|
| Cost | none | none for the game; their terms and business model | the server's storage and traffic; weeks of building and upkeep |
| Upload | a pull request to the index (a GitHub account); the client can prepare it | in the client or on their site (a mod.io account) | in the client (our accounts) |
| Moderation | every entry is reviewed before it merges; issues for reports; revert to take down | their tools, reports and staff, plus ours | all ours: review, reports, takedowns, abuse |
| Safety | reviewed entries, SHA-256 per file, a signed index; zips stay data-only | their virus scan; our client must still enforce data-only and hashes | as we build it |
| Versioning, compatibility, dependencies | ours, in `mod.toml` and the index | their metadata, partly fits ours | ours |
| Offline | the cached index and the local zips | their SDK caches; the local zips | as we build it |
| Privacy | GitHub sees the player's address on download | an account for subscriptions; their data handling | ours to run under GDPR |
| Lock-in, openness | plain files anyone can mirror or fork | a third-party service; an unofficial engine for someone else's game may not be accepted (to be asked) | ours |
| Client work | an HTTPS fetch, a signature check, a zip download | their REST API and auth flows | the most |

**Recommendation: the GitHub index**, at least until the mod count or the review load outgrows it; mod.io stays the
later option if players want in-client uploads, ratings and subscriptions (after asking mod.io whether an unofficial
engine qualifies). Self-hosting is the most work and the most liability for the least gain.

How it works:

- **The index**: a public repository (`openIG2-org/mods`) with one file per mod release,
  `mods/<id>/<version>.toml`: id, name, author, version, `requires` (openIG2 range), `depends`, `conflicts`,
  licence (SPDX), description, picture URL, download URL, size and SHA-256. The download is a release asset of the
  author's own repository (or of the index repository when the author has none). The maintainers' CI checks every
  pull request: the schema, the download's size and hash, the zip's safety rules, the file kinds (TOML, PNG, the
  game's data formats; no executables, scripts or archives inside) and the format checks that need no install
  (`openIG2-check --package` without an install: TOML parses, keys, story syntax). A merge builds one
  `index.json` and signs it with the maintainers' Ed25519 key (minisign format).
- **The client** (Mods page → Browse): the index is fetched over HTTPS only when the player opens Browse (an opt-in
  network use, said on the page; never at start), its signature checked against the key built into the client, and
  cached for offline use. Browse lists the mods compatible with this version (search, picture, description, author,
  licence, size, dependencies), with Install, Update and Remove. A download is checked against the index's SHA-256
  and the zip rules before it is moved into `mods/`; an update keeps the old zip until the new one is verified.
  Installed mods that have a newer compatible release show "Update". Mods installed by hand keep working as today.
- **Assets and licences**: the engine ships no original files, and the repository holds none either. No file from
  the game, decoded, converted or upscaled from it, may be uploaded; only the author's own work under a licence that
  allows sharing. Review asks for it; the client refuses a mod whose file is byte-identical to the install's (it
  would change nothing anyway). A takedown is a revert of the index entry; the signed index drops it at the next
  fetch.
- **Uploading**: the author runs `openIG2-check --package <folder>` (or the Mods page's Package button): it checks
  the mod, writes the zip, its SHA-256 and the index entry. Publishing is a pull request to the index with that entry;
  the Mods page opens the browser at the repository's "add file" page with the entry filled in. An in-client
  sign-in (GitHub's device flow) is a later option, not part of this decision.

### 2. `mod.toml`, version 2

New, optional fields (old files stay valid): `id` (a stable lower-case name; default the folder's name), `version`
(semantic versioning), `requires = ">=0.7, <0.9"` (replaces `game`, which stays read), `depends = { "zorg-race" =
">=1.2" }`, `conflicts = ["other-mod"]`, `license = "CC-BY-4.0"`, `kind = "visual"` (a look-only mod, outside the
multiplayer fingerprint, as `hires/` today). The Mods page orders dependencies under the mods that need them, refuses
a mod whose dependency is missing or whose range excludes this version, and says why.

### 3. Multiplayer

The room's admission message gains the host's mod list (id, version, SHA-256); a joiner whose mods differ sees which
ones, and with the repository, "Download and join" installs the missing ones (closing ADR 0068 §6's open item). This is
a lobby protocol version bump; the fingerprint stays the authority.

### 4. Tutorials

The first, `docs/modding/first-mod.md` ("Your first mod": a story with a ticker line and money, checked, switched on
and shared), ships with this ADR. The Mods page gets a "How to make a mod" link that opens it in the browser (the
repository's docs page). Planned walk-throughs, each a page in `docs/modding/` with a mod the reader builds:

1. Changing rule values (`rules.toml`, with the rule ids from the dump tool, §5).
2. Translating and editing texts (`texts.<lang>.toml`, finding a key).
3. A new tech and building.
4. New ships, weapons and tanks.
5. A new race and planet type.
6. Writing a story campaign (timers, flags, battles, diplomacy).
7. Texture and model packs (`hires/`).
8. Testing, packaging, publishing and multiplayer.

### 5. Modder tools in the release

`openIG2-check --dump-rules <file>` (today `ig2-rules-dump`, only in a source build) and `--dump-texts <file>` (every
text file's keys and lines) write the player's own install's values and keys for their own use; the files are never
shared (they hold the game's data), which the tutorials say.

### 6. Opening more to mods

Ordered by value for the effort; each is a proposal row with its size and risks:

| # | What | Effort | Risk (determinism, multiplayer, saves) |
|---|---|---|---|
| 552 | Defences (kinds 28..=43) as `like` sources; extra research fields so the ship and tank categories take new techs | 1–2 weeks | new kinds already refuse to load without their mod; research field layout is part of the save |
| 553 | Scenarios and galaxies as data: mod `.scn` files in the scenario list; a galaxy file (stars, planets, owners, start fleets) and custom-game presets | 2–3 weeks | start data only; the save holds the galaxy, so none after start |
| 554 | Looks and sounds for new entries: `model`, `icon`, `sound` fields on new techs, buildings, hulls and weapons; music tracks added to the playlist | 2 weeks | presentation only; kept out of the sim's state |
| 555 | The AI's weights as rule lists (research and build priorities, war and peace thresholds, aggressiveness) | 2–4 weeks | data in the fingerprint; the defaults must stay the original's exactly (replay tests) |
| 556 | Story mods, later parts: planet info lines, videos by name, story texts as message parameters, new messages with their talking heads | 2 weeks | story state is in the save (format bump per new state) |
| 557 | A sandboxed script layer (e.g. Rhai) for rules and events | months | the highest: determinism (no clock, no own RNG, bounded steps and memory), save state, speed; decided only after 552–556 show what stories can't do |
| 558 | New languages as mods (all text files, the language in the options) | 2–3 weeks | none for the sim; the game's font covers one code page, so a language needs its glyphs |
| 559 | Mods in the browser build (zips from the player's storage) | 1 week | as the desktop |

## Consequences

- Players install, update and remove mods from the Mods page; manual installs keep working; the game never needs the
  network unless the player opens Browse.
- The project maintains an index repository, its CI, a signing key (kept offline; the client embeds the public key
  and accepts a second one for rotation) and a review and takedown routine.
- `mod.toml` grows; old mods stay valid.
- Rejected: hosting mod files on the project's own server (cost and liability); mod.io now (a third-party account and
  terms for every author, and the question whether it accepts this engine); executable mods (native code, scripts
  outside a sandbox).
