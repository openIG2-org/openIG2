"""Spawn the user's ig2.exe under Frida, run tools/trace/ground.js, write a JSONL ground-war trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_ground.py <install-dir> <out.jsonl>
          --mode war|indit [--count N] [--fuzz] [--seconds 560] [--seed 4242]
- war:   --count staged landings + auto-resolved wars (AddTanksToPlanet, WF::Simulate) with snapshots before,
         after the landing and after the war; --fuzz: ground pods, garrisons, building flags, human attacker,
         story bonus.
- indit: --count GroundWarIndit calls (routing and outcome), StartGrWar patched out.
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Spec: re/rules/ground-combat.md.
"""
import argparse

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--mode", required=True, choices=["war", "indit"])
ap.add_argument("--seconds", type=float, default=120)
ap.add_argument("--count", type=int, default=200)
ap.add_argument("--every", type=int, default=4)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
a = ap.parse_args()


def on_event(ev):
    if ev["k"] in ("error", "done"):
        print("agent:", ev)


t = Tracer(a.install, agent_source("ground.js"), a.out, on_event=on_event, banner=f"spawned ig2.exe pid {{pid}}; mode {a.mode}, up to {a.seconds}s",
           config={"mode": a.mode, "scenario": a.scenario, "count": a.count,
                   "fuzz": a.fuzz, "every": a.every, "seed": a.seed})
try:
    t.start()
    t.wait(a.seconds)
finally:
    t.stop()
    print(f"events: {t.counts} -> {a.out}")
