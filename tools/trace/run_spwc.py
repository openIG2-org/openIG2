"""Spawn the user's ig2.exe under Frida, run tools/trace/spwc.js, write a JSONL real-time battle trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_spwc.py <install-dir> <out.jsonl>
          [--steps 3000] [--battles 20] [--seed 4242] [--fuzz] [--ai-human] [--planets] [--join] [--seconds 560]
--ai-human: the battle AI plays the human's side too; --planets: half the battles are at a human planet
given defence ships and planet guns; --fuzz: random special pods, weapons, shields and experience;
--join: right after a battle starts another AI fleet joins it (SpaceWar_NewBattle's join path);
--fuzz-designs: random weapons, shields and pods in the participants' designs, big fighter wings, armour and
skipped/switched-off defence buildings.
Every SpaceWarIndit call is wrapped in 'presetup'/'setup' snapshots (the battle setup, spec §7.10), and
leaving the battle screen records the end globals ('left', §7.11).
The agent starts real-time battles (human fleet vs AI fleet) and records every SpwC::SpaceWarFrissit call
(exe 0x56dff0): the full state at a battle's first step ('init'), then byte diffs per step ('step') and
between steps ('between'). Run it in the foreground (a background shell stalled the game).
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Spec: re/rules/space-combat.md §7.
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
ap.add_argument("--seconds", type=float, default=560)
ap.add_argument("--steps", type=int, default=3000)
ap.add_argument("--battles", type=int, default=20)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--ai-human", action="store_true")
ap.add_argument("--planets", action="store_true")
ap.add_argument("--join", action="store_true")
ap.add_argument("--fuzz-designs", action="store_true")
ap.add_argument("--speed", type=int, default=4)
ap.add_argument("--per-battle", type=int, default=1500)
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "spwc.js"), encoding="utf-8").read()
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
            if ev["k"] in ("error", "start", "started", "populated", "left", "multi", "abandoned", "groundwar", "join"):
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
script.post({"type": "config", "payload": {"scenario": a.scenario, "steps": a.steps, "battles": a.battles,
                                           "seed": a.seed, "fuzz": a.fuzz, "aiHuman": a.ai_human,
                                           "planets": a.planets, "join": a.join, "fuzzDesigns": a.fuzz_designs, "speed": a.speed, "perBattle": a.per_battle}})
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
