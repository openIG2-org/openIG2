"""Spawn the user's ig2.exe under Frida, run tools/trace/story.js, write a JSONL trace of the story engine.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_story.py <install-dir> <out.jsonl>
          [--scenario scenario/intruders.scn] [--count N] [--fuzz] [--every 40] [--speed 1|2|3] [--seconds 540] [--seed 4242]
          [--campaign 1|2|3 [--difficulty 1|2|4] [--registry none|V] [--storyline N] [--storymap N] [--inline 402,903]]
One 'tick' record per FRISSIT_Story (exe 0x5c1ff0), one 'check' record per outermost Story_Check (0x5c5610) the
game enters from a Story_* hook, and with --fuzz one 'staged' record per call the tracer makes itself; each holds
the story state and the world inputs of the scenario scripts before and after. --fuzz also queues the scenario
stories 360..375 with random variables, planet and fleet ids every --every frames.
--campaign N starts campaign N instead (seeded with --seed, the unported scripts patched out) and records InitStory
('init'); with --fuzz it stages the inline stories of Story_Check (402..918, or --inline), their specials, counters
and arrivals through a native trampoline, logging the calls the stories make to other systems ('acts').
--registry/--storyline/--storymap override what InitStory reads from the registry and the config. --keep multi leaves
st_Multi_include in and stages the multiplayer modes 1001..1006 instead (counters, ground battles, specials 10/15,
the radar offer, base arrivals, space battles; re/rules/traces/2026-10-16-multi.md). --keep brnd leaves st_BRnd_include
in and stages the random stories 220..239 (starts, counters with the world they test set up, answers, daily checks,
battle starts, specials 0x11/0x19; --inline narrows the stories; re/rules/traces/2026-10-16-brnd.md). --keep begin leaves
st_Begin_include in and stages the opening episodes 1..24 (starts, counters, messages, daily checks, specials, arrivals,
space and ground battles, the radar, spy news; re/rules/traces/2026-10-16-begin.md). --keep rnd leaves st_Rnd_include in,
keeps its unported stories from running (their st_Story calls get an unknown id) and stages the ported ones (starts,
counters, answers, daily checks, a pirate battle's end, specials 0x14..0x19, battle starts; re/rules/traces/2026-10-16-rnd.md). --keep elem
leaves st_Elem_include in, patches its unported blocks out the same way and stages the ported storyline episodes
(part 1: 805..868; starts, counters, messages read/answered, ground battles, radar contacts, arrivals, specials 10/13,
daily checks; re/rules/traces/2026-10-16-elem.md).
--keep fable leaves st_Fable_include in and stages the random fables 301..316 (their timers, answers, merchants'
battles, the escort's arrival, the agent's death; battles are auto-resolved so the clock keeps running;
re/rules/traces/2026-10-16-fable.md).
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Record to _local/traces/pending-story-*.jsonl and rename to story-*.jsonl when complete (the replay globs story-*).
Run in the foreground with --seconds 540 (the tool limit is 600 s).
Spec: re/rules/campaign.md.
"""
import argparse
import time

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--seconds", type=float, default=560)
ap.add_argument("--count", type=int, default=2000)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/intruders.scn")
ap.add_argument("--every", type=int, default=40)
ap.add_argument("--speed", type=int, default=3, help="SetIdoSpeed index: 1 = x1, 2 = x4, 3 = x16")
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--debug", action="store_true")
ap.add_argument("--campaign", type=int, default=0)
ap.add_argument("--difficulty", type=int, default=2)
ap.add_argument("--registry", default=None, help="'none' (the read fails) or the stored storyline value")
ap.add_argument("--storyline", type=int, default=None)
ap.add_argument("--storymap", type=int, default=None)
ap.add_argument("--inline", default="", help="comma-separated inline stories to stage (default: all)")
ap.add_argument("--keep", default="", help="comma-separated scripts not to patch out (begin,rnd,brnd,fable,line,elem,multi)")
a = ap.parse_args()
registry = None if a.registry is None else ("none" if a.registry == "none" else int(a.registry))


# The last record (tick, staged, hook check): a game that left the scenario (or hangs) writes none; after 150 s the
# run stops early.
last_record = None


def on_event(ev):
    global last_record
    if ev["k"] in ("error", "setup", "stall"):
        print("agent:", ev)
    if ev["k"] in ("tick", "staged", "check"):
        last_record = time.time()


t = Tracer(a.install, agent_source("story.js"), a.out, on_event=on_event, v8=True,
           banner=f"spawned ig2.exe pid {{pid}}; fuzz {a.fuzz}, up to {a.seconds}s",
           config={"scenario": a.scenario, "count": a.count, "fuzz": a.fuzz, "seed": a.seed,
                   "every": a.every, "speed": a.speed, "campaign": a.campaign,
                   "difficulty": a.difficulty, "registry": registry, "storyline": a.storyline,
                   "storymap": a.storymap,
                   "inline": [int(x) for x in a.inline.split(",") if x],
                   "keep": [x for x in a.keep.split(",") if x], "debug": a.debug})
probed = False


def each():
    global probed
    if last_record is not None and time.time() - last_record > 150:
        print("no records for 150 s: stopping")
        return True
    if a.debug and not probed and time.time() - t.last_event > 25:
        # The game thread is stuck (possibly inside the tracer's own callbacks): sample every stack from a
        # second script, which has its own lock.
        probed = True
        t.probe_hang()


try:
    t.start()
    t.wait(a.seconds, each=each)
finally:
    t.stop()
    print(f"events: {t.counts} -> {a.out}")
