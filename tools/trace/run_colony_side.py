"""Spawn the user's ig2.exe under Frida, run tools/trace/colony_side.js, write a JSONL trace of the
colony side systems (re/rules/colony-side.md).

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_colony_side.py <install-dir> <out.jsonl>
          [--mode economy|elfoglal|fellazad|colonise|money|arrive] [--fuzz] [--seconds 120] [--count 100]
          [--every 5] [--seed 12345] [--speed 3]
economy: before/after snapshots of every FRISSIT_Planet(true, 0) (with --fuzz: rebellion/autobuild/money
states pushed into the live game first). The other modes call the exe's own functions from the frame hook
every --every frames, --count times. The output holds game state from the user's install: keep it under
_local/, never commit it.
"""
import argparse
import sys

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--mode", default="economy", choices=["economy", "elfoglal", "fellazad", "colonise", "money", "arrive"])
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--seconds", type=float, default=120)
ap.add_argument("--count", type=int, default=100)
ap.add_argument("--every", type=int, default=5)
ap.add_argument("--seed", type=int, default=12345)
ap.add_argument("--speed", type=int, default=3)
ap.add_argument("--scenario", default="scenario/quick.scn")
a = ap.parse_args()

records = 0


def on_event(ev):
    global records
    if ev["k"] not in ("loaded", "static"):
        records += 1
        if ev["k"] == "error":
            print("agent exception:", ev["msg"], file=sys.stderr)
        elif records % 10 == 1:
            print(f"{ev['k']} #{ev.get('seq')}")


t = Tracer(a.install, agent_source("colony_side.js"), a.out, on_event=on_event,
           banner=f"spawned ig2.exe pid {{pid}}; mode {a.mode}; tracing for {a.seconds}s",
           config={"scenario": a.scenario, "startAfterFrames": 1200, "speed": a.speed,
                   "mode": a.mode, "fuzz": a.fuzz, "count": a.count, "every": a.every,
                   "seed": a.seed})
try:
    t.start()
    t.wait(a.seconds, until_done=False, tail=0)
finally:
    t.stop()
    print(f"{records} records written to {a.out}")
