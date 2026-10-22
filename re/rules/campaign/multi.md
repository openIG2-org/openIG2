# Campaign scripting §8.10 `st_Multi_include` (exe 0x5ee4d0, ELF 0x79af20): the multiplayer game modes

Split out of `../campaign.md` (sections, numbering and references unchanged: `campaign.md §8.10` means this file).
Shared terms, the story engine and the notation are in `../campaign.md` §1..§8.8.

Ported in `engine/crates/ig2-sim/src/multi.rs` ([`MultiStories`]). `Story_Check` runs it in SP and MP, after
the other scripts. Its stories are the MP game modes (the game set-up starts them; the campaign never does):
1001 and 1002 king of the hill, 1003 three planets, 1005 capture the flag; 1004 and 1006 are empty (the exe only
sets `StoryLastChecked`). Each story block catches an abort (§8.7: `st_bSearch(0, mode ≠ 1)` without a free
planet the human sees) with `WIN(0, 1)` and goes on with the next story; a story that wins goes on with the
next story too. `b(x)` = `st_bSearch(x, 0)`, `P[p]` the planet record (slot 0 when a search fails),
`PlayerNev[e]` exe 0xBAE588 + 11·e (the players' names), `trig(id)` = flag 4 with `GrWarBolygo == b(id)`
(`id == 0`: any ground battle), or flag 14 with special type 10 (a colonisation, `fleet-movement.md` §4.3), or
`st_Counter(id)`. Info lines are `st_AddGameInfo(…, owner −1)` (owner = id / 10: they belong to their story),
texts `AddSzoveg` (UI, the human's only).

- **`st_bShow(x, owner)`** (exe 0x603f20, ELF 0x7355d0): planet `p` = `−x` for `x ≤ 0`, else `b(x)`; `owner
  == −1` → `x / 10`. `p < 1`: `StoryError := 1`. Else `StoryError := 0`, `st_bUnhide(−p)` (every empire's
  negative view of `p` becomes 0, exe 0x604040); a non-zero owner `o ≠ EN` the human doesn't know is met
  (`FelfedezBirodalom(o, 1)`, `BirodalomIsmeri[EN][o] := 1`); the human's view `+0xd8[EN] < 1` becomes 1; for
  `x > 0` with an info string `−x` (`InfoString_GetIndex` 0x53ec40: its index in the table at exe 0xBAA1F0,
  12-byte records, count 0xBAD0D0; index 0 counts as none): `BolygoInfoAdd(p, −x, true)` (0x53beb0); `owner != 0`: undo
  `(2, p << 16 | x & 0xffff, owner)`.
- **`st_GetBirodalomSulypont(&x, &y, e)`** (exe 0x5ea570): the f32 mean of `+0x18`/`+0x1c` over the planets
  **0**..=`HanyBolygo` owned by `e` that the human sees (`+0xd8[EN] ≥ 0`), summed in order; none → (0, 0).
  **`st_bGetNearest(x, y, owner)`** (exe 0x603980): over the planets 1..=n the human sees with owner `owner`
  (−1: any live owner) the first, or one with `(f32)(dy² + dx²) < (f32) best` (`best` = the last pick's
  truncated value); 0 when there is none.

| Story | What it does |
|---|---|
| 1001 | start: planet 10011 (`st_bSearch(10011, 1)`, else `st_bSearch(0, 0)` gets the id), info 10011 (the planet), `st_bShow(10011, −1)`, `v(10011) := 0`. `trig(10011)`: if `P[b(10011)].owner != v(10011) = o`: remove info `o == 0 ? 10011 : o == EN ? 10012 : 10013`, `v(10011) := owner`, `v(10012) := v(10010)` (0 → 150), counter 10011 := 1 day, info `owner == EN ? 10012 : 10013` (planet, `v(10012)`). Counter 10011 (after the block above, in the same check): `o = v(10011)`, `n = v(10012) − 1`, `v(10012) := n`; `n ≥ 1`: info `o == EN ? 10012 : 10013` replaced (planet, `n`), counter 10011 := 1 day; else text (`o == EN`: "AMAZING! YOU ARE THE WINNER!", else "`PlayerNev[o]` is the winner!"), `WIN` |
| 1002 | as 1001 with planet/variable/counter 10021 and infos 10021/10022/10023, but one count per owner: the start also sets `v(10041 + e) := v(10020)` (0 → 600) for `e` = 1..=8, an owner change doesn't reset it, and the count shown and decremented is `v(10041 + o)` (`10041 + 0` for an owner-less planet: a missing variable reads 0, so that tick wins) |
| 1003 | start: unless planet 10031 exists, 51 tries of three `st_bSearch(0, 2)` (**stream 2**; it aborts when there's no free planet the human sees); for three different planets the sum of the truncated distances `(i32)(f32) sqrt((f64)(f32)(dx² + dy²))` (a–b, b–c, c–a) replaces the best when strictly larger (the best starts as planets 1, 1, 1 with sum 0); the best get ids 10031, 10032, 10033. `st_bShow` all three (−1), info 10031 (no text), counters 10031 := 10, 10034 := 50 days. Counter 10034: message 0x2347 (`NewMessage(0x2347, 300)`, parameter 1 `PlayerNev[EN]`; the ELF also disables answer 2 below 20,000 money, the exe dropped that). Answer 2 (flag 6, block 0x2347, part 2): `Multi_Send(0x66, EN, 4, &10031)` (it comes back as `Story_Special(15, sender, 10031)` through `MULTIGET_Story`); part 4: counter 10034 := 50 days; promotion (flag 7, block 0x2347): counter 10034 := 100 days. Special 15 with `SpecialData2 == 10031`: `FUN_0053fb10(d1 > 0 ? d1 : EN, −1, −20000.0)`; `d1 == EN`: counter 10033 := 1 day. Counter 10033 (the bought radar, re-armed daily): every live fleet of an owner `≠ EN`, `≤ 8` loses `+0x52` bit 8 (always visible); for `k` = 0..=2 and planet `p = b(10031 + k)` owned by the human: over those fleets the nearest `d = (i32)(f32) sqrt((f64)(f32)(dx² + dy²))` (planet − fleet; below 1000, the first of equals; its owner is kept across `k`), fleets with `d < 80` get bit 8; info `10032 + k` removed and, for `d < 50` / `< 80` / `< 110`, added: `PlayerNev[owner]`, "RED" / "YELLOW" / "GREEN", the planet, colour 6 / 3 / 7. `trig(0)`: counter 10031 := 10 days; the three planets' owners equal and non-zero → text (the human: "YOU ARE THE WINNER!", else "… is the winner!"), `WIN` |
| 1005 | start: unless planet 10051 exists: the nearest free planet (`st_bGetNearest(trunc x, trunc y, 0)`) to empire 1's centre (`st_GetBirodalomSulypont(1)`) gets id 10051 (when non-zero) and the name "Base 1" (written into planet slot 0 when there's none), the nearest to empire 2's centre id 10052 and "Base 2" (the same planet → `st_bSearch(0, 0)` instead). Unless `v(10051) != 0`: the "Flag Carrier" design (below) for empire 1 (`TervNew(1, 0)`, the record copied over it, `TervUjraSzamol(1, d, 0, 1)`) → `v(10051)`, the same record for empire 2 → `v(10052)`. `st_bShow` 10051, 10052 (−1), `v(10055) := v(10056) := 0`, `v(10059)` 0 → 250, `v(10060)` 0 → 50, info 10051 (""). Special 10 on planet `d2 ∈ {b(10051), b(10052)}`: `d1 == EN` → "You cannot colonize `<planet>`!"; the planet goes back to owner 0 (`st_SetBolygoTulaj(d2, 0, −1)`: free → `FUN_0059af50(p, 0, PlayerFaj[0], 1, 0)`, else `BolygoElfoglal(p, 0)` and `+0x21 := PlayerFaj[0]` when non-zero). A fleet arrives at a planet (flag 0, type 1): `which` = 1 / 2 for `b(10051)` / `b(10052)`, `o` = the fleet's owner. **Take**: `v(10054) == 0`, `which == 1`, `o == 2` → `x = 2`, else `v(10053) == 0`, `which == 2`, `o == 1` → `x = 1`: texts (`EN == x`: "You have the flag of `PlayerNev[which]`!", `EN == which`: "`PlayerNev[x]` has your flag!"), `v(10052 + x) := 1`, `HajoNew(fleet, v(10050 + x))`. Otherwise, with `which != 0`, **score** when the fleet holds a ship (the last of `Hajok` 1..=n in that fleet) of design `[0, v(10051), v(10052)][o]` (owner > 2: none) and (`v(10054) == 0`, `which == 1`, `o == 1`) or (`v(10053) == 0`, `which == 2`, `o == 2`), `x = which`: texts (`EN == x`: "TOUCHDOWN!!!", `EN == 3 − x`: "`PlayerNev[x]` has captured your flag!"), `v(10052 + x) := 0`, `HajoDelete(ship)`, `v(10054 + x) := v(10056 + x)·v(10060) + v(10054 + x) + v(10059)` (read in that order), `v(10056 + x) += 1`. A space battle ends (flag 3) with empire 1 or 2 among the participants (`SpwFlottak[0]`, `[1]`, and `[2]` when there are more than two; owner `+0x18`): `a` / `b` = 1 or 2 when participant 0 or 1 is empire 1 / 2; carrier 1 / 2 = a ship of design `v(10051)` / `v(10052)` in a fleet of empire 1 / 2; `a != 0`: `v(10055) += CTF_value[3 − a]`, `b != 0`: `v(10056) += CTF_value[3 − b]` (`CTF_value` exe 0xF261EC: per side the attack lost in the last auto-resolved battle, `space-combat.md` §5); `v(10053) != 0` without carrier 1: `v(10053) := 0`, texts (`EN == 1`: "You have lost the flag!", `EN == 2`: "Your flag has been returned!"); the same for `v(10054)` and carrier 2 with the empires swapped. Any of these parts: info 10051 replaced by the score `"%s: %4d/%d %c   %s: %4d/%d %c"` (`PlayerNev[1]`, `v(10055)`, `v(10057)`, `v(10053) ? 'F' : ' '`, `PlayerNev[2]`, `v(10056)`, `v(10058)`, `v(10054) ? 'F' : ' '`; the exe reads 10054, 10058, 10056, 10053, 10057, 10055, so `StoryError` is 10055's) |

The "Flag Carrier" design record (0x84 bytes, the `.scn` `TERV` layout): name "Flag Carrier" (+0x00..+0x14
zero-filled), `+0x16` i16 1, `+0x18` 0, `+0x19` 1, `+0x1a` 1, `+0x1c` 0, `+0x20` 250,000, `+0x24` 3,500, `+0x28`
0, `+0x2c` 0, `+0x30` 0, `+0x34` 0 (4 bytes), `+0x38` 0 (2 bytes), `+0x3c` f32 1.0, `+0x40` i16 `EN` (the race
field gets the player number), `+0x44` 13, `+0x48` 13, `+0x4c..+0x83` 0. The exe leaves `+0x15`, `+0x1b`,
`+0x29..+0x2b`, `+0x3a`, `+0x3b`, `+0x42`, `+0x43` as its stack held them (the port writes 0). `TervNew(e, 0)`
(exe 0x5422f0) takes the first slot 1..=n with `+0x16 < 0`, else appends one.

Exe vs ELF: the exe has no `st_oldLetilt(0x2347)` money check in 1003; it inlines the variable, planet-id and
counter accessors (the same results; the reads that set `StoryError` follow the exe's order above) and
`st_RemoveGameInfo`; its distances are `sqrt` in f64 of an f32 sum, truncated through f32. Quirks kept: 1002
counts `v(10041)` down for an owner-less hill; 1003's best starts as planets (1, 1, 1) (three ids on planet 1
when no try finds three different planets); 1005 reads `SpwFlottak[1]` even after a one-sided battle (the port
reads owner 0 past the list) and the arriving fleet's design list past index 2 for an owner > 2 (stack bytes;
the port finds no carrier).
