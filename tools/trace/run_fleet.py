"""Spawn the user's ig2.exe under Frida, run tools/trace/fleet.js, write a JSONL fleet-movement trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_fleet.py <install-dir> <out.jsonl>
          [--seconds 90] [--ticks 1500] [--scenario scenario/quick.scn] [--fuzz] [--fuzz-radar] [--fuzz-approach]
The game window opens; the agent starts the scenario, steps the speed 1 -> 3 -> 2, records
`--ticks` FRISSIT_Starmap calls as before/after snapshots, then the game process is killed.
`--fuzz` gives random move/waypoint/patrol/follow orders and speed overrides every 40 ticks;
`--fuzz-radar` perturbs what the radar pass reads (alliances, owners, radar stations, stealth, story fields).
`--fuzz-approach` runs at x16 and stages unseen enemy attacks on the human on refresh ticks, so the radar
pass's "enemy approaching" SetIdoLelassit(1) slows the same tick's movement (fleet-movement.md §6.3).
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
ap.add_argument("--seconds", type=float, default=90)
ap.add_argument("--ticks", type=int, default=1500)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--fuzz-radar", action="store_true")
ap.add_argument("--fuzz-approach", action="store_true")
ap.add_argument("--mp-radar", action="store_true", help="MultiPlayerMode 1 during half the refresh ticks (MP radar branches)")
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "fleet.js"), encoding="utf-8").read()
out = open(a.out, "w", encoding="utf-8")
ticks = 0


def on_message(msg, _data):
    global ticks
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    if p.get("type") == "batch":
        for ev in p["events"]:
            if ev["k"] == "tick":
                ticks += 1
                if ticks % 250 == 0:
                    b = ev["before"]
                    print(f"tick #{ev['seq']}: ido2 {b['ido2']} x{b['mult']}, {len(b['fleets'])} fleets")
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
plan = [{"at": 0, "speed": 1}, {"at": 20000, "speed": 3}, {"at": 45000, "speed": 2}]
if a.fuzz or a.fuzz_radar or a.fuzz_approach or a.mp_radar:
    plan = [{"at": 0, "speed": 3}, {"at": 90000, "speed": 1}]
script.post({"type": "config", "payload": {"scenario": a.scenario, "startAfterFrames": 1200,
                                           "speedPlan": plan, "ticks": a.ticks, "fuzz": a.fuzz,
                                           "fuzzRadar": a.fuzz_radar, "fuzzApproach": a.fuzz_approach,
                                           "mpRadar": a.mp_radar}})
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
    print(f"{ticks} ticks written to {a.out}")
