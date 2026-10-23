# Trade: the merchants, purchases and deliveries

Status: **verified at runtime** (`traces/2026-10-16-trade.md`): 1,640 whole-game ticks from an original save,
with staged deliveries and four purchases through the exe's own `MULTIGET_Trade`, byte-exact against
`engine/crates/ig2-sim` (`trade.rs`, `game.rs`, `tests/save_replay.rs`).

One per-tick system, `FRISSIT_Trade`, run every strategic tick after the daily block and before
`FRISSIT_Research` (`time-and-rng.md` §1), plus the purchase message the trade screen sends through the lockstep
queue (`MULTIGET_Trade`, also in single player). The market holds up to 40 **offers** (`TradeTervek`: copies of
design records with the merchant's count and date); a purchase becomes a **delivery** (`TradeSzallitas[e]`, up to
50 per empire) that counts down days and then builds the units at the buyer's planet. All randomness is
**stream 11**, apart from one libc `rand()` per new month that only picks the trade screen's default delivery
time.

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_Trade` | **0x605ed0** | 0x7b0ad0 | the tick (§3) |
| `MULTIGET_Trade` | **0x6070b0** | 0x7afa70 | a purchase arrives (§4), called by `MultiSendPackets` 0x5126b0 |
| `MegerkezikSzallitmany(e, delivery)` | 0x607280 | 0x7afee0 | a delivery arrives (§3.2) |
| `VanBolygoja(e)` | 0x60b3f0 | 0x7afd90 | `e` owns a planet with a working trade port (§3.3) |
| `MelyikBolygojanVanMegKereskedelmiUrkikoto(e)` | 0x607440 | 0x7afe30 | the last such planet (§3.3) |
| `st_GetRandomTradeTerv(&hull)` | 0x5eac80 | 0x72ffb0 | a random story design (§3.5) |
| `st_GetTerv(e, id, def)` | 0x5eacf0 | 0x72fcb0 | the design with story id `id`, made on first use (§3.5) |
| `st_CheckTradeTervAr(i)` | 0x5d4850 | 0x7372c0 | the story discount (§3.6) |
| `TradeAr(i, n)` | 0x608830 | 0x7ac600 | the price of `n` units of offer `i` (§4) |
| `DatumToIdo(ev, honap, nap, x)` | 0x53c2b0 | 0x620660 | `(nap + (honap + 12·ev)·30 − 31)·50 + x` |
| `SearchBuilding(p, kind, working)` | 0x541150 | 0x616720 | finished buildings of a kind (`working`: `+0x19 & 1`) |
| `TervNew` / `TervClear` / `TervEqual1`→`TervEqual2` | 0x5422f0 / 0x541be0 / 0x5421a0→0x5421e0 | | design records (§3.2; `ai-empire.md` §10.2) |
| `TervUjraSzamol(e, d, false, price)` | 0x5423b0 | | design stats (`space-combat.md` §2.4) |
| `SearchBolygohozKozeliFlotta` / `Legyart` | 0x540f50 / 0x59d740 | | delivery fleet and units (`production-research.md` §4, §5) |
| `MoneyHozzaadLog(e, k, x)` | 0x53fb10 → 0x53fbd0 | | money (`colony-side.md` §6.4; AI income scaled) |
| `DesignPoenosanElnevez(owner, hull, name, special)` | 0x4b19d0 | 0x5087c0 | a localised design name (string table 0x3b), no RNG |
| `Story_Special(0x13, e, d, n, p, 0, 0, 0)` | 0x5c9550 | | the story engine hears of the delivery |

**The exe is normative.** It follows the ELF statement by statement, except: the story merchant's per-human
hull check is a `switch` the ELF decompile couldn't recover (a jump table, §3.5), `st_GetTerv` only scans the
designs when there are at least 40, and the price factors are computed in f64 and stored as f32 (§3.4, asm
0x6062eb..0x6062ff: `cvtdq2pd; divsd 100.0; cvtpd2ps`); the price scaling is f32 (`mulss`, `cvttss2si`), the
story discount f64 (`mulsd 0.8`, `cvttsd2si`).

## 1. State

| Global | exe | Size | Saved | Meaning |
|---|---|---|---|---|
| `TradeHanyTradeCuccVan` | 0xF439CC | i32 | yes | offers on the market, `N` (≤ 40) |
| `TradeTervek` | 0xB9AAF0 | 41 × 0x84 | `N` records | the offers: design records (`space-combat.md` §2.3) |
| `TradeHanySzallitas` | 0xF66DE0 | i32 × 19 | yes | deliveries per empire (index 0 unused) |
| `TradeSzallitas[e]` | 0xF439D0 + e·0x1DB0 | 50 × 0x98 | `count` records | deliveries of empire `e` (`e` = 1..=18; empire 1 at 0xF45780) |
| `TradeOldNap` | 0xF66E2C | i32 | yes | day of the last daily pass (new game: −1) |
| `OldEtaHonap` | 0x928DB0 | i32 | no | month of the last delivery-time draw (new game: −1) |
| `AktEtaDays` | 0xF43428 | i32 | no | the trade screen's delivery time in days (20..=39) |
| `TradeSzallitasTemp` | 0xF43388 | 0x98 | no | the purchase message being built or read |
| `StoryAdat+0xc4` | 0xF274BC | i32 | yes | the story merchant's countdown in `Ido` units |
| `StoryAdat+0x60` | 0xF27458 | i32 | yes | story discount on new offers (§3.6) |
| `StoryDesignData` | 0xF2F8C0 | n × 0x94 | no | `designs.txt` (`story-data.md`), count `StoryDesignDataMut` 0xF419D0, next free id `StoryDesignElsoTerv` 0xF419D4 |
| `TradeCheat` | 0xF43835 | u8 | no | every own planet counts as a trade port for re-targeting (§3.3) |

An offer is a design record whose trade fields are: `+0x16` class (1 ship, 2 vehicle; **< 1 = the slot is
free**, −1 after a sell-out or expiry), `+0x24` price per unit, `+0x36` fighter flag (a unit = 5 fighters,
`TradeAr`), `+0x6c` units left, `+0x74/+0x78/+0x7c` the offer's year/month/day. The design it was copied from
keeps the same `+0x6c` (count), `+0x70 = 0`, `+0x74..+0x7c` (date) (these overwrite the design's spy
"fake figure" fields, `space-combat.md` §2.3), and `+0x80` is a story design's id (0 = none).

A delivery (0x98 B): `+0` planet, `+4` days left, `+8` offer index, `+0xc` units, `+0x10` price paid (i32),
`+0x14` the design (0x84 B, a copy of the offer at purchase time).

## 2. Constants
Offers 40 (`0x28`); deliveries per empire 50 (`0x32`); random merchant chance `rndlog(11, 0, 2000) <
IdoSpeedSzorzo` per tick; story merchant period `rndlog(11, 120, 200) × 50` `Ido`; expiry after
`DatumToIdo(0, 7, 1, 0) = 9000` `Ido` (180 days), then `rndlog(11, 0, 500) < IdoSpeedSzorzo` per day; trade port =
building kind 50 (`0x32`); story merchant's empire 9.

## 3. `FRISSIT_Trade` (every tick)

```
if OldEtaHonap != Honap: AktEtaDays = rand() % 20 + 20; OldEtaHonap = Honap      (libc, §3.1)
newday = TradeOldNap != Nap
if newday: TradeOldNap = Nap; countdowns (§3.2)
drop finished deliveries (§3.2)
check the destinations (§3.3)
merchants (§3.4, §3.5)
if newday: expiry (§3.7)
```

### 3.1 The monthly libc draw
`AktEtaDays` is only read by the trade screen (`Trade_Main`: `AktEtaDays2 = AktEtaDays` when the player picks a
destination; the purchase message carries it as the delivery's days). The draw shifts every later libc draw of
the tick (`game.rs` pins the libc state per tick, ADR 0003).

### 3.2 Countdowns and arrivals
```
for e in 1..=18, i in 0..count[e] (count re-read each step):
    D = TradeSzallitas[e][i]
    if D.days > 0: D.days −= 1
    if D.days == 0: MegerkezikSzallitmany(e, copy of D)
