# ADR 0086: a mod repository, modding tutorials and what to open to mods next

Date: 2026-11-08 (hosting revised the same day). Status: **proposed** (draft for the maintainers' decision; nothing outward-facing is built until it
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

### 1. The mod repository: a Thunderstore community, read and written from the client

The wish is a mod.io-like experience (browse, install, update, upload and rate from inside the game, with
moderation) that is free for the project and the players and free to use, copy and move. Researched 2026-11-08; the
service terms below are as their public pages state them, and every service's answer to "will you take this
project" is still to be asked (§1.4).

#### 1.1 The options

- **mod.io.** A closed-source commercial service with a REST API, SDKs and a web site. Its documentation says the
  core features "will always be free". Money comes from premium features, SaaS contracts with large studios and a
  marketplace cut. The API is free for permitted users under its API Access Terms, which require mod.io's branding
  wherever the API is used. Reading needs a per-game API key. Every write (subscribe, rate, report, upload) needs a
  player account, made by an emailed code or a platform or OpenID sign-in. Moderation tools are part of it (four
  layers, content approval, reports), and there is a separate test environment (`test.mod.io`). Adding a game is a web
  form; the public pages don't say whether an unofficial engine for someone else's game is accepted, so it must be
  asked. AGPL fit: the client may call a closed service, but the per-game read key would be in the public source,
  which needs mod.io's OK. Players pay nothing; the marketplace (paid mods) would stay off.
- **Thunderstore** (thunderstore.io; backend `thunderstore-io/Thunderstore`, **AGPL-3.0**, Django + PostgreSQL +
  Redis + S3-compatible storage, Docker Compose). A free, ad-supported mod site hosting many games in "communities"
  (Lethal Company, Risk of Rain 2, Valheim and others), most of them without the game's developer taking part. A new
  community is requested on its Discord and added by a pull request to its public `ecosystem-schema` repository. The
  stated requirement is existing mod authors' interest, ideally with mods ready to upload; developer permission
  isn't listed. Downloading needs no account. Uploading needs a GitHub or Discord sign-in on the site, or a team's
  service-account token for tools. Packages are zips with `manifest.json` (name, `version_number`, description,
  website, dependencies), `icon.png` and `README.md`. Pages show download counts and likes. The free mod managers
  r2modman (MIT) and Gale (GPL-3.0) already read its API. Moderation is the site's staff plus moderators per
  community, who can be the project's own people.
- **Modrinth** (backend `labrinth`, **AGPL-3.0**, Rust; web front end AGPL-3.0, app GPL-3.0). The most polished open
  platform, but its data model is Minecraft's (loaders, game versions) and the hosted site takes Minecraft content
  only. Self-hosting it means forking and reshaping it: weeks of work and a fork to keep up with.
- **ModHost** (MIT, Rust + Svelte). A small self-hostable mod host for any game, in beta, with a handful of stars.
  Too young to rely on.
- **A GitHub index** (this ADR's first draft): a public repository of TOML entries, CI checks and a signed
  `index.json`. Free and fully ours, but no in-client upload, ratings or subscriptions without our own server.
- **Self-hosting** one of the open backends. Free-tier storage covers the files: Cloudflare R2 has 10 GB-month free
  and no egress charge; Backblaze B2 has 10 GB free and egress up to three times the stored amount. The application
  needs a small VPS (a few euros a month at Hetzner or similar), backups of the database, updates, and someone on
  duty for moderation, abuse and GDPR requests. It isn't free in money or time.

#### 1.2 Compared

| | mod.io | Thunderstore, hosted | Thunderstore, self-hosted | GitHub index |
|---|---|---|---|---|
| Cost now | none | none | a VPS (€4–10 a month) + R2 free tier | none |
| Cost at growth | free core; terms can change | none (their ads pay) | storage beyond 10 GB, bigger VPS | none (GitHub limits per file 2 GiB) |
| Freedom to use, fork | closed service, their terms | AGPL backend; the packages are plain zips anyone can mirror | ours entirely | plain files |
| In-client API | full REST: browse, subscribe, rate, report, upload | REST: list and download without an account; upload with a token | same as hosted | read a JSON file |
| Ratings, subscriptions | both, server-side | likes and download counts; "installed" is kept by the client | same | none |
| Moderation | their tools and staff, plus ours | their staff plus community moderators we name | all ours | review before merge, all ours |
| Safety | their scan; we still enforce data-only and hashes | we enforce data-only and hashes on download | we can add server-side checks | CI checks, signed index |
| Privacy, GDPR | an account (email) for any write; mod.io is the controller | no account to download; the site sees the address | we are the controller | GitHub sees the address |
| Lock-in | high: API and accounts are theirs | low: the same backend can be run by us, mods move as they are | none | none |
| Approval | to be asked; unofficial engine unclear | to be asked; stated requirement is mod-author interest | none | none |

#### 1.3 Recommendation

**A Thunderstore community, hosted on thunderstore.io**, used through its API from the Mods page. It is the
closest free match to mod.io: browse, search, install, update and upload, likes and download counts, moderation
with our own community moderators, and no account for players who only download. It is free to the project and the
players, and it doesn't lock the project in: the backend is AGPL-3.0, the packages are plain zips, so the same client
code works against our own instance if thunderstore.io ever stops fitting.

**Fallback if Thunderstore says no or the terms stop fitting: run the Thunderstore backend ourselves** (Docker
Compose on a small VPS, files on Cloudflare R2's free tier, nightly database backups), with the same client code
pointed at our address. Cost: a few euros a month and the moderation and GDPR duty. If even that is too much, the
GitHub index of the first draft (no server, no cost, no in-client upload) remains the last resort.

**mod.io is a later, optional second source**, only if mod.io agrees in writing that it takes an unofficial,
open-source engine and that the read key may be public. Its subscriptions and server-side ratings are the one thing
Thunderstore lacks; it isn't worth an account requirement and a closed dependency as the only source.

#### 1.4 How it works

- **One mod, two manifests.** A package is a zip holding `mod.toml` and the mod's files (as today) plus Thunderstore's
  `manifest.json`, `icon.png` (256×256) and `README.md` at its root. The loader ignores those three files (they are no
  game files and stay outside the multiplayer fingerprint), so the same zip works from the site and dropped in by hand.
  `openIG2-check --package <folder>` writes all three from `mod.toml` (`id` → name, `version` → `version_number`,
  `depends` → dependencies as `Author-Name-x.y.z`) and refuses what the site would refuse.
- **Browse** (Mods page tab). Opens the community's package list over HTTPS only when the player opens the tab (an
  opt-in network use, said on the page; never at start) and caches it for offline use. It shows the mods compatible
  with this version (search, categories, icon, description, author, licence, size, likes, downloads, dependencies),
  with Install, Update and Remove. A download is checked against its listed size, against the zip rules and the
  data-only file kinds before it lands in `mods/`; its SHA-256 is recorded so a changed file is noticed. An update keeps
  the old zip until the new one is verified. Mods installed by hand keep working as today.
- **Rate and report.** "Like" and "Report" open the mod's page in the browser (both need the site's sign-in); the
  client stores no account.
- **Upload.** Package (above), then either the site's upload page (the Mods page opens it with the zip's folder) or,
  for authors who want it, Upload in the client with a service-account token the author pastes once (kept in the
  settings folder, never sent anywhere but the site).
