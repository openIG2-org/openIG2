"""Spawn the user's ig2.exe under Frida, run tools/trace/colony.js, write a JSONL colony trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_colony.py <install-dir> <out.jsonl>
          [--seconds 70] [--scenario scenario/quick.scn] [--speed 3] [--fuzz]
The game window opens; the agent starts the scenario at the given speed, records every economy
call FRISSIT_Planet(true, 0) as a before/after snapshot, then the game process is killed.
The output holds game state derived from the user's install: keep it under _local/, never commit it.
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
ap.add_argument("--seconds", type=float, default=70)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--speed", type=int, default=3)
ap.add_argument("--fuzz", action="store_true", help="randomise live planet state before each economy call")
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "colony.js"), encoding="utf-8").read()
out = open(a.out, "w", encoding="utf-8")
calls = 0


def on_message(msg, _data):
    global calls
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    if p.get("type") == "batch":
        for ev in p["events"]:
            if ev["k"] == "economy":
                calls += 1
                b = ev["before"]
                print(f"economy #{ev['seq']}: {b['ev']}-{b['honap']}-{b['nap']}, "
                      f"{len(b['planets'])} planets, {len(ev['rnds'])} rnd/nested events")
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
script.post({"type": "config", "payload": {"scenario": a.scenario, "startAfterFrames": 1200, "speed": a.speed,
                                         "fuzz": a.fuzz}})
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
    print(f"{calls} economy calls written to {a.out}")
