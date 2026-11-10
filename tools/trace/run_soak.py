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

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--ais", type=int, default=3)
ap.add_argument("--mp")
ap.add_argument("--seconds", type=float, default=560)
a = ap.parse_args()


def on_payload(p, _data):
    if p.get("type") == "sample":
        t.out.write(json.dumps(p) + "\n")
        t.out.flush()
        print(p["date"], [(e[0], e[3], e[5]) for e in p["empires"] if e[1]])
    else:
        print("agent:", p)


t = Tracer(a.install, agent_source("soak.js"), a.out, on_payload=on_payload, banner=f"spawned ig2.exe pid {{pid}}; up to {a.seconds}s",
           config={"ais": a.ais, "mp": json.loads(a.mp) if a.mp else None})
try:
    t.start()
    t.wait(a.seconds, until_done=False, tail=0)
finally:
    t.stop()
