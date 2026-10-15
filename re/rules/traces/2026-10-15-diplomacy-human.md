# Runtime verification: diplomacy, the human's side and edge cases (2026-10-15)

Spec: `re/rules/diplomacy.md` §6.3–§10. Port: `engine/crates/ig2-sim/src/diplomacy.rs`
(`Diplomacy::receive_message`, `receive_answer`, `tick`, `deal_value`; deals through `Deals` / `ColonyDeals`).
Replay: `engine/crates/ig2-sim/tests/diplomacy_trace.rs` (reads `_local/traces/diplo-*.jsonl`, never committed).
Follows `2026-10-15-diplomacy.md` (the daily AI side).

## Method
`tools/trace/run_diplomacy.py --mode human --fuzz` stages one call every 2 frames from the `Fake_FRISSITES`
frame hook, on the revived-empires fuzz state of the daily traces, by calling the exe's own functions:
- `FeldolgozAjanlat` (0x4c1240): the human's messages to an AI (deals cat 1/2/3, specials cat 4, negotiate, talk;
  half the time the receiver's view of the human is made friendly), AI-to-AI negotiate/talk messages (friendly
  wars so cease-fires and peace pass; empire power set around the 0.7/1.4 talk ratios), AI messages to the human;
- `FeldolgozValasz` (0x4c1480): the human's answers to AI messages, a third of them counter-proposals;
- `FRISSIT_Diplomacy` (0x4c0f90) over a contact list with fuzzed waiting messages (stale planets, hostages,
  wars, alliances) and, a quarter of the time, 1 January.
Deal items use real state (owned planets with buildings, developed techs), kidnapped family members and the
forced-answer switch are fuzzed. Frida's hooks don't fire inside the agent's own `NativeFunction` calls, so libc
`rand()` is pinned: `srand(S)` on the game thread before the call, two `rand()` after it, and the call's draws
are the MSVC LCG outputs from S up to that pair (no record lost its draws). Records hold byte-region diffs as
before, plus money and pools, `CsaladtagElrabolva…`, the research state, per-planet morale/unrest/occupancy/
building slots, ruler ages. The replay runs deals through `ColonyDeals` over the colony-side port
(`BolygoElfoglal`, `MoneyHozzaadOld`, `TalalmanyKifejlesztve`) and compares the planets, buildings, money and
research parsed from the "after" state too.

`--edges` (daily mode) adds boundary fuzz for the paths the first report listed as unreached: war pairs with
power relation 5999..7001 ("too scared"), main fleets around `attack(c)/4 == attack(e)` (negative strengths
too), friendly wars, talk power ratios, and an AI losing its planets with countdown 97 (the `< 96` rule).

## Results
| Trace | Calls | Notes |
|---|---|---|
| earlier daily traces (`diplo-plain`, `diplo-fuzz-2..4`) | 5,424 | still byte-exact; the contact list is now compared in full (stored messages) |
| `diplo-edges-505`, `diplo-edges-606` | 1,636 | edge-fuzzed daily calls; 4 deaths (AI 7 starts planetless at countdown 97) |
| `diplo-human-101`, `-202`, `-303` | 1,200 | 740 messages, 289 answers, 171 `FRISSIT_Diplomacy` passes |
| **total** | **8,260** | 1,470 messages (1,227 AI answers), 310 accepted deals, 64 planet transfers, 140 auto-cancels, 62 wars, 5 deaths, 1,788 `rand()` draws |

**Every call matches byte for byte.** Accepted deals: offers-for 143 (all 7×7 item pairs but two), demands 63,
gifts 104; accepted negotiate messages (AI and human proposals): cease-fires 26, peace 29, alliances 8.

Findings while recording:
- **Original bug, kept:** for a gifted tech (cat 3, item 3) both binaries test the tech table indexed swapped,
  status byte `+0x45 + value` of tech number `b` (exe 0x4c0486), so the AI takes techs it already has and may
  refuse others. The spec had the intended check; 4 records disagreed until the port copied the original.
- Contact-list slot −1 (empires without a slot) lies over `AktDipUzenet` (+0x40) and `HanyevesAzUralkodo`
  (+0x74): every send overwrites its stored message, answers to such empires overwrite ruler ages. Cosmetic,
  not modelled; the replay compares slot −1 only below +0x40.
- A new contact-list slot is all zeros (`Message::default()` is zeros, not the AI's −1 items).
- The human's packets go through the lockstep queue also in SP (`MULTIGET_Diplomacy` 0x4c7880).

## Mutation pass (scratchpad script, 60 mutations of `diplomacy.rs`)
29 caught: the coin flip, the counter-proposal's tech check and cat-1 restriction, peace by the answerer, the
seen-byte order, hostage and war items, the stored letter, the 0.6 offer factor, the planet-building and at-war
rules, hostage/war values, the gift's favour change and swapped tech index, demand halving and its 500, the talk
set, specials refused, all five auto-cancel rules, **the accepted cease-fire level** and **the `< 96` death
rule** (both unreached before), the 0.7/1.4 talk ratios and the talk roll's `<`.
Not caught:
- equivalent: the order of the two money bookings (different treasuries), the recompute after a tech deal
  (`TalalmanyKifejlesztve` already recomputed);
- boundaries the fuzz reaches too rarely: the radar map's skipped last planet, money ≥ value, 8000, the 100,000
  tax cap, 1.2, the f32 rounding of the richness term, the planet's race, morale 300, the tech value's 1.3 and
  +1, capturability of gifts, the demand thresholds (30000, 5501, difficulty 4, radar level), the offer-for
  thresholds (40000, money for money, the final `<`, 5500, `v17 == 1`, 5500, the doubled `want`), the January
  test of the birthdays, the clearing of `+0x16`;
- still unreached as before: alliance offers judged by the NAP rule (the edge fuzz produced no alliance offer
  that only the alliance rule would accept), the 7000 "too scared" bound, the `/4` main-fleet test, the f32
  rounding of `ln`.
These stay pinned by the exe decompilation and asm (`diplomacy.md` §6.3–§10 cites them).

## Not traced
MP (the transport, human-to-human counter-answers, the 0x3d/0x40/0x41 handlers), the tutorial, story specials'
own effects (`Story_Diplomacy_Special`, `Story_Special`: events), `ProgramPlayers` 18.
