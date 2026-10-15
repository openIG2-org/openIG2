# Runtime verification: diplomacy (2026-10-15)

Spec: `re/rules/diplomacy.md`. Port: `engine/crates/ig2-sim/src/diplomacy.rs`. Replay:
`engine/crates/ig2-sim/tests/diplomacy_trace.rs` (reads `_local/traces/diplo-*.jsonl`, never committed).

## Method
`tools/trace/run_diplomacy.py` + `diplomacy.js` spawn the GOG `ig2.exe`, load `scenario/quick.scn` from the
`Fake_FRISSITES` frame hook, run at speed 3 and hook `FRISSIT_Viszony` (exe 0x50df60) and `MI_DIPLOMACY`
(0x4c7060). Each call is recorded as byte-region diffs (relations, power tables, danger, message timers,
contacts and contact ages, empire flags, planets, fortifications, fleets, waypoints, colony/terraformer pools,
the tech table, the human's contact list `AktPlayerek[-1..]`, all 21 RNG seeds) before and after, plus every
libc `rand()` return inside the call (hook on the msvcr120 import at IAT 0x8353AC). The replay rebuilds the
state, runs the port, writes it back into the raw regions and compares byte for byte; the contact list is
compared on (empire, open, waiting); the number of `rand()` draws must match.

`quick.scn` has only two living empires (AI 3 and the human 6), with `ProgramPlayers` 8, so the plain game
never sends a message. `--fuzz` therefore revives empires 1..8 (not 5, not the human) as AIs that never die,
hands them planets, makes everyone know everyone and every planet visible, leaves one planetless AI (8)
running its death countdown, and per call perturbs relation levels and fields (some pairs get "friendly"
values so offers pass), trade flags, contacts, message timers, contact ages, attack orders on other
empires' planets and fleets, the AI's main fleet and its strength, the human's contact-list flags,
difficulty, and switches `DEBUGX` on for two of three `MI_DIPLOMACY` calls (restored after the call).

## Results
| Trace | Calls | Messages | AI answers | Wars | Deaths | `rand()` |
|---|---|---|---|---|---|---|
| `diplo-plain` (1,200 days) | 2,400 | 0 | 0 | 0 | 0 | 0 |
| `diplo-fuzz-2` (seed 202) | 1,200 | 286 | 253 | 10 | 0 | 278 |
| `diplo-fuzz-3` (seed 303) | 1,170 | 217 | 193 | 0 | 0 | 209 |
| `diplo-fuzz-4` (seed 404) | 654 | 60 | 50 | 0 | 1 | 60 |
| **total** | **5,424** | **563** | **496** | **10** | **1** | **547** |

**Every call matches byte for byte**, including every stream-9 draw and every `rand()` count. Messages by
code (cat/sub, answer): negotiate cease-fire 29 (all refused), peace 49 (refused), NAP 5 (refused), break NAP
32 (26 accepted), alliance 1 (refused, the NAP rule), break alliance 42, war together 13 (5 accepted), trade 16
(1 accepted); talk threats 10, improve 5, spies 313, insults 9, scheming 13, greetings 26; 67 went to the
human (answers wait in the UI). Answers rejected by the AI's rules dominate: the AI almost never accepts
another AI's peace offers with the traced relation values.

Findings while recording: none of the spec's formulas needed a change. The 10 wars come from declarations
and "war together" answers; the death is the countdown path (`BirodalomMeghal` changed nothing the
compared regions hold beyond `BirodalomEl`).

## Mutation pass (scratchpad script, 30 mutations of `diplomacy.rs`)
20 caught. Not caught:
- asm-pinned float details: the f32 rounding of `ln(ratio)` (no traced ratio lands on a truncation
  boundary) and the order of the f32 distance sum (few planet pairs per empire);
- not reachable with `ProgramPlayers` 8: the `a·9 + b` cell overlap of `DiplomaciaMikorHivtaUtoljara` (cells
  stay ≤ 80);
- paths the traces don't reach: an accepted cease-fire (the level it sets), the AI judging alliance offers by
  the NAP rule (one offer, refused either way), the 0.7 "weak" ratio, the 7000 "too scared" bound, the `/4`
  main-fleet test of `MI_KitAkarTamadni`, the `<` of the talk reaction roll, and the `< 96` death rule (the
  death came from the countdown).
These stay pinned by the exe decompilation and asm (`diplomacy.md` §3–§8).

## Not traced
MP (message transport, the +1000 bonus), the tutorial, forced answers, the human's own offers and answers,
`ProgramPlayers` 18 (the cell alias with the AI table at 0xDAF668).
