"""Spawn the user's ig2.exe under Frida, run tools/trace/ground_rt.js, write a JSONL real-time ground-war trace.

Usage (project venv): _local/venv/Scripts/python.exe -u tools/trace/run_ground_rt.py <install-dir> <out.jsonl>
          --mode place|turret|battle [--count N] [--fuzz] [--seconds 540] [--seed 4242]
- place:  staged real-time landings (AddTanksToPlanet(f, p, 0): LocateTankSuitablePosition, the landing zone,
          splits) and sometimes StartGrWar; --fuzz: ground pods, more vehicles, human-controlled attacker.
- turret: EpuletNewSearchLocation with defence buildings (AddBuildingTurrets, AddTurret, GetHeight, BuildingBuilt).
- battle: real-time battles on the colony screen, snapshots around the game's own WF::MI / WF::Refresh.
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Spec: re/rules/ground-combat.md.
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
ap.add_argument("--mode", required=True, choices=["place", "turret", "battle"])
ap.add_argument("--seconds", type=float, default=120)
ap.add_argument("--count", type=int, default=200)
ap.add_argument("--every", type=int, default=4)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--ticks", type=int, default=1500)
ap.add_argument("--steps", type=int, default=100000)
ap.add_argument("--speed", type=int, default=0)
ap.add_argument("--end-after", type=int, default=0, help="battle: remove the attacker after N ticks (defender wins)")
ap.add_argument("--human", action="store_true", help="battle: the human attacks (the end countdown runs)")
ap.add_argument("--destruct-after", type=int, default=0, help="battle: set the self-destruct timer after N ticks")
ap.add_argument("--ai-fuzz", action="store_true", help="battle: perturb the ground AI's plans, timers, hits and air strikes before WF::MI")
ap.add_argument("--ai-fuzz-every", type=int, default=25, help="battle: perturb before about one WF::MI call in N")
ap.add_argument("--small", action="store_true", help="battle: small forces, so battles end (countdown, GroundWarFinished)")
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "ground_rt.js"), encoding="utf-8").read()
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
script.post({"type": "config", "payload": {"mode": a.mode, "scenario": a.scenario, "count": a.count,
                                           "fuzz": a.fuzz, "every": a.every, "seed": a.seed,
                                           "ticks": a.ticks, "steps": a.steps, "speed": a.speed,
                                           "small": a.small, "endAfter": a.end_after,
                                           "human": a.human, "destructAfter": a.destruct_after,
                                           "aiFuzz": a.ai_fuzz, "aiFuzzEvery": a.ai_fuzz_every}})
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
