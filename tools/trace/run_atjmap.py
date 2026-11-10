"""Spawn the user's ig2.exe under Frida, run tools/trace/atjmap.js, write a JSONL trace of
`PlanetAtjarasMap::GenerateMap` (exe 0x479300) calls: the planet passability maps.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_atjmap.py <install-dir> <out.jsonl>
          [--count N] [--fuzz] [--seconds 540] [--seed 4242]
The first 75 probes cover every colony map unchanged; with --fuzz the rest perturb the live record
(heights, obstacle mask B, the next record's head) for the call. Recorded to _local/traces/atj-*.jsonl.
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Spec: re/rules/ground-combat.md §11.13.
"""
import argparse

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--seconds", type=float, default=540)
ap.add_argument("--count", type=int, default=150)
ap.add_argument("--every", type=int, default=2)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
a = ap.parse_args()


def on_event(ev):
    if ev["k"] == "gen" and t.counts["gen"] % 10 == 0:
        print("gen", t.counts["gen"], "map", ev["m"], ev["ms"], "ms")
    if ev["k"] in ("error", "done"):
        print("agent:", ev)


t = Tracer(a.install, agent_source("atjmap.js"), a.out, on_event=on_event, banner=f"spawned ig2.exe pid {{pid}}; up to {a.seconds}s",
           config={"scenario": a.scenario, "count": a.count, "fuzz": a.fuzz,
                   "every": a.every, "seed": a.seed})
try:
    t.start()
    t.wait(a.seconds)
finally:
    t.stop()
    print(f"events: {t.counts} -> {a.out}")
