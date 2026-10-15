"""Spawn the user's ig2.exe under Frida, run tools/trace/prodres.js, write a JSONL production/research trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_prodres.py <install-dir> <out.jsonl>
          --mode research|production [--count N] [--fuzz] [--seconds 200] [--seed 4242]
- research:   before/after every FRISSIT_Research call (exe 0x51ddd0).
- production: before/after the production block of FRISSIT_Starmap (exe 0x597322..0x597aac).
Records hold byte-region diffs ('init' = full dump, then 'pre'/'post' changes per record).
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Spec: re/rules/production-research.md.
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
ap.add_argument("--mode", required=True, choices=["research", "production"])
ap.add_argument("--seconds", type=float, default=200)
ap.add_argument("--count", type=int, default=3000)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "prodres.js"), encoding="utf-8").read()
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
            if ev["k"] == "error":
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
try:
    script = session.create_script(agent, runtime="v8")
except frida.InvalidArgumentError:
    script = session.create_script(agent)
script.on("message", on_message)
script.load()
script.post({"type": "config", "payload": {"mode": a.mode, "scenario": a.scenario, "count": a.count,
                                           "fuzz": a.fuzz, "seed": a.seed}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; mode {a.mode}, up to {a.seconds}s")
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
