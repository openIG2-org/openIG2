# `.scn`: scenario and campaign state (text)

Status: **syntax verified** on all 29 shipped files (`ig2_formats::scn`, golden test
`every_scn_parses_with_known_keywords`). Field semantics are Phase 2 work.

Files: `dataraw/scenario/*.scn` (campaign chapters `530`…`730`, skirmish scenarios),
`dataraw/tutorial/*.scn`. Wrapped in `ecGs` like everything else.

## Evidence

| Symbol (ELF) | Address | Role |
|---|---|---|
| `LoadScenario(const char*, int)` | 0x62b880 | parser: 24k lines at -O0, 225 keyword checks |
| `SaveScenario(const char*, const char*)` | — | writer (not yet read; gives exact output order) |
| `Betolt_Scenario(const char*)` | — | wrapper: `InitNewGame` → `LoadScenario` → `FelfedezBirodalom` → `SingleStartGame` |
| `GetInstantParam` / `GetInstantParamF` | — | value parsers (atoi- and atof-like) |

Exe counterparts (ported names): `LoadScenario` @0x545b40, `Betolt_Scenario` @0x4f5ba0.

## Syntax

- Text in the language's 8-bit codepage, CRLF line endings.
- `;` starts a comment line.
- A line is `<keyword> <values…>`. Leading whitespace is ignored (indentation is cosmetic).
- Keyword match: `strncasecmp(line, kw, strlen(kw)) == 0 && isspace(line[strlen(kw)])`.
  So matching is **case-insensitive and whole-word**. Shipped files use both `Diplomaciatiltas`
  and `diplomaciatiltas`.
- Values are whitespace- or comma-separated (`FLEET -9.81, -82.29 1`). Names keep embedded spaces
  (`pNev Central 7`, `tNev Tank 1`), so the reader keeps the raw rest-of-line.
- Numbers use atoi/atof semantics (`Money 50645.00` read as an integer gives 50645).

## Keywords

The full list in check order is in [scn-keywords.txt](scn-keywords.txt) (225 entries, extracted
from the `strncasecmp` call sites).

Record openers start a new record. Later field keywords apply to the newest record:

| Opener | Field prefix | Meaning |
|---|---|---|
| `SPY n` | `s…` | spy (name, race, portrait, skills, XP, rank, mission) |
| `TALALMANY` | `r…` | technology ("találmány" = invention): prerequisites, level, cost, time, per-empire state |
| `TERV n` | `t…` | design ("terv" = plan): ship/tank blueprint, pods, cost, build time, speed |
| `PLANET` | `p…` + `szin bolygomeret csillagmeret SpinSpeed Yaw Dist Speed` | planet |
| `BUILDING` | `e…`, `ex ey ez` | building on the preceding planet ("épület") |
| `FLEET x, y owner` | `f…` | fleet ("flotta") |
| `DUMMYFLEET` | `f…` | placeholder fleet |
| `VISZONY a b` | `v…` | relation between empires a and b ("viszony") |

Everything else is global: `Title`, `EN`, `Money` (per player), `Ev`/`Honap`/`Nap`/`Ido`
(year, month, day, tick time), `Program*` (mode, difficulty, players, galaxy shape), `PlayerKi`/
`PlayerFaj` (controller and race per slot), `RandSeed` (RNG state!), `BirodalomIsmeri` (contact
matrix, one line per empire), `LaborSzint`, camera state (`Starmap*`, `Colony*`), scenario switches
(`Diplomaciatiltas`, `tradetiltas`, `spytiltas`, `ViszonyNemValtozhat`, `Nehezseg`).

## Oddities

- `ProgramBattleMode` appears in 21 shipped files, but `LoadScenario` never checks it, and the
  string doesn't occur in either binary. It's dead data from another tool or version.
- `RandSeed` stores RNG state, so loading a scenario is deterministic. To map: which of the
  21 `rndold` streams these values seed (Phase 2, RNG spec).
- Several keywords are accepted but unused in shipped files (`pVirusIdo`, `fMeteor`, `szStart`,
  `pSpecials`, …). These are candidates for scripted campaign content.
