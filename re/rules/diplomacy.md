# Diplomacy: relations and AI diplomacy

Status: **verified at runtime** (`traces/2026-10-15-diplomacy.md`, `traces/2026-10-15-diplomacy-human.md`):
7,060 daily calls and 1,200 staged human-side calls (the human's messages, deals, answers and
counter-proposals, AI messages to the human, the auto-cancel) with 1,470 messages, 310 accepted deals,
64 planet transfers and 1,788 libc `rand()` draws reproduced byte for byte by `engine/crates/ig2-sim`
(`diplomacy.rs`, `tests/diplomacy_trace.rs`). Two daily systems driven by the sim tick
(`Fake_FRISSITES` exe 0x4de720, on a day change, `time-and-rng.md` §1), plus the per-tick bookkeeping (§10.3):

```
FRISSIT_Planet (Nap % 6 == 1) → FRISSIT_Viszony → FRISSIT_MI (unless DAT_00b77244) → FRISSIT_Spy → MI_DIPLOMACY
```

| Function | exe | ELF | Role |
|---|---|---|---|
| `FRISSIT_Viszony` | **0x50df60** | 0x5ab250 | empire power, relation drift, overall opinion, danger state (§3) |
| `MI_DIPLOMACY` | **0x4c7060** | 0x52d110 | the AI picks one message per known empire pair (§4) |
| `ElkuldUzenetMI` | 0x4c0ae0 | 0x51d690 | send a message (race 5 on either side: dropped) |
| `FeldolgozAjanlat` | 0x4c1240 | 0x51d820 | receive it: contact, immediate effects, reaction, the answer (§5) |
| `VegrehajtAzonnalVegrehajtodoDolgokat` | 0x4c86d0 | 0x51ef80 | immediate effects (war, broken pacts) |
| `AjanlatHatastLekezel` | 0x4b7d50 | 0x51f1b0 | the receiver's reaction to talk-about messages (stream 9) |
| `EldontIgenNemUzenet` | 0x4c01e0 | 0x51fcd0 | the AI's yes/no (§6) |
| `ElkuldValaszMI` → `FeldolgozValasz` | 0x4c0c50 → 0x4c1480 | 0x51dd70 → 0x51dee0 | the answer and its effects (§7) |
| `SzeretneTuzSzunetet` / `…BeketKotni` / `…NonAgressionPackot` / `…Szovetseget` / `…TradeAggreementet` | 0x4c8580 / 0x4c8220 / 0x4c8320 / 0x4c8390 / 0x4c84e0 | 0x52c8c0 / 0x52ca60 / 0x52cc00 / 0x52ccf0 / 0x52cfc0 | "wants a cease-fire / peace / NAP / alliance / trade" (§6.1) |
| `NemAkarBeszolni` | 0x4c81d0 | 0x52c810 | "too scared to insult" |
| `MI_HannyalHarcol(e)` / `MI_KitAkarTamadni(e)` | 0x50f040 / 0x50f080 | 0x5b37c0 / 0x5b3850 | wars of `e` / whom `e` wants to attack (§6.2) |
| `DiplomaciaMegtamadas(a, b, declared, partner)` | 0x4bd850 | 0x50e990 | war (§8.1) |
| `BirodalomBeketKot(a, b)` | 0x53b280 | 0x61e2c0 | neutral + stop attack orders (§8.2) |
| `ElkuldHaduzenet(a, b)` | 0x4c09c0 | 0x520db0 | declaration of war message |
| `MiertValaszoltAztAmitValaszolt` | 0x4c7e80 | (ELF same name) | the text "reason" code of an answer (cosmetic, §5) |
| `MegallapitViszonyValtozokat` | 0x4c7c00 | 0x50e260 | `vv_*` flags for the dialogue text (cosmetic) |
| `DiplomaciaErtek(e, kind, value, demand, offer_for, level)` | 0x4bd3a0 | 0x52c130 | what a deal item is worth to the AI (§6.3) |
| `getMiAltalElfoglalhato(p)` | 0x4efe70 | | `owner < 9 && +0x187 == 0` (`colony-side.md`) |
| `SearchBuilding(p, kind, working)` | 0x541150 | | finished buildings of `kind` on `p` (and flag bit 0 when `working`) |
| `DiplomaciaMegtamadasHaduzenettelTogether(a, b, t)` | 0x4bda90 | | `DiplomaciaMegtamadas(a, t, true, b)`, then `(b, t, true, a)` |
| `FRISSIT_Diplomacy` | **0x4c0f90** | 0x52b680 | every tick: rulers' birthdays, auto-cancel of stale messages to the human (§10.3) |
| `MULTIGET_Diplomacy` | 0x4c7880 | 0x520e90 | the human's messages and answers out of the lockstep queue (§10.2) |
| `KeresPlayer(e)` | 0x4c65d0 | | the human's contact-list slot of `e` (last match, −1 if none) |

**The exe is normative.** Where it differs from the ELF: the power ratio's logarithm is rounded to f32 (§3.2),
the planet-distance sum adds `|Δy| + |Δx|` before the accumulator (§3.3), `DiplomaciaErtek` returns the item
kind where the ELF leaves its result uninitialised (§6.3), and the AI decisions are written with different but
equivalent arithmetic (checked case by case below). No rule values here: the tables (§6.3, §8.1) are read from
the user's exe (ADR 0004).

