"""Spawn the user's ig2.exe under Frida, run tools/trace/time_rng.js, write a JSONL trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_trace.py <install-dir> <out.jsonl>
          [--seconds 45] [--scenario scenario/quick.scn]
The game window opens; the script starts a scenario itself, steps the game speed 1 → 2 → 3,
then kills the spawned game process. Nothing in the install is modified.
"""
import argparse

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--seconds", type=float, default=45)
ap.add_argument("--scenario", default="scenario/quick.scn")
a = ap.parse_args()

plan = [{"at": 15000, "speed": 2}, {"at": 25000, "speed": 3}, {"at": 35000, "speed": 1}]
t = Tracer(a.install, agent_source("time_rng.js"), a.out, banner=f"spawned ig2.exe pid {{pid}}; tracing for {a.seconds}s",
           config={"scenario": a.scenario, "startAfterFrames": 1200, "speedPlan": plan})
try:
    t.start()
    t.wait(a.seconds, until_done=False, tail=0)
finally:
    t.stop()
    print("events:", t.counts)
