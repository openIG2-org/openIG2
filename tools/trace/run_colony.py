"""Spawn the user's ig2.exe under Frida, run tools/trace/colony.js, write a JSONL colony trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_colony.py <install-dir> <out.jsonl>
          [--seconds 70] [--scenario scenario/quick.scn] [--speed 3] [--fuzz]
The game window opens; the agent starts the scenario at the given speed, records every economy
call FRISSIT_Planet(true, 0) as a before/after snapshot, then the game process is killed.
The output holds game state derived from the user's install: keep it under _local/, never commit it.
"""
import argparse

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--seconds", type=float, default=70)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--speed", type=int, default=3)
ap.add_argument("--fuzz", action="store_true", help="randomise live planet state before each economy call")
a = ap.parse_args()


def on_event(ev):
    if ev["k"] == "economy":
        b = ev["before"]
        print(f"economy #{ev['seq']}: {b['ev']}-{b['honap']}-{b['nap']}, "
              f"{len(b['planets'])} planets, {len(ev['rnds'])} rnd/nested events")


t = Tracer(a.install, agent_source("colony.js"), a.out, on_event=on_event, banner=f"spawned ig2.exe pid {{pid}}; tracing for {a.seconds}s",
           config={"scenario": a.scenario, "startAfterFrames": 1200, "speed": a.speed, "fuzz": a.fuzz})
try:
    t.start()
    t.wait(a.seconds, until_done=False, tail=0)
finally:
    t.stop()
    print(f"{t.counts.get('economy', 0)} economy calls written to {a.out}")