## 1. Data (exe VAs)
| Name | exe VA | Layout |
|---|---|---|
| `Viszony` | 0xDABF38 | `[19][19]` records of 0x26 bytes, `Viszony[a][b]` at `a·0x2d2 + b·0x26`: a's view of b (§2) |
| `BirodalomEro` | 0xBAEA78 | f32 `[19]`: empire power (§3.1) |
| `BirodalomUrEro` | 0xBAEAC8 | f32 `[19]`: fleet attack sum |
| `BirodalomHanyBolygo` | 0xBAEB18 | i32 `[19]`: planets owned |
| `BirodalomVisszaSzamlalas` | 0xBAEB68 | i32 `[19]`: death countdown (100 while owning a planet) |
| `MiVeszelyAllapot` | 0xDB27F8 | i32 `[19]`: AI danger state 1..=5 (§3.5) |
| `DiplomaciaMikorHivtaUtoljara` | 0xDAF520 | i32 `[9][9]` **indexed `a·9 + b` with a, b ≤ 18** (§4) |
| `BirodalomIsmeri` | 0xBAEBB8 | u8 `[19][19]`: `[a][b]` a has met b |
| `BirodalomMiotaIsmeri` | 0xBAED28 | i32 `[19][19]`: contact age (AI) |
| `BirodalomEl` / `BirodalomMIzik` / `BirodalomTamadhat` | 0xBAE6AC / 0x915F58 / 0x915F44 | u8 `[19]`: alive / AI-controlled / may attack |
| `PlayerKi` / `PlayerFaj` | 0xBAE560 / 0xBAE574 | u8 `[19]`: 1 = human / race |
| `MiFoFlotta` | 0xDB0598 | i32 `[19]`: the AI's main fleet |
| `BolygoMennyiErodEro` | 0xB99CB8 | i32 per planet: fortification strength |
| `ProgramPlayers` / `ProgramPlayers2` | 0x912ACC / 0x912AD0 | empire loop bounds (18 after `quick.scn`) |
| `DiplomaciaEngedelyezve` | 0xBAE3AD | u8: diplomacy on (SP) |
| `Nehezseg`, `DEBUGX`, `EN`, `MultiPlayerMode` | 0x92827C, 0xBAE587, 0x912A3C, 0xBAE2C0 | |
| `StoryAdat+0xb0` / `+0xd0 + 4e` | 0xF274A8 / 0xF274C8 | story: no AI proposals / `e` must not attack the human |
| tutorial active / paused | 0xF6915D / 0xF66F2A | every message is accepted, no effects |
| forced answer | 0xB7724C | 0 = decide, 1 = no, else yes (debug) |
| AI off | 0xB77244 | skips `FRISSIT_MI` and `MI_DIPLOMACY` |
| `Megbizhatatlansag_DeclareWar` | 0x902FC0 | i32 `[2][4][2]` (§8.1) |
| tech table | 0xB9C0C8 | status `+0x45 + e` (5 = developed), seen matrix `+0xc0 + 9a + b` (`colony-side.md` §9) |
| planets | 0xB85EA8 | `+0x18/+0x1c` position, `+0x20` owner, `+0x28` population, `+0x5c` defence, `+0x70/+0x74` ship/vehicle capacity, `+0xd8 + 2e` visibility |
| fleets | *0xBB40A4 / count 0xBB40A8 | `+0x15` owner, `+0x60` attack, `+0x68` ground strength, `+0x40..` order (`fleet-movement.md` §1.1) |
| `AktPlayerek` / `MaxPlayerek` | 0xA41E98 / 0xAB64DC | the human's contact list, 0xc4-byte slots (§10.1) |
| `CsaladtagElrabolvaKi` / `…Kialtal` | 0xBB405C / 0xBB4080 | i32 `[9]` each: kidnapped family member state / by whom (spy system) |
| `Money` | 0xBAD168 | f64 `[19]` (`colony-side.md` §6.4) |
| `BolygoNeededMoral` | 0xB9A630 | i32 per planet |
| `FajBolygoViszony` | 0x90B9D8 | i32 `[15][9]`: race × surface affinity (§6.3) |
| `FajSpeckoKutatasMoneySzorzo` / `…IdoSzorzo` | 0x8F87BC / 0x8F87F8 | f32 `[15]` per race (§6.3) |
| `HanyevesAzUralkodo` / `VoltmarUdvozloUzenetEbbenAzEvben` | 0xA41E48 / 0xA41CC4 | i32 / u8 `[19]`: ruler age, greeting sent this year (texts) |
| `DipOldNap` | 0xAB6FFC | the day `FRISSIT_Diplomacy` last saw |
| `DiplomacyStarted` | 0xAB69B5 | u8, set by `NEWGAME_Diplomacy` |

## 2. The relation record (`Viszony[a][b]`, 0x26 bytes)
Eighteen i16 values (index `v[i]` at `+2i`, normally 0..=10000, 5000 = neutral), a trade flag and the level.
Names follow their use; the `vv_*` text flags (`MegallapitViszonyValtozokat`) name some of them.

