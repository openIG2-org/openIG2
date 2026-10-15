"""Spawn the user's ig2.exe under Frida, run tools/trace/time_rng.js, write a JSONL trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_trace.py <install-dir> <out.jsonl>
          [--seconds 45] [--scenario scenario/quick.scn]
The game window opens; the script starts a scenario itself, steps the game speed 1 → 2 → 3,
then kills the spawned game process. Nothing in the install is modified.
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
ap.add_argument("--seconds", type=float, default=45)
ap.add_argument("--scenario", default="scenario/quick.scn")
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "time_rng.js"), encoding="utf-8").read()
out = open(a.out, "w", encoding="utf-8")
counts = {}


def on_message(msg, _data):
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    if p.get("type") == "batch":
        for ev in p["events"]:
            counts[ev["k"]] = counts.get(ev["k"], 0) + 1
            out.write(json.dumps(ev) + "\n")
    else:
        print("agent:", p)


device = frida.get_local_device()
pid = device.spawn([exe], cwd=a.install)
session = device.attach(pid)
script = session.create_script(agent)
script.on("message", on_message)
script.load()
plan = [{"at": 15000, "speed": 2}, {"at": 25000, "speed": 3}, {"at": 35000, "speed": 1}]
script.post({"type": "config", "payload": {"scenario": a.scenario, "startAfterFrames": 1200, "speedPlan": plan}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; tracing for {a.seconds}s")
try:
    time.sleep(a.seconds)
finally:
    try:
        script.unload()
    except frida.InvalidOperationError:
        pass
    device.kill(pid)
    out.close()
    print("events:", counts)
