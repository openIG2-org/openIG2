# 2026-10-22: the story scripts' ticker lines and the tutorial's planet rename, against ig2.exe

Specs: `re/rules/info-messages.md` §3.4 (the nine `ScrollUzenet` calls of the scripts), `re/rules/tutorial.md` §4
(event 1's rename in `FRISSIT_Starmap`). Port: `ig2_sim::messages::{Texts::with_story, printf, story_line}`,
`Game::messages_for` (the lines join the tick's postings in event order), `Game::tutorial_sighting`
(`game/tutorial.rs`, the pass's first `StarmapEvent::TutorialPlanetSighted`).

## The ticker lines (the campaign traces)

The 72 campaign recordings (`_local/traces/campaign-*.jsonl`, `run_story.py --campaign N`, the exe in Hungarian)
log every `ScrollUzenet` the scripts make with its text and id. `tests/campaign_trace.rs` now renders each
`StoryEvent::Scroll*` with `story_line` over `s_story.txt` (loaded through `Texts::with_story`, every language of the
install) and compares it with the exe's text: **574 lines, all exact**, names the traces don't dump (techs, planets,
empires) as wildcards; id 1 every time. Covered: 51's `_s(5, 511)` (106 lines), 850's and 860's food sales
`%d … %d` (68), 19's sabotage `sprintf(_s(5, 191), …)` (about 100, several buildings and planets), 814's `""` (225:
key 8140 isn't in table 5), 837's 8371 (20), 230's 2302 (13), 824's 8242 (about 30). Not reached: 308's 3081 (its
capture branch); its important flag 0 is the asm's (0x5e82df), the other flags too (no record logged them).

Two port errors the call sites showed, fixed: story 51 posted key 510 (absent; the text is 511, `push 0x1ff` at
0x5d092b), and 308's line was posted as important.

## The tutorial rename (a staged save replay)

`run_replay.py --tutorial-sighting` (`replay.js` `stageTutorial`) loads `quick-1.sav` and, before save 0, sets
`TutorialPaused` 1, `TutorialEvent` 1, `TutorialActive` 0 and `TutorialActMouseAllapot` −1, stubs `TutorialFrissit`,
and gives the human a fleet with a radar 10 units short of planet 76 (the unseen planet nearest to one of its own;
its first warship design gets radar level 2). `replay-quick1-tutsight-x16` (40 ticks, ×16): on tick 2 the fleet sees
planet 76, whose name becomes "Új bolygó" (`GetString(2, 4, 6)` in Hungarian) and the flags turn to Active 1,
Paused 0, Event 0. `tests/save_replay.rs` replays it **byte-exact, all 40 ticks** (the planet record, and
`tut_active`/`tut_paused`/`tut_event` now among the compared unsaved globals); the staged flags change nothing else
over the run (the ticker stays silent on both sides).

Lessons: quick-1 has no fleet with a radar (fleet radars come from the designs' +0x6a, faded to 0 outside the
owner's supply) and nothing unexplored within reach, so a sighting has to be staged. With a tutorial turned Active
without `TutorialStart`, the game stopped after one tick while `TutorialActMouseAllapot` was 0 (its value in a fresh
game; likely `ReadNext`/`Mouse_Tutorial_Kezel` reading the missing recording); −1, as `TutorialStart` sets, fixed it. A recording without ticker lines takes the language from the remaster's setting
(`locale::exe_language`) in the save replay.
