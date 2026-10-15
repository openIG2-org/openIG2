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
import json
import os
import sys
import time

import frida

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

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "colony_side.js"), encoding="utf-8").read()
out = open(a.out, "w", encoding="utf-8")
records = 0


def on_message(msg, _data):
    global records
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    if p.get("type") == "batch":
        for ev in p["events"]:
            if ev["k"] not in ("loaded", "static"):
                records += 1
                if ev["k"] == "error":
                    print("agent exception:", ev["msg"], file=sys.stderr)
                elif records % 10 == 1:
                    print(f"{ev['k']} #{ev.get('seq')}")
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
                                         "mode": a.mode, "fuzz": a.fuzz, "count": a.count, "every": a.every,
                                         "seed": a.seed}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; mode {a.mode}; tracing for {a.seconds}s")
try:
    time.sleep(a.seconds)
finally:
    try:
        script.unload()
    except frida.InvalidOperationError:
        pass
    try:
        device.kill(pid)
    except frida.ProcessNotFoundError:
        print("game process already gone (crashed?)", file=sys.stderr)
    out.close()
    print(f"{records} records written to {a.out}")
