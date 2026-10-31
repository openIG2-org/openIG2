"""Spawn the user's ig2.exe under Frida, run tools/trace/soak.js: an AI-only generated galaxy sampled once a game month.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_soak.py <install-dir> <out.jsonl>
          [--ais 3] [--mp '{"seed": S, "planets": 40, "en": 1, "humans": [1, 2], "alive": [3, 6], "speed": 4}']
          [--seconds 560]
Default: the skirmish menu's defaults with --ais computer opponents; --mp starts the multiplayer galaxy instead (as
run_newgame.py --mp). The humans are handed to the AI. One JSON line per month: date, tick, and per empire
[empire, alive, race, planets, fleets, money]. Run in the foreground. Spec: re/rules/ai-empire.md §14.
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
ap.add_argument("--ais", type=int, default=3)
ap.add_argument("--mp")
ap.add_argument("--seconds", type=float, default=560)
a = ap.parse_args()

agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "soak.js"), encoding="utf-8").read()
out = open(a.out, "w", encoding="utf-8")


def on_message(msg, _data):
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    if p.get("type") == "sample":
        out.write(json.dumps(p) + "\n")
        out.flush()
        print(p["date"], [(e[0], e[3], e[5]) for e in p["empires"] if e[1]])
    else:
        print("agent:", p)


device = frida.get_local_device()
pid = device.spawn([os.path.join(a.install, "ig2.exe")], cwd=a.install)
session = device.attach(pid)
script = session.create_script(agent)
script.on("message", on_message)
script.load()
script.post({"type": "config", "payload": {"ais": a.ais, "mp": json.loads(a.mp) if a.mp else None}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; up to {a.seconds}s")
t0 = time.time()
try:
    while time.time() - t0 < a.seconds:
        time.sleep(1)
finally:
    try:
        script.unload()
    except frida.InvalidOperationError:
        pass
    device.kill(pid)
    out.close()
