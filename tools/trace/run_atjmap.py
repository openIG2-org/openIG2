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
import json
import os
import sys
import time

import frida

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

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "atjmap.js"), encoding="utf-8").read()
out = open(a.out, "w", encoding="utf-8")
counts = {}
finished = False


def on_message(msg, _data):
    global finished
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    if p.get("type") == "batch":
        for ev in p["events"]:
            counts[ev["k"]] = counts.get(ev["k"], 0) + 1
            if ev["k"] == "gen" and counts["gen"] % 10 == 0:
                print("gen", counts["gen"], "map", ev["m"], ev["ms"], "ms")
            if ev["k"] in ("error", "done"):
                print("agent:", ev)
            if ev["k"] == "done":
                finished = True
            out.write(json.dumps(ev) + "\n")
        out.flush()
    else:
        print("agent:", p)


device = frida.get_local_device()
pid = device.spawn([exe], cwd=a.install)
session = device.attach(pid)
script = session.create_script(agent)
script.on("message", on_message)
script.load()
script.post({"type": "config", "payload": {"scenario": a.scenario, "count": a.count, "fuzz": a.fuzz,
                                           "every": a.every, "seed": a.seed}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; up to {a.seconds}s")
t0 = time.time()
try:
    while time.time() - t0 < a.seconds and not finished:
        time.sleep(1)
    time.sleep(1)
finally:
    try:
        script.unload()
    except frida.InvalidOperationError:
        pass
    device.kill(pid)
    out.close()
    print(f"events: {counts} -> {a.out}")
