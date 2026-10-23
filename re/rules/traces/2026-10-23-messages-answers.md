# 2026-10-23: the inbox's writers and the messages screen's orders against the exe

Spec: `re/rules/info-messages.md` §6–§7. Port: `ig2-sim::game::message_orders`, `ig2-sim::messages::{InboxRules,
find_block, block_choices}`; test: `engine/crates/ig2-sim/tests/save_replay.rs` (`inbox_step`, record kind `morder`).

## Recording
`tools/trace/run_replay.py <install> _local/saves/quick-1.sav _local/traces/replay-quick1-answers-x16 --ticks 60
--answers 4 --answers-every 1 --seed 777 --slot-base 300 --pre-slot 298` (`replay.js` `stageAnswers`).
Set-up before save 0: `LoadMessagesFile` (the kinds: 49 conversations, 12 automatic, 150 plain), no dialogue
running (`FolyamatbanU` −1, `ParbeszedVege` 1), five messages (three conversations, one automatic, one plain).
Then four steps a tick through the exe's own functions, 266 in all: 35 `NewMessage` (10 portrait races, 11 text and 5
number parameters), 2 `KiszedUzenet`, 5 `KiszedUtolsoUzenet`, 5 `LetiltReszlet`; 39 rows opened, 33 answers, 28
automatic-message checks, 18 arrow steps, 75 typing ends (transcribed from exe 0x507b43..0x507c1b). `NewMessage`'s
`AktKepKell` 10 for an automatic kind is put back after the call: the first run without it opened the messages screen,
which paused the game after one save.

## Results
All 60 ticks byte-exact: every saved chunk and the dumped dialogue bits (`msg_over`, `msg_typed`, `msg_rows`,
`msg_nchoices`, `msg_unread`, `msg_ch0..3`). The first comparison failed at tick 1 on two extra dumps at
0xD8E2D0/0xD8E32C that held text: they lie in `ParbeszPlace` (0xD8E280, the dialogue text), so `Valasztasok` has four
slots and those two dumps were dropped. The labels' unbounded copy (`mesAtteszSorvegeig` 0x509f20, written after each
choice's part/next/disabled) is ported because the four slots are compared.
Mutation: writing `+5` on the opened entry instead of `Uzenetek[UzenetekSzama]` (§7.2) fails at tick 1.

Not traced: `MessageOrder::Enter` (`Messages_Belep` re-initialises the screen when called), the stories' reactions to
flags 6/8/9 in quick.scn (its scripts don't wait on messages: the hand-offs run `Story_Check` and match, the
campaign's are in the story traces), `MessageText` kinds other than literal text and numbers.