- **Server address.** The site's address and community name are one setting, so a self-hosted instance (the fallback)
  or a test instance needs no code change.
- **Assets and licences.** The engine ships no original files, and no upload may contain one: no file from the game,
  decoded, converted or upscaled from it; only the author's own work under a licence that allows sharing. The
  community's rules say so, the client refuses a mod with a file byte-identical to the install's, and our moderators
  remove what breaks the rule.

#### 1.5 What needs the maintainers' approval or action

Nothing below is done; each step is the maintainers' own:

1. **Ask Thunderstore** (their Discord, `#game-requests`) for an openIG2 community: say it is a free, open-source
   engine that ships no original files, and ask whether that is accepted, whether the client may use the API to list,
   download and upload, and who may moderate. Approve the `ecosystem-schema` pull request text before it is opened.
2. **Create the accounts**: a Thunderstore team for the project (GitHub or Discord sign-in) and, for the release
   tooling, a service-account token kept out of the repository.
3. **Name the community moderators** and approve the community's rules (data only, own work, no original files,
   takedown on request).
4. **Optional: ask mod.io in writing** whether it accepts an unofficial open-source engine, whether the read key may
   sit in public source, and whether its branding rule fits the Mods page. Only if the answer is yes does a mod.io
   source get its own proposal.
5. **Only for the fallback**: choose a VPS and storage provider, the domain, who holds backups and keys, and the
   privacy policy, since the project then becomes the data controller.

Sources (read 2026-11-08): mod.io's API documentation (`docs.mod.io/restapi`), API Access Terms (`mod.io/apiterms`),
Game Terms and its support pages on premium features and the marketplace; Thunderstore's wiki page "Adding a new
game" and the `thunderstore-io/Thunderstore` repository; the `modrinth/labrinth` repository; the
`redstonewizard08/modhost` repository; Cloudflare R2's and Backblaze B2's pricing pages. Terms change: recheck them
before step 1.

### 2. `mod.toml`, version 2

New, optional fields (old files stay valid): `id` (a stable lower-case name; default the folder's name), `version`
(semantic versioning), `requires = ">=0.7, <0.9"` (replaces `game`, which stays read), `depends = { "zorg-race" =
">=1.2" }`, `conflicts = ["other-mod"]`, `license = "CC-BY-4.0"`, `kind = "visual"` (a look-only mod, outside the
multiplayer fingerprint, as `hires/` today). The Mods page orders dependencies under the mods that need them, refuses
a mod whose dependency is missing or whose range excludes this version, and says why.

### 3. Multiplayer

The room's admission message gains the host's mod list (id, version, SHA-256); a joiner whose mods differ sees which
ones, and "Download and join" installs the missing ones from the community (matched by name, version and SHA-256) (closing ADR 0068 §6's open item). This is
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
- The project depends on thunderstore.io for hosting, with its own instance of the same AGPL backend as the way out;
  it names community moderators and keeps the community's rules.
- `mod.toml` grows; old mods stay valid; packages also carry Thunderstore's three files, which the loader ignores.
- Rejected: mod.io as the only source (closed service, an account for every write, branding duty, acceptance
  unclear; a later second source if it agrees); self-hosting from the start (cost and duty with no gain while the
  hosted community is free); the GitHub index as the main way (no in-client upload or ratings; kept as last resort);
  Modrinth's backend (Minecraft-shaped) and ModHost (too young); executable mods (native code, scripts outside a
  sandbox).
