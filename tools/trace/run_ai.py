"""Spawn the user's ig2.exe under Frida, run tools/trace/ai.js, write a JSONL trace of the AI empire's daily pass.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_ai.py <install-dir> <out.jsonl>
          [--count N] [--fuzz] [--ships] [--seconds 560] [--seed 4242]
One record per FRISSIT_MI (exe 0x50a6e0) call: byte-region diffs ('init' = full dump, 'pre' = changes before the
call, then a checkpoint diff when entering and leaving FRISSIT_MI_SZAMOLAS and, per AI empire, FRISSIT_MI_GYARTAS,
FRISSIT_MI_KEMKEDES and FRISSIT_MI_FEJLESZTES, and at the end), with the callees reached (coverage). --fuzz
revives empires 1..8 as AIs and perturbs the AI's inputs before each call; --ships adds warships and hands the
human's empire to the AI (otherwise real-time battles stop the clock on the battle screen).
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Record to _local/traces/pending-ai-*.jsonl and rename to ai-*.jsonl when complete (the replay globs ai-*).
Run in the foreground with --seconds 560 (the tool limit is 600 s): from a background shell the game stalled.
Spec: re/rules/ai-empire.md.
"""
import argparse
import time

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--seconds", type=float, default=560)
ap.add_argument("--count", type=int, default=400)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--debug", action="store_true")
ap.add_argument("--ships", action="store_true", help="add warships; hands the human's empire to the AI")
a = ap.parse_args()


def on_event(ev):
    if ev["k"] in ("error", "setup", "stall"):
        print("agent:", ev)


t = Tracer(a.install, agent_source("ai.js"), a.out, on_event=on_event, v8=True,
           banner=f"spawned ig2.exe pid {{pid}}; fuzz {a.fuzz}, up to {a.seconds}s",
           config={"scenario": a.scenario, "count": a.count, "fuzz": a.fuzz, "seed": a.seed,
                   "debug": a.debug, "ships": a.ships, "aiHuman": a.ships})
probed = False


def each():
    global probed
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