```
A delivery bought with 0 days arrives on the next day change; one with negative days never arrives and is never
removed. Then every delivery with `days == 0` is removed (each removal shifts the rest down with `memmove`; the
loop restarts until none is left: order kept), so a 0-day delivery that wasn't there at a day change is dropped
without arriving.

`MegerkezikSzallitmany(e, D)` (exe 0x607280):
```
T = D.design;  if T.class == 2: T.+0x40 = (i16)e              (a vehicle design becomes the buyer's)
d = 0; for k in 0x29..=OsszHanyTerv[e]: if Tervek[e][k].class >= 0 and TervEqual2(Tervek[e][k], T): d = k
if d == 0: d = TervNew(e, 0); Tervek[e][d] = T (all 0x84 bytes); +0x18 = 0 (not listed); +0x1a = 1
TervUjraSzamol(e, d, false, false)                           (stats; the price is kept)
f = SearchBolygohozKozeliFlotta(D.planet, 6.0, own, idle, false, false, 0)
Legyart(e, D.planet, f, d, D.units, 0, T'.class == 1, T'.class, T'.+0x36, T'.+0x37, T'.+0x48 == 0x50, single = true)
Story_Special(0x13, e, d, D.units, D.planet, 0, 0, 0)
```
`T'` = the design record after the recompute. `TervEqual2(a, b)`: same `+0x16`, `+0x40` (i16), `+0x44`, `+0x48`,
`+0x4c`, and for slots `k = 1..=+0x4c` the same `+0x50+k` and `+0x5d+k`. The last match wins. `single`: a fighter
unit delivers `n` fighters, not `5n` (`production-research.md` §5). `Legyart` makes a new fleet when `f == 0`
for ships; vehicles go into `f` if it has room, else to the planet's garrison.

### 3.3 Destinations
```
for e in 1..=18:
    repeat:
        changed = false
        for i in 0..count[e] (re-read; after a removal the next index is skipped until the next round):
            if Bolygok[D.planet].owner != e:
                changed = true
                if not VanBolygoja(e): MoneyHozzaadLog(e, 0, (f64)D.price); remove D
                else: D.planet = MelyikBolygojanVanMegKereskedelmiUrkikoto(e)
    until not changed
```
`VanBolygoja(e)`: some planet `p` in 1..=`HanyBolygo` with owner `e` and (`SearchBuilding(p, 50, true) > 0` or
`DEBUGX`). `Melyik…(e)`: the **last** such planet, where `TradeCheat` also counts. The refund goes to pool 0
(split by the shares) and an AI buyer's refund is scaled by its income multiplier (`MoneyHozzaadOld`).

### 3.4 The random merchant
```
free = 40 − N + #{i < N : offer[i].class < 1}
StoryAdat+0xc4 −= IdoSpeedSzorzo                          (every tick)
if free < 1: done
if rnd(0, 2000) < IdoSpeedSzorzo:                         (all rnd = rndlog(11, lo, hi))
    e = rnd(1, ProgramPlayers); kind = rnd(1, 2); n = rnd(1, 3)
    c = rnd(1, 3) == 1;  f = rnd(1, 10) == 1;  b = rnd(1, 20) == 1;  s = rnd(1, 30) == 1
    if kind == 2:  c = f = b = s = false
    elif s:        n = 1; c = f = b = false
    elif b:        c = f = false
    elif f:        c = false
    dear = rnd(1, 3) != 1 or f or b
    factor = (f32)((f64)(dear ? rnd(110, 140) : rnd(70, 120)) / 100.0)
    if c: n = 5n
    tries = 0
    loop:
        d = rnd(0x29, OsszHanyTerv[e]); tries += 1
        if tries >= 500: done (no offer)
        D = Tervek[e][d]
        if D.class != kind or D.+0x80 != 0: continue
        plain = D.+0x34..+0x36, +0x38 all 0
        accept if (D.+0x36 == 0 and plain and not (c or f or b or s))      (an ordinary design, no special wish)
               or (D.+0x36 != 0 and c)                                     (fighters, ×5)
               or (D.+0x34 != 0 and f) or (D.+0x35 != 0 and b) or (D.+0x38 != 0 and s)
                                                     (colony ship, terraformer, movable base)
    if D.+0x37 != 0: done (a space station is never offered)
    offer (§3.6) design d of empire e, count n, factor, named after its hull tech
```
`OsszHanyTerv[e] < 0x29`: `rnd` returns 0x29, a record past the table (the exe reads the heap there; the port
reads it as empty and rejects it). Empires are drawn among all `ProgramPlayers`, dead or alive, the human too.

### 3.5 The story merchant (when the random merchant didn't come)
```
elif StoryAdat+0xc4 < 0:
    StoryAdat+0xc4 = rnd(120, 200) × 50
    e = 9; tries = 0
    loop:
        id = st_GetRandomTradeTerv(&hull); if id == 0: done (no offer)
        ok = true; owned = false; ht = Alapok[hull].tech (+0xc)
        for p in 1..=ProgramPlayers with PlayerKi[p] == 1:
            if hull has a prerequisite R (table) and status[R][p] != 5: ok = false
            if status[ht][p] == 5: owned = true
        if tries < 5 and owned: ok = false
        tries += 1; if tries >= 500: done (no offer)
        until ok
    d = st_GetTerv(9, id, 3)
    big = Tervek[9][d].+0x48 > 4;  n = big ? 1 : 5
    factor = (f32)((f64)(owned ? rnd(80, 120) : rnd(110, 140)) / 100.0)
    offer (§3.6) design d of empire 9, count n, factor; name: owned → its hull tech's name;
          else big → DesignPoenosanElnevez(+0x40, +0x44, name, rnd(0, 1)) + " " + the story design's name
```
Prerequisites by hull (`switch` at exe 0x606486): 4 → tech 2, 6 → 5, 7 → 6, 8 → 7, 9 → 8, 10 → 9, 11 → 10,
20 → 0x91, 21 → 0x92, 22 → 0x93, 23 → 0x92; other hulls have none. So the merchant prefers a design whose hull
the humans can build but don't have yet; after five tries it settles for any buildable one.

`st_GetRandomTradeTerv(&hull)`: `id = rndlog(11, 10000, StoryDesignElsoTerv − 1)`; the first `StoryDesignData`
record with that id gives `hull = +0x24` and returns `id`; none: 0. `st_GetTerv(e, id, def)`: if
`OsszHanyTerv[e] ≥ 40`, the first `k` in 40..=`OsszHanyTerv[e]` with `Tervek[e][k].+0x80 == id`; else the story
record `S` with that id makes one: `k = TervNew(e, S.+0x20)`, `strcpy(name, S.+4)`, `+0x80 = id`, `+0x18 =
(S.+0x1c != 0)`, `+0x19 = 1`, `+0x44 = S.+0x24`, for slots `j = 1..=S.+0x28`: `+0x50+j = (u8)S[+0x60 + 4j]`,
`+0x5d+j = (u8)S[+0x2c + 4j]`, then `TervUjraSzamol(e, k, false, true)`; no record: `def`.

### 3.6 An offer
```
design d of empire e: +0x6c = n; +0x70 = 0; +0x74, +0x78, +0x7c = Ev, Honap, Nap
i = first i < N with offer[i].class < 1, else N
if i == N: if N == 40: done (the design keeps the fields); N += 1
offer[i] = Tervek[e][d] (0x84 bytes); name as in §3.4/§3.5 (strcpy / strcat: a long name runs into +0x14..)
offer[i].price = (int)((f32)offer[i].price × factor)
if StoryAdat+0x60 != 0: offer[i].price = (int)((f64)offer[i].price × 0.8)          (st_CheckTradeTervAr)
```
The hull-tech name is the live tech table's (`Talalmanyok[+0x48]`, localised).

### 3.7 Expiry (on a new day)
```
for i in 0..N:
    if 9000 < DatumToIdo(Ev, Honap, Nap, 0) − DatumToIdo(offer[i] date, 0):
        if rnd(0, 500) < IdoSpeedSzorzo: offer[i].class = −1
            if i == TradeAktualisCucc (0xF43444, the screen's selection): the trade screen drops it (UI)
```
Free and already expired slots keep their date, so they keep drawing every day until a new offer takes the slot.

## 4. A purchase (`MULTIGET_Trade`, message 0x51)
The trade screen (`Trade_Main`, icon 0x3a) sends `TradeSzallitasTemp` = {planet = the chosen destination, days =
`AktEtaDays2`, offer `i`, units `n`, price `TradeAr(i, n)`, design = `offer[i]`}, with `TradeAr(i, n) = offer.+0x36
? price·n / 5 : price·n` (i32). The handler, for each queued message from empire `e`:
```
if count[e] < 50
   and (f64)(−price) <= MoneyMennyi(e, 0)            [quirk: always true with money ≥ 0; no affordability check]
   and offer[i].class >= 0 and n <= offer[i].+0x6c and Bolygok[planet].owner == e:
    MoneyHozzaadLog(e, 0, (f64)(−price))
    TradeSzallitas[e][count[e]] = message; count[e] += 1
    offer[i].+0x6c −= n; if offer[i].+0x6c < 1: offer[i].class = −1
    e == EN: the screen's selection is updated (UI)
```
The offer index isn't bounds-checked. The delivery builds the design *as offered at purchase time*.

## 5. Order and RNG
Per tick: the libc draw (new month), the countdowns and arrivals (empires 1..=18, deliveries in order; arrivals
change designs, fleets, vehicles and run the story engine), the removals, the destinations (refunds in pass
order), the merchant draws (stream 11, in the order of §3.4/§3.5), the expiry draws (stream 11, offers in order).
`st_GetTerv` and the arrivals' `TervNew` append design records; `Legyart` appends fleets/ships/vehicles as in
production. Nothing else draws stream 11 in a strategic tick.

## 6. Port (`ig2-sim::trade`)
`Trade` (offers, deliveries, `TradeOldNap`, `OldEtaHonap`, `AktEtaDays`) with `Trade::tick(&mut dyn Market)` and
`Trade::buy(e, &Delivery, &mut dyn Market)`; the market hook reaches the other systems: designs, planets, trade
ports, money, the story design data and the arrival (`Legyart` + `Story_Special`, done by `game.rs`). Events:
`Arrived`, `Refunded`, `Retargeted`, `Offered`, `OfferNamed` (a localised `DesignPoenosanElnevez` name to render; `Market::design_name` via `Game::design_texts`, new-game.md §14),
`OfferWithdrawn` (the screen's selection may need to drop), `Bought`. `TradeAr` is `trade::price`. ADR 0003: the libc draw only feeds the UI; in MP the delivery time is part
of the purchase message, so it is synchronised already.
