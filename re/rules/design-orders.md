# The design orders: `MULTIGET_Design` (exe 0x4b4d60, ELF 0x50c430)

The Design screen's and the starmap info panels' lockstep messages 0x33..0x35: a new or edited design, the sale of a
design's units (and the design's retirement), and the refit of a design's units to its upgrade target. Called every
frame by `MultiSendPackets` (exe 0x5126b0) like the other `MULTIGET_*`; the messages go through the queue in SP too.
Port: `ig2-sim::game::design_orders` (`DesignOrder`, `Game::design_order`, `ig2_net::Command::Design`). Verified by
the exe replay (`re/rules/traces/2026-10-22-design-orders.md`).

## 1. Functions

| Name | exe | ELF | Role |
|---|---|---|---|
| `MULTIGET_Design` | 0x4b4d60 | 0x50c430 | the handler (§2–§5) |
| `Multi_GetNextTipus(0x33, 0x3b)` | 0x513340 (calls 0x4b4d7a, 0x4b5740) | | the next queued type in range; 0x36..0x3b have no case here |
| `Multi_Get` | 0x513160 (calls 0x4b4dd4 / 0x4b5328 / 0x4b565e) | | one message's issuer and fields |
| `HanyHajoVanATervbol(e, h, d)` | 0x4b4410 | 0x506520 | units of design `d` or upgradable to it (§6) |
| `TervHajoinakAra(e, h, d)` | 0x4b70b0 | 0x506880 | their price (§6) |
| `GetTervEladasOszto(e, d)` | 0x4b4370 | 0x506c40 | the resale divisor (`ai-empire.md`'s `CsokkentettTervAr`) |
| `TervHajoinakCsokkentettAra` | inlined | 0x506e00 | `trunc((f32)TervHajoinakAra / div)`, 0 for `div < 0` |
| `TervAzOsszesHajotAtalalitja(e, h, d)` | 0x4b6ee0 | | units whose design upgrades to `d` become `d` (§6) |
| `TervAzOsszesHajorolAtalalitja(e, h, d)` | 0x4b6d60 | | units of `d` become its upgrade target (§6) |
| `WF::ChangeDesign` (no name) | 0x4a78d0 | | a battlefield's units of a design take another (§6) |
| `TervNew(e, 0)` / `TervCopy2(e, rec, d)` | 0x5422f0 / 0x541ca0 | | `ai-empire.md` §10.2; `TervCopy2` = 0x84-byte copy |
| `TervDelete(e, d, h, 1)` | 0x541d50 | | `ai-empire.md` §12.8 |
| `st_CheckTradeTervAr(k)` | 0x5d4850 | | `TradeTervek[k] +0x24 = (int)((f64)p * 0.8)` when `StoryAdat+0x60` |
| `Design_UjraRendez` / `TervRendezoKeres` | 0x4b3cb0 / 0x4b72b0 | | the Design screen's sorted list (UI only) |

Notation: `T[e][d]` = `Tervek[e][d]` (0x84 bytes; `+0x16` i16 class, < 0 deleted; `+0x18..+0x1a` u8 flags; `+0x1c`
i32 upgrade target; `+0x20` i32 build time; `+0x24` i32 price; `+0x44` hull type; `+0x6c..+0x7c` the retirement:
units, `+0x70` u8, year, month, day). "UI" = `e == EN && AktKep == 5 && DesignTervezesMod == 0 &&
!DesignSelectDeleteMode` (exe 0x912a3c, 0xbae0fc, 0xa3f0b4, 0xa3f0b8): the Design screen's list follow-up, not sim
state. Money: `MoneyMennyi(e, 0)` = the total; `MoneyHozzaadLog(e, 0, x)` (with the AI's income factor for x > 0).

## 2. 0x33: a design (`Multi_Send(0x33, EN, 4, &DesignTervezesMod, 4, &AktTerv, 0x84, rec, 1, &flag)`, exe 0x4b1b20)
Fields: `mode` i32, `d` i32, `rec` 0x84 bytes, `flag` u8.

- **mode ≠ 3** (new): `n = TervNew(e, 0)`, `T[e][n] = rec`. UI: re-sort, `AktTerv = n`.
- **mode 3** (edit `d`): `old = (f32)T[e][d] +0x24`, `new = (f32)rec +0x24`, `bc = 0`.
  - `flag == 0` (in place): `cost = max(0, (HanyHajoVanATervbol(e, 0, d)·rec+0x24 − TervHajoinakAra(e, 0, d))·2)`
    (i32); when `(f64)cost ≤ money` or `cost < 1`: `MoneyHozzaadLog(e, 0, −cost)` (design.cpp 0x969), `T[e][d] =
    rec`, UI re-sort, `TervAzOsszesHajotAtalalitja(e, 0, d)`, `bc = d`, and every battlefield's live units of `e`
    with design `d` (`alive`, `flags & 0x4020000 == 0`) get design `d` again and `WF::CheckDesign` (planets
    1..=`HanyBolygo`, exe `WF[p−1]` at 0x9c3b30 + 0x8ec·(p−1)). Not affordable: nothing changes, `bc` stays 0.
  - `flag & 1` (a new copy): `bc = TervNew(e, 0)`, `T[e][bc] = rec`; `T[e][d] +0x18 = 0, +0x19 = 1, +0x1a = 1, +0x1c
    = bc`; every `k` in 1..=`OsszHanyTerv[e]` with `+0x16 ≥ 0` and `+0x1c == d` gets `+0x1c = bc`. UI: re-sort,
    `AktTerv = bc`. No money.
  - Then, either way (also when nothing was affordable), the queues of every planet 1..=`HanyBolygo` owned by `e`
    (`Bolygok +0x20`), lines 0 and 1 (`+0x78 + 8·line`, four i16 designs; progress i32 `+0xa0 + 4·line`):
    - When the head `S0 == d` or `T[e][S0] +0x1c == bc`: `c = T[e][d] +0x20` (after the copy), `x = (f32)c −
      ((f32)(c − prog)·old)/new` (SSE f32, exe 0x4b5206..0x4b5249), `prog = trunc(clamp(x, 0, c))` (`x < 0` → 0, `x >
      c` → c; NaN → c).
    - `flag & 1`: each of the four words `S` with `S == d` or `T[e][S] +0x1c == bc` becomes `bc`.
    [original quirk: with `bc == 0` (unaffordable) the test matches lines whose design has no upgrade target.]

## 3. 0x34: a sale (`Multi_Send(0x34, EN, 4, &d, 4, &mode, 4, &h)`)
Senders: the Design screen (exe 0x4b2190, `h = 0`), the fleet panel (`mode 2`, `h = AktFleet`), the vehicle panel
(`mode 2`, `h = −AktPlanet`; `starmap.md` §19.7, §20).

1. `mode == 2`: `n = HanyHajoVanATervbol(e, h, d)`, `price = TervHajoinakCsokkentettAra(e, h, d)` (`div =
   GetTervEladasOszto(e, d)`: `price = div < 0 ? 0 : trunc((f32)TervHajoinakAra(e, h, d) / div)`, exe 0x4b5378..
   0x4b539f). When `n > 0`:
   - `MoneyHozzaadLog(e, 0, price)` (design.cpp 0x9d8);
   - unless `T[e][d] +0x44 == 5 && StoryAdat+0xac` (a story hero's ship): `T[e][d] +0x6c = n, +0x70 = 0, +0x74/
     +0x78/+0x7c = Ev/Honap/Nap, +0x1a = 1`; the first offer `k` in `TradeTervek[0..count)` with `+0x16 < 1`, else
     `k = count++` while `count < 40` (a full table: no offer); on a slot `TradeTervek[k] = T[e][d]`, `+0x24 =
     (+0x24 · rndlog(11, 110, 130)) / 100` (i32, design.cpp 0x9f0), `st_CheckTradeTervAr(k)`; then (also with a
     full table) `Story_Special(0x17, e, d, n, 0, 0, 0, 0)`.
2. Always: `h == 0` → `TervDelete(e, d, 0, 1)` (UI: `AktTerv` moves to the list's neighbour, re-sort); else
   `TervDelete(e, d, h, 1)` (only that holder's units; the design stays).

## 4. 0x35: a refit (`Multi_Send(0x35, EN, 4, &d, 4, &h)`; the fleet and vehicle panels)
`n = HanyHajoVanATervbol(e, h, d)`, `cost = max(0, (n·T[e][T[e][d]+0x1c] +0x24 − TervHajoinakAra(e, h, d))·2)`;
when `(f64)cost ≤ money` or `cost < 1`: `MoneyHozzaadLog(e, 0, −cost)` (design.cpp 0xa19),
`TervAzOsszesHajorolAtalalitja(e, h, d)`, `StarmapAktFlottaValtozott = 1` (exe 0xb9a961, UI).

## 5. After the loop (UI)
On the starmap's fleet view (`StarmapMode` 0xb9998c == 2) whose `AktFleet` the player can no longer see or own, the
first own idle fleet, else any own fleet, else any visible fleet becomes `AktFleet`; none: `StarmapMode = 1`. Not
sim state (the client's).

## 6. The unit walks
Holder `h`: 0 = every holder of `e`; a fleet; a planet as `−p`. Units: ships `Hajok[1..=n]` (6 bytes: fleet i16, ?,
design i16; owner = the fleet's `+0x15`), vehicles `Tankok[1..=n]` (holder i16, design i16, count i32; owner = the
fleet's, or planet `−holder`'s `+0x20`), stations `Bolygok[p] +0x100` (`+0x114` of them; owner `+0x20`; only for `h
== 0`). Design 0x7fff = an empty entry.

- `HanyHajoVanATervbol`: ships + vehicle counts + stations whose design is `d` or whose `T[e][design] +0x1c == d`.
- `TervHajoinakAra`: the same units, each adding `T[e][design] +0x24` (vehicles × count).
- `TervAzOsszesHajotAtalalitja(e, h, d)`: ships, vehicles and (h == 0) stations whose `T[e][design] +0x1c == d` take
  design `d`; `FlottakUjraSzamol()`.
- `TervAzOsszesHajorolAtalalitja(e, h, d)`: when `T[e][d] +0x1c ≠ 0`, ships and vehicles of design `d` take `T[e][d]
  +0x1c` (vehicles of a planet `h < 0` first rename the planet's battlefield units, `WF[p−1]` 0x4a78d0: live units of
  `e` with design `d` and `flags & 0x4020000 == 0` get the target, `WF::CheckDesign`); `FlottakUjraSzamol()`.
  Stations keep their design.