| i | off | meaning (a's view of b) | daily step (§3.4) |
|---|---|---|---|
| 0 | +0x00 | spy grievance (`vv_SokatKemkedett`: `Viszony[b][a].v0 > 600`) | −5, ≥ 0 |
| 1 | +0x02 | trust | level: war −8, NAP +6, alliance +8; ≤ 5000 at war/cease-fire (level < 2), ≥ 5000 in alliance; then 4 toward 5000; 0..=10000 |
| 2 | +0x04 | story attitude (`st_ViszonyValt` adds 100·n) | 1 toward 5000 |
| 3 | +0x06 | attitude scaled by power | from v6, v8 (§3.4) |
| 4 | +0x08 | messages received today (+1000 each) | set to 0 (the code's "−5, if ≠ 0 then 0") |
| 5 | +0x0a | unreliability (`vv_Megbizhatatlan` > 1000) | −5, ≥ 0 |
| 6 | +0x0c | friendliness | 3 toward 5000 |
| 7 | +0x0e | respect | 4 toward v14 |
| 8 | +0x10 | favour balance (gifts, demands) | 1 toward 5000; **never clamped** |
| 9..13 | +0x12..+0x1a | recent threats, insults, (unused), greetings, scheming reports | −5, ≥ 0 |
| 14 | +0x1c | power relation (`vv_Eroviszony`: < 4200 / < 5801 / else) | recomputed (§3.2) |
| 15 | +0x1e | shared friends | recomputed (§3.3) |
| 16 | +0x20 | distance | recomputed (§3.3) |
| 17 | +0x22 | overall opinion (`vv_RosszViszony` < 3000) | recomputed (§3.4) |
| — | +0x24 | u8 trade agreement | |
| — | +0x25 | u8 level: 1 war, 2 cease-fire, 3 neutral (peace), 4 non-aggression pact, 5 alliance | |

## 3. `FRISSIT_Viszony` (exe 0x50df60), daily
`P2 = ProgramPlayers2`. "Alive" = `BirodalomEl`.

### 3.1 Power
For e in 1..=P2: `power[e] = 1.0`, `planets[e] = 0`, `space[e] = 0.0`. Then for every planet p (1..=HanyBolygo)
with owner o ≠ 0 (f32, adds in this order):
```
power[o] = power[o] + (f32)(pop/2000);  power[o] += (f32)(def5c + fort[p]);  power[o] += (f32)((cap70 + cap74)/200)
planets[o] += 1;  countdown[o] = 100
```
(integer divisions truncate). For every fleet slot 1..=count whose owner byte isn't 0xFF:
`power[o] += (f32)attack; power[o] += (f32)ground; space[o] += (f32)attack` (owner 0 and 10 included).

### 3.2 Death check
For e in 1..=P2 that is alive, owns no planet and is AI-controlled (`BirodalomMIzik`): `countdown[e] −= 1`; if
`countdown[e] < 1`, or `power[e] == 1.0 && countdown[e] < 96`, the empire dies (`BirodalomMeghal` exe
0x53b630: out of scope here, an event; it clears `alive[e]`, which the later loops see).

### 3.3 Pair pass 1 (a ≠ b, both alive, a and b in 1..=P2; `r = Viszony[a][b]`)
```
ratio = power[a] / power[b]                       (f32)
k = 5000 − (i32)((−5000.0 / ln 5.0) · (f64)(f32)ln((f64)ratio))      (the exe rounds ln to f32)
v14 = 10000 − clamp(k, 0, 10000)
v5 −5 (≥0); v0 −5 (≥0); v4 = 0
level 1: v1 −= 8; level 4: v1 += 6; level 5: v1 += 8
level < 2: v1 = min(v1, 5000); level > 4: v1 = max(v1, 5000)
v1 += 4·sign(5000 − v1); clamp v1 to 0..=10000
v2 += sign(5000 − v2);  v6 += 3·sign(5000 − v6);  v7 += 4·sign(v14 − v7);  v8 += sign(5000 − v8)
v9..v13 −= 5, each ≥ 0
f = clamp(ratio, 0.5, 1.5)
v3 = v6 < 5000 ? (i16)(i32)((f32)(v6 − 5000) · f + 5000.0f) : v6
if v8 > 5000: v3 = (i16)(i32)((f64)(v8 − 5000) · 0.3 + (f64)v3)
```
(i16 arithmetic wraps; `sign` is −1/0/1.) Distance (f32): over planets i owned by a and j owned by b,
`n += 1; s = (|y_i − y_j| + |x_i − x_j|) + s`; `v16 = n == 0 ? 0 : (i16)(i32)(((s · 10000) / n) / 600)` (f32).
Shared friends, over alive c ∉ {a, b} in 1..=P2 (i32, starting at 5000):
`acc += (level[a][c] − 3)·250·(level[b][c] − 3) + ((v17[a][c] − 5000)·(v17[b][c] − 5000)) / 7000`
(yesterday's `v17`: pass 2 rewrites it), `v15 = clamp(acc, 0, 10000)`. Constants (exe .rdata): −5000.0 @0x8570D8,
5.0 @0x852390, 0.3 @0x852248, 0.7 @0x852260, 1.4 @0x853070 (f64); 5000/10000/600/1.5/1500/1000 (f32).

### 3.4 Pair pass 2 (same pairs, after pass 1 for all pairs)
```
war = [0, −2000, −1000, 0, 1000, 2000][level]
near = min(v16 − 5000, 0)
v17 = (i16)(i32)( 5000 − v5·0.5 − v0 + (v1−5000)·0.3 + (v2−5000) + (v3−5000)·0.5 + (v14−5000)·0.4
                  + (v15−5000)·0.2 + near·0.5 + war )                    (f64, left to right)
level 1: v17 = min(v17, 3000); level 2: min(v17, 5000); level 4: max(v17, 4000); level 5: max(v17, 7000)
level > 3: every tech t in 1..=192 that b has developed (status 5) is marked seen by a: byte +0xc0 + 9a + b
clamp v0..v7, v9..v17 to 0..=10000 (v8 is left alone)
```

### 3.5 Danger state (`MiVeszelyAllapot`)
For each alive e in 1..=P2: 1; 2 if at war with anyone (level 1, alive c ≠ e); 3 if also
`space[e] < Σ space[c]` over those enemies (f32 sum); 4 if 3 and fewer than 3 planets; 5 if no planets.

## 4. `MI_DIPLOMACY` (exe 0x4c7060), daily
Runs only if AI isn't off and (diplomacy is on or MP). `P = ProgramPlayers`.
1. `last[a·9 + b] += 1` for every a, b in 1..=P. The array is declared `[9][9]`; with P = 18 the cells
   overlap (each cell is hit up to twice a day) and run past the array into the next global (exe 0xDAF664..
   0xDAF7F4, an AI table at 0xDAF668 written by `FRISSIT_MI`). The port keeps the 181 flat cells; the alias
   with the AI table is an open item until the AI is ported.
2. For a in 1..=P not human, b in 1..=P, a ≠ b, both alive, neither is 5, a has met b, and
   (`last[a·9+b] > 300`, or DEBUGX in SP and `> 30`), and **stream 9** `rnd(1,100) == 1` (or, DEBUGX in SP,
   `rnd(1,10) == 1`, a second draw only when the first failed), and (MP, or b isn't the human, or the human
   isn't already answering a message from a: `Diplomacy_WaitingForReply` 0x4c0190), and (MP or diplomacy on):
   pick a message `(cat, sub, third)`, `cat = −1` = none. `r = Viszony[a][b]`, `S = StoryAdat+0xb0 == 0`.
   Every `rnd` is stream 9; `&&` short-circuits as written.

| step | condition | result |
|---|---|---|
| a | S, level < 3, `wants_peace(a,b,−1000)`, rnd(1,100) < 51, rnd(1,2) == 1 | (0, 1) peace |
| b | S, level 1, `wants_ceasefire(a,b,−1000)`, rnd(1,100) < 34, rnd(1,2) == 1 | (0, 0) cease-fire |
| c | S, level 3, `wants_nap(a,b,−1000)`, rnd(1,100) < 34 | rnd(1,2) == 1 ? (0, 3) NAP : (6, 2) improve relations |
| d | S, level 4, `wants_alliance(a,b,−1000)`, rnd(1,100) < 34 | rnd(1,3) == 1 ? (0, 5) alliance : (rnd(1,2) == 1 ? (6, 2) : unchanged) |
| e | S, level > 3, no trade, `wants_trade(a,b,−600)`, rnd(1,100) < 34 | (0, 8) trade |
| f | level 5, rnd(1,100) < 6 | (6, 8) greeting |
| g | rnd(1,100) < 51 and (target(a) == b or (rnd(1,100) < 51 and level 1 and not `wants_ceasefire(a,b,−1000)`)) and `MiotaIsmeri[a][b] < 500` and not `too_scared(a,b)` | rnd(1,100) < 31 and level < 4 → (6, rnd(1,2) == 1 ? 1 threat : 4 insult); then `bad = rnd(1,100) < 51` |
| h | rnd(1,100) < 51 and v0 > 1000 and level > 1 | (6, 3) spies complaint; rnd(1,100) < 51 and level > 2 → `bad` |
| i | S, `bad`, level > 2 | level 5 → (0, 6) break alliance; level 4 → (0, 4) break NAP; rnd(1,100) < 51 and (level 3 or Nehezseg > 2) and target(a) == b and `MiotaIsmeri[a][b] < 0` → `ElkuldHaduzenet(a, b)`, cat = −1 |
| j | S, rnd(1,100) < 26, level > 2, t = target(a) > 0, t ≠ b, `v17[b][t] < 4500`, level[b][t] == 3 | rnd(1,2) == 1 ? (0, 7, t) war together : (6, 6, t) report scheming |
| k | always | one rnd(1,100), unused |

(With S false the exe jumps from a/b/c/d straight to f and from i to k: the same draws as the ELF.)
If `cat ≥ 0`: `last[a·9+b] = 0`, then `ElkuldUzenetMI(a, b, false, cat, sub, third)` (§5).

`ElkuldHaduzenet(a, b)`: race 5 on either side, or diplomacy off in SP → `DiplomaciaMegtamadas(a, b, true, 0)`;
else `ElkuldUzenetMI(a, b, true, 0, 2)` (a declaration, §5).

## 5. Receiving a message (`FeldolgozAjanlat` exe 0x4c1240)
Message codes: `cat` 0 negotiate (sub 0 cease-fire, 1 peace, 2 declare war, 3 NAP, 4 break NAP, 5 alliance,
6 break alliance, 7 war together against `third`, 8 trade agreement), 1 offer-for, 2 demand, 3 offer,
4 special, 6 talk about (0 urge peace, 1 threat, 2 improve relations, 3 spies, 4 insult, 6 report scheming
about `third`, 8 greeting, 9 urge alliance); `diplomac/ajanlat.eng` header (`formats/text.md` §4).
From `a` to `b`:
1. b is the human and doesn't know a yet → discovery (event). `met[a][b] = met[b][a] = 1`;
   `Viszony[b][a].v4 += 1000`.
2. Immediate effects (`VegrehajtAzonnal…`, not in tutorial, cat 0 only): sub 2 → `DiplomaciaMegtamadas(a, b,
   true, 0)`; sub 4 → `Viszony[b][a].v5 += 1000`, `BirodalomBeketKot(b, a)`, both levels 3, both trade flags 0;
   sub 6 → `BirodalomBeketKot(b, a)`, `Viszony[a][b].v5 += 2000`, both levels 3.
3. Reaction (`AjanlatHatastLekezel`; `q = Viszony[b][a]`, `weak = (f64)power[b] < (f64)power[a]·0.7`):
   - cat 0: sub 2 → if `q.v8 < 5000`: `q.v5 += 5000 − q.v8`; if `q.v6 > 5000`: `q.v5 += 1000`; `q.v5 += 2000;
     q.v6 −= 3000`. Sub 4 or 6 → `q.v6 −= 3000; q.v5 += 3000`. Returns −1 (no text case).
   - cat 6, sub 0/2/9: `p = clamp((i32)((((f64)q.v9 − (q.v14 − 5000)/2.0 − 1000)·60)/2000 + 20), 20, 80)`;
     stream 9 `x = rnd(1,100)`; `x > p` → `q.v9 += 1000`, if `q.v9 < 2001` `q.v6 += 1000`, `q.v7 −= 1000`,
     case `weak ? 2 : 1`; else `q.v6 −= q.v9/2`, `q.v9 −= 500`, case 0. Then `q.v10 > 0` → `q.v5 += 500`.
   - cat 6, sub 1/3/4: `q.v10 += 1000`; `q.v7 = max(q.v7, 5000)`; case = `(f64)power[b] ≤ (f64)power[a]·1.4`
     (1 or 0), `weak` → 2. Sub 3: `s = Viszony[a][b].v0`; case −1 if s < 100, +1 if s > 1500, clamp 0..=2;
     `Viszony[a][b].v0 = max(s − 500, 0)`. `m = sub 4 ? 1.5 : 1.0` (f32). Case 1 or 2:
     `p = clamp((i32)((((f64)q.v10 − (q.v14 − 5000)/1.5 − 1000)·60)/2000 + 20), 20, 80)`, stream 9
     `x ≤ p` → `q.v10 −= 1000`, `q.v7 = 5000`, case 3. Then case 0: `q.v6 = (i16)(i32)((f32)q.v6 − m·1500)`;
     1: `q.v6 −= 500, q.v7 += 300`; 2: `q.v7 = (i16)(i32)(m·1000 + (f32)q.v7)`; 3: `q.v6 −= 1000`. Then
     `q.v8 < 4900` → `q.v5 += 2000`; `q.v9 > 0` → `q.v5 += 500`; sub 1 or 4 and `q.v8 > 5000` → `q.v8 = 5000`.
   - cat 6, sub 6 (about t): `d = clamp((v17[b][t] − q.v17)/3, −2000, 2000)`;
     `p = clamp((i32)(((f64)(q.v13 + d)·40)/2000 + 50), 20, 80)`, stream 9 `x > p` → `q.v13 += 1000`,
     `Viszony[b][t].v6 −= 2000`, case 2; else `q.v6 += −2000 − q.v13/2`, `Viszony[b][t].v6 += q.v13 + 500`,
     `Viszony[t][b].v6 += 500`, case 0.
   - cat 6, sub 8: `q.v12 += 1000`; if `q.v12 < 500` `q.v6 += 500` (never true); `q.v7 −= 500`. Case −1.
4. b is the human, a isn't, and (the human's player slot of a has the "open" flag `+0x14`, or the message is a
   declaration): **libc `rand() % 10000`** picks the dialogue variant (`+0x60`), `+0x17` (waiting) = 1, and the
   whole message is copied to `+0x30..+0x64` (§10.1; the copy's last dword overwrites the text variant). It
   waits in the human's diplomacy screen. Nothing else.
5. b isn't the human: `yes = EldontIgenNemUzenet` (§6; forced answer if set). The text case is the reaction's,
   else `MiertValaszolt…(a, b, 3, cat, yes)`; that function draws stream 9 but **restores stream 9's state**
   (exe 0x4c7e80 saves/restores 0xBAF2F4), so it doesn't change the stream. In tutorial, `cat 6` → case 2.
   Then `ElkuldValaszMI(b, a, …, yes)` (§7).

## 6. `EldontIgenNemUzenet` (exe 0x4c01e0): AI accepts?
Tutorial → yes. `bonus = race[a] == 7 ? 1000 : 0`. For AI-to-AI messages (cat 0 / 6):
- cat 0: sub 0 → `wants_ceasefire(b, a, bonus)`; 1 → `wants_peace(b, a, bonus)`; 2, 4, 6 → yes;
  3 **and 5** → `wants_nap(b, a, bonus)` (alliance offers are judged by the NAP rule, which needs level 3, so the
  AI never accepts the AI's alliance offers made at level 4); 7 → b at war with nobody and `target(b) == third`;
  8 → `wants_trade(b, a, bonus)`.
- cat 6: yes.
Deals (cat 1/2/3/4, only the human sends them; exe order):
- cat 4 (special `sub`): `Lehetseges = 1`, `Story_Diplomacy_Special(SpecialLeiro[sub], b)` (exe 0x5c8c60, story
  hook: an event), then **no**.
- cat 2 (demand) with sub 3 (a tech) that b hasn't developed (status ≠ 5) → no. cat 1 with sub2 3 (b's tech)
  that b hasn't developed → no.
- `bonus` as above; `give = Ertek(a, sub, value, demand = cat 2, offer_for = cat 1, level[b][a])` (§6.3) for
  cat 1/2/3; cat 1 also `want = Ertek(b, sub2, value2, true, true, level[b][a])`, and sub 1 with sub2 ≠ 1 → no.
§6.4 decides; the effects follow in §7.

### 6.3 `DiplomaciaErtek(e, kind, value, demand, offer_for, level)` (exe 0x4bd3a0)
Item kinds (`sub`/`sub2` of a deal; `value` says which): 0 money, 1 radar map of empire `value`, 2 planet `value`,
3 tech `value`, 4 release a kidnapped family member (`value` = special 1..=5), 5 declare war on `value`, 6 make
peace with `value`.
- 0: `value`; 0 if `Money[e] < (f64)value`, `offer_for` and not `demand`.
- 1: 8000; 0 if `e == value`.
- 2 (`P` = planet `value`, `n` = `planets[e]`): `x = min(P.+0x44 · 24, 100000)` (last month's tax, i32 multiply);
  `demand`: n < 4 → 10,000,000; n < 5 → ×2; n < 6 → ×2. `x = (i32)((f64)x · (demand ? 1.2 : 0.6))`.
  `r = P.owner ≠ 0 ? P.+0x21 (race) : e` (the empire number used as a race). With `g = GetBolygoGazdagsag(P)`
  (`colony-economy.md` §8) and `f = FajBolygoViszony[r·9 + P.+0x22]`:
  `x = (i32)(((f32)((f64)(f32)((f64)(g − 100) / 100.0) + (f64)(f − 100) / 100.0) / 3.0f + 1.0f) · (f32)x)`.
  Not capturable by the AI (`getMiAltalElfoglalhato`): `demand` → 10,000,000, else 0. `demand` → return.
  Otherwise 0 if `BolygoNeededMoral[P] < 300`, if it has no finished building of kind 1, none of kinds 2/3, or
  none of kinds 4/5 (`SearchBuilding(P, k, false)`); and 0 if `offer_for` and `level == 1`.
- 3 (tech `t`, empire `e`, race `r`): `x = (i32)(((f32)t.+0x34 · money_mult[r] · (f32)t.time[e]) /
  ((f32)t.+0x38 · time_mult[r] + 1.0f))` (all f32; `time[e]` = `+0x58 + 4e`); status 1 → `(i32)((f64)x · 1.3)`;
  status 5 → 0.
- 4: special 1 → 50000, 2 → 25000, 3 → 0, 4/5 → 1000, other → 4 (the kind; the ELF's result is uninitialised).
- 5: `level[e][value]` 1/2 → 30000, 3 → 10000, 4/5 → 0, other → 5.
- 6 and others: the kind itself (6).
Constants: 1.2 @0x852300, 0.6 @0x852250, 100.0 @0x852428, 1.3 @0x852308 (f64), 3.0f @0x848544, 1.0f @0x837b24.

### 6.4 The AI's verdict on a deal (`q = Viszony[b][a]`, `w = wars(b)`, i32 wrapping arithmetic)
- **cat 3 (a gives):** sub 2 and the planet isn't AI-capturable → no; sub 3 and status byte `+0x45 + value` of
  **tech number b** is 5 → no (sic: exe 0x4c0486 and the ELF index the tech table swapped, so the AI accepts techs
  it already has; verified in the traces).
  Else `q.v8 += give/10`, `Viszony[a][b].v8 −= give/10` (i16), **yes**.
- **cat 2 (a demands):** `t = (q.v8 − 5000)·5`; `q.v6 > 5000` → +5000; level > 3 → +5000; level > 4 → +10000,
  and +20000 more for a tech (sub 3); `planets[b] < 6` and money (sub 0) → `t /= 2`;
  `t += (bonus − 5000 + max(q.v7, 5000) − q.v5)·10`. `t = 0` if level < 3, if `q.v8 < 4500`, or if
  (`q.v8 < 5501`, level < 4, `q.v6 < 5001` and `q.v7 < 6000`). Then `Nehezseg == 4` → no; `q.v8 < 4500` → no.
  sub 0/3/4: `give > t` → no; else `q.v8 = q.v8 − give/10 − 500`, `Viszony[a][b].v8 += give/10 + 500`,
  `q.v7 −= 2000`, **yes**. sub 1: `b == value` → no; level > 3 → yes; `level[b][value] > 2` → no; else yes.
  Other subs → no.
- **cat 1 (a gives sub/value for b's sub2/value2):** `s = 5000`; `q.v6 > 5500` → `s = 10000`; `q.v6 < 4500` →
  −10000; level 1 → −30000, 2 → −20000, 4 → +10000, 5 → +20000 (+40000 instead when sub is 3); `q.v8 < 4500` →
  −10000. Money for money (sub 0, sub2 0): `want < give` → **yes**, else `s −= 1,000,000`.
  sub 1: `b == value` → no. sub 5 (a declares war on `value`): no if `level[a][value] == 1`, `q.v5 > 1000`, or
  `Viszony[b][value].v17 ≠ 1` (exactly 1); `give = (Viszony[b][value].v14 − 4000)·10000`, `w > 1` → `+= (w − 1)·20000`,
  `planets[a] > 10` → ×2. sub 6 (peace with `value`): no unless `wants_peace(a, b, 0)`, `q.v14 ≥ 6000`, level 1 and
  `q.v5 ≤ 1000`; `give = (q.v14 − 5000)·10000`, `w > 1` → `+= (w − 1)·20000`, `planets[a] > 10` → **`want` ×2**
  (sic). sub2 1: `b == value2` → no. sub2 5 (b declares war on `value2`): `target(b)` is evaluated (unused); no if
  `q.v5 > 1000`, `w > 0`, `Viszony[b][value2].v14 > 5500`, or its level > 3 or == 1;
  `want = (v14 + 2)·10000` (`planets[a] > 5`: `·20000`). sub2 6: `wants_peace(a, b, 0)` evaluated (unused); no
  unless level 1 and `q.v5 ≤ 1000`; `want = (7000 − q.v14)·20000` (`planets[a] > 5`: `·40000`), sub 2 → `× planets[a]`.
  **yes** iff `want < s + bonus + give`.
(`level`, `planets` = `BirodalomHanyBolygo`; none of this draws a random number.)

### 6.1 The "wants" rules (`r = Viszony[x][y]`, `m = bonus + (MP ? 1000 : 0)`, `w = wars(x)`)
- `wants_peace(x, y, m)`: level ≤ 2, `r.v6 ≥ 4900`, (`r.v6 ≥ 5500` or `r.v14 ≥ 6000` or w > 1), and
  `(15 − w)·500 < r.v14 + (r.v6 − 5000)/2 + (i32)(((f64)(r.v15 − 5000)·0.2 − ((f64)r.v5·0.5 + (f64)r.v0·0.5))·0.5) + m`.
- `wants_ceasefire(x, y, m)`: level 1, `r.v6 ≥ 4900`, (`r.v6 ≥ 5500` or `r.v14 ≥ 5500` or w > 1), and the same
  sum `> (13 − w)·500`.
- `wants_nap(x, y, m)`: level 3, `r.v6 ≥ 4900`, `r.v0 < 1000`, `r.v17 + m > 6000`, `target(x) ≠ y`.
- `wants_alliance(x, y, m)`: s1 = the strongest alive empire ≠ y (first strict maximum of `power` over
  1..=P), s2 = the strongest alive ≠ y, s1; level 4, `(level[y][s1] == 5) + (level[y][s2] == 5) + (x ∈ {s1, s2}) < 3`,
  `r.v6 ≥ 4900`, `r.v0 < 1000`, `r.v1 > 5720`, `r.v17 + m > 7000`, `target(x) ≠ y`.
- `wants_trade(x, y, m)`: level > 3, no trade, `r.v6 ≥ 4900`, `r.v0 < 1000`, (`r.v1 > 5360` or `r.v8 > 5100`),
  `r.v17 + (r.v8 − 5000)/2 + m > 7000`, `target(x) ≠ y`.
- `too_scared(x, y)` (`NemAkarBeszolni`): level < 2 and (`r.v14 ≥ 7000` or (`r.v14 ≥ 6000` and w > 1)).

### 6.2 `wars(e)` and `target(e)`
`wars(e)` = count of c in 1..=P2, c ≠ e, with level[e][c] == 1. `target(e)` (`MI_KitAkarTamadni`): 0 unless
`may_attack[e]` and `wars(e) == 0`. `g = Nehezseg > 2 ? 1.2 : 1.0` (f32). For c in 1..=P, c ≠ e, alive,
`may_attack[c]`, `met[e][c]`, (MP or c not the human or `StoryAdat+0xd0+4e == 0`), (Nehezseg > 1 or c not
human or `wars(c) == 0`):
```
score = v17[e][c] − v5[e][c]/3 + {2: −500, 4: +3000, 5: +6000}[level]  + (c human and wars(c) > 0 ? (3 − Nehezseg)·500 : 0)
reach = some planet p of c with visibility[p][e] > 0 and
        ((i32)((f32)(def5c[p] + fort[p]) · g) ≤ ground(main_fleet[e]) or planets[e] == 0)
```
the c with the smallest `(f32)score` (strict) among those with `reach` and `attack(main_fleet[c])/4 < attack(main_fleet[e])`
(`/4` truncates toward zero) wins. `main_fleet` 0 reads fleet slot 0.

## 7. The answer (`FeldolgozValasz` exe 0x4c1480), from b back to a
The answer record (§10.1) carries the message's `cat, sub, value, sub2, value2` and three flags: `counter`
(byte +0x10, a counter-proposal), `reply` (+0x11, set on the AI's reply to a counter-proposal) and `yes` (+0x12).
One **libc `rand()`** at entry, always (`r`; the dialogue variant, stored only when a is the human).
**A counter-proposal** (`counter` set; only the human's screen sends one, answering an AI's message):
- a is a human: if a is the local human (`EN`), slot(b) `+0x2c` = 1, `+0x0c` = 0, the answer is copied to
  `+0x94` (with `r % 10000` at `+0xbc`); nothing else.
- a is an AI: `yes = EldontIgenNemValasz` = **libc `rand() % 2 ≠ 0`** (exe 0x4c14e4; forced answer: `≠ 1`).
  Only cat 1 goes on: sub 3 (a tech) needs a to have it developed already (status 5), else no. The reason code
  (`MiertValaszolt…(EN, a, 3, 1, yes)`, stream-neutral), then a replies: `FeldolgozValasz` from a to b with
  `reply` and `yes` set, the same items — so b (the counter-proposer) gives `sub/value` and a gives `sub2/value2`
  (§7's cat 1 below, with the roles of that reply).
**Otherwise:** a is `EN` → slot(b) `+0x18` (answer arrived) = 1, `+0x16` = 0, `+0x0c` = 0, the answer is copied
to `+0x64` (`r % 10000` at `+0x8c`). Tutorial or `yes == false` → no effect. Otherwise, by cat (a = proposer,
b = who answered):
- cat 0: sub 0 → `BeketKot(a, b)`, levels 2; 1 → `BeketKot(a, b)` (neutral); 3 → `BeketKot`, levels 4; 5 →
  `BeketKot`, levels 5; 7 → `DiplomaciaMegtamadas(a, third, true, b)` then `(b, third, true, a)`; 8 → both trade
  flags 1; 2, 4, 6 → nothing more.
- cat 1: a gives `sub/value` to b, then b gives `sub2/value2` to a. cat 2: b gives `sub/value` to a. cat 3: a
  gives `sub/value` to b. "x gives item to y":
  - 0 money: `MoneyHozzaadLog(y, 0, (f64)value)` then `(x, 0, −(f64)value)` (`MoneyHozzaadOld`, `colony-side.md`
    §6.4: an AI receiver's income multiplier applies). In cat 1's second half and cat 2 the receiver is paid
    second: `(x, 0, −value)` then `(y, 0, value)`.
  - 1 radar map of empire `value`: for planets 1..`HanyBolygo` − 1 (**the last planet is skipped**) owned by
    `value`, visible to x (`+0xd8 + 2x > 0`) and with `+0xd8 + 2y == 0`: `+0xd8 + 2y = 1`.
  - 2 planet: `BolygoElfoglal(value, y)` (`colony-side.md` §4).
  - 3 tech: `TalalmanyKifejlesztve(y, value)` (`production-research.md` §2.1), seen-matrix byte
    `+0xc0 + 9x + y` of tech `value` = 1, `Research_TalalmanyUjraSzamol` (0x5245c0).
  - 4 family member: `CsaladtagElrabolvaKi[y] = CsaladtagElrabolvaKialtal[y] = 0`.
  - 5 war: `DiplomaciaMegtamadas(x, value, true, 0)`.
  - 6 peace: `BirodalomBeketKot(value, b)` — always b, the answerer (sic: in cat 1's first half and cat 3 that
    makes **b**, not the giver a, neutral with `value`).
  - cat 4 and others: nothing.
Then `Story_Special(0xc, cat, a, b, sub, value, sub2, value2)` (campaign hook: an event).

## 8. Shared effects
### 8.1 `DiplomaciaMegtamadas(a, b, declared, partner)`
Nothing if a or b is 9 or 10, a == b, or they're already at war. `met[a][b] = met[b][a] = 1`.
`row = {5: 0, 4: 1, 3: 2, 2: 3}[level[a][b]] + (declared ? 0 : 4)`. `Viszony[b][a].v6 = min(v6, 2000)`. For k in
1..=18, k ∉ {a, partner}, with `level[b][k]` not 1 or 2 and `met[k][a]`:
`Viszony[k][a].v5 += table[row][k == b ? 0 : 1]` (`Megbizhatatlansag_DeclareWar` exe 0x902FC0);
`level[b][k] == 5` → `Viszony[k][a].v6 −= 5000`, `== 4` → `−= 2000`; after each, `Viszony[k][a].v3 < 0` → 0
(the original clamps v3, not v6). Then both levels 1, both trade flags 0. Message and discovery: events.
### 8.2 `BirodalomBeketKot(a, b)`
Both levels 3. Every fleet slot of a with order 2 (attack) on a planet (kind 1) owned by b, or on a fleet
(kind 2) owned by b, and the same for b against a, gets `FlottaMozgat(f, 0, 0, 0, 0)` (inlined;
`fleet-movement.md` §2). `MiotaIsmeri[a][1..=18]` and `[b][1..=18]` = max(·, 600).

## 9. libc `rand()` versus seeded streams
All decisions and reactions use **stream 9** (`rndlog(9, …)`, source lines 0x1db3..0x1e8f and 0x1ac5/0x1aff/0x1b25
of `diplomac.cpp`); the reason-code draws (0x37d..0x380) restore the stream. libc `rand()` (msvcr120, IAT
0x8353AC) is called by: `FeldolgozValasz` once per answer (exe 0x4c14b3), `FeldolgozAjanlat` for a message
to the human (0x4c134c), the counter-answer coin flip `EldontIgenNemValasz` (0x4c14e4, human only),
`NEWGAME_Diplomacy` (srand(SDL_GetTicks): cats, parrots, anniversaries, weaknesses, ruler ages: cosmetic),
`GetSzoveg` (text variants) and `Diplomacy3DInit` (graphics). **Only the coin flip changes the simulated state**
(the AI's answer to the human's counter-proposal, §7); the others pick texts. All of them advance the shared
libc state that ground combat and trade also use. The port takes every one of these draws from one injected
source (`World::libc`): in the engine a seeded ADR 0003 stream, in the trace replay the recorded values; the
text draws are reported with their value (`DiploEvent::RandDraw`).

## 10. The human's side
### 10.1 Records
Message (`DiplomaciaiUzenetTipus`, 0x34 bytes; `AktDipUzenet` 0xA41E14): `+0x00` declaration (u8), `+0x04` `Ido`,
`+0x08` from, `+0x0c` to, `+0x10` cat, `+0x14` sub, `+0x18` value (the "third" empire of §4), `+0x1c` sub2,
`+0x20` value2, `+0x24` extra, `+0x28` cancelled (u8), `+0x2c` reason code, `+0x30` (unused). The AI's own messages
have sub2 = value2 = extra = −1 (`ElkuldUzenetMI(…, −1, −1, −1, reason)`).
Answer (`DiplomaciaiValaszTipus`, 0x30; `AktDipValasz` 0xA43748): `+0x00` from, `+0x04` to, `+0x08` cat, `+0x0c`
`Ido`, `+0x10` counter, `+0x11` reply, `+0x12` yes (u8s), `+0x14` sub, `+0x18` value, `+0x1c` sub2, `+0x20`
value2, `+0x24` extra, `+0x28` text variant, `+0x2c` reason code.
Contact-list slot (`AktPlayerek[i]`, 0xc4 bytes, `KeresPlayer`: the last slot with `+0x00 == e`, else −1, which
reads the bytes before the array): `+0x00` empire, `+0x04` `PlayerKi[empire]` when met (`FelfedezBirodalom`
ELF 0x50f090: `InitBirodalom(MaxPlayerek)` then these two; a dead empire is met too), `+0x0c` (cleared by answers and auto-cancels: UI),
`+0x14` open (messages of this empire reach the human; set by `InitBirodalom`), `+0x15` the other side
blocks calls (MP), `+0x16` the human's message to this empire awaits its answer (cleared by answers),
`+0x17` **waiting**: a message of this empire awaits the human's answer (`Diplomacy_WaitingForReply` 0x4c0190,
read by §4), `+0x18` an answer arrived, `+0x2c` a counter-answer arrived, `+0x30` the waiting message,
`+0x58` = its cancelled byte, `+0x60` its text variant, `+0x64` the last answer, `+0x94` the last counter-answer.
Slot −1 (exe 0xA41DD4, used for every empire without a slot, e.g. those ≥ 9) lies over other globals: its
`+0x40..+0x74` is `AktDipUzenet` (0xA41E14) and `+0x74..+0xc0` is `HanyevesAzUralkodo` (0xA41E48). So each
message sent overwrites slot −1's stored copy, and an answer to an empire without a slot overwrites ruler
ages (verified in the traces). Only texts read either; the port keeps slot −1 as a record of its own, and its
`+0x00..+0x40` (empire, open, waiting, answered, …) behaves as in the original.

### 10.2 Transport
The human's screen never calls `FeldolgozAjanlat`/`FeldolgozValasz` directly: `ElkuldUzenet` (ELF 0x51d580),
`ElkuldValasz` (0x51dc20), `CancelLastMessage` (0x51d4a0) and `ElkuldEngedelyezes`/`ElkuldLetiltas` put packets
0x3e / 0x3f / 0x40 / 0x3d into the lockstep queue (`Multi_Send`, **also in SP**), and `MULTIGET_Diplomacy`
(exe 0x4c7880, from `MultiSendPackets`) delivers them a frame later: 0x3e → `FeldolgozAjanlat` (§5, from the
human: no race-5 check), 0x3f → `FeldolgozValasz` (§7). The 0x3d, 0x40 and 0x41 handlers only act on another
human's packets (sender ≠ `EN`), so in SP they change nothing. The port models the two deliveries as calls
(`Diplomacy::receive_message`, `Diplomacy::receive_answer`); the queue is the engine's (MP, Phase 3).

### 10.3 `FRISSIT_Diplomacy` (exe 0x4c0f90), every sim tick
Runs in the tick after `FRISSIT_Messages`, before `FRISSIT_Design` and `FRISSIT_Starmap` (Fake_FRISSITES exe
0x4deae0). On a new day (`DipOldNap ≠ Nap`, then `DipOldNap = Nap`) that is 1 January: for e in 1..=18,
`HanyevesAzUralkodo[e] += 1`, `VoltmarUdvozloUzenetEbbenAzEvben[e] = 0` (both only feed texts).
Then, once `DiplomacyStarted`: `MostHanyPlayerVan = MaxPlayerek`, and every slot whose message isn't cancelled
(`+0x58 == 0`) and is waiting (`+0x17`) is **auto-cancelled** when it can no longer be carried out
(`m` = the copy at `+0x30`, `f`/`t` its from/to):
- cat 0, sub 5 (alliance) and `level[f][t] == 5` already;
- cat 1: sub 4 and `CsaladtagElrabolvaKi[t] == 0`; sub 2 and planet `value` isn't owned by f; sub 5 and
  `level[value][f] == 1`; sub2 4 and `Ki[f] == 0`; sub2 2 and planet `value2` isn't owned by t; sub2 5 and
  `level[value2][t] == 1`;
- cat 2: sub 4 and `Ki[f] == 0`; sub 2 and planet `value` isn't owned by t; sub 5 and `level[value][t] == 1`;
- cat 3: sub 4 and `Ki[t] == 0`; sub 2 and planet `value` isn't owned by f; sub 5 and `level[value][f] == 1`.
Cancel: `+0x58 = 1`, `+0x17 = 0`, `Multi_Send(0x41, EN, f)` (a no-op handler in SP). No random draws.

## 11. Not ported (open)
`MiertValaszolt…` reason codes and `vv_*` text flags, `BirodalomMeghal`, `Story_Special` and
`Story_Diplomacy_Special`, `NEWGAME_Diplomacy` (initial ruler ages and the cosmetic tables), the MP transport
(§10.2; the 0x3d/0x40/0x41 handlers), the human's screen itself (which messages it lets the human compose), the
alias of `DiplomaciaMikorHivtaUtoljara` with the AI table, the race diplomacy thresholds (`FajBekeMogalomSzint`
& co. have no reader in the ELF).
