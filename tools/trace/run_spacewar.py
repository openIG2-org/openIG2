"""Spawn the user's ig2.exe under Frida, run tools/trace/spacewar.js, write a JSONL space-combat trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_spacewar.py <install-dir> <out.jsonl>
          --mode design|auto|indit [--count N] [--ticks N] [--fuzz] [--seconds 120] [--seed 4242]
- design: TervUjraSzamol on every live design slot (up to --count), fuzzed hulls/pods/race with --fuzz.
- auto:   --count SimulateSpBattle calls (auto-resolve), compositions reshuffled with --fuzz.
- indit:  --count SpaceWarIndit calls (routing), then --ticks FRISSIT_Starmap ticks with the frozen
          tactical battles (in-battle marking); --fuzz perturbs debug/story switches, ion guns, units.
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Spec: re/rules/space-combat.md.
"""
import argparse

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--mode", required=True, choices=["design", "auto", "indit"])
ap.add_argument("--seconds", type=float, default=120)
ap.add_argument("--count", type=int, default=200)
ap.add_argument("--ticks", type=int, default=300)
ap.add_argument("--every", type=int, default=6)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
a = ap.parse_args()


def on_event(ev):
    if ev["k"] in ("error", "groundwar"):
        print("agent:", ev)


t = Tracer(a.install, agent_source("spacewar.js"), a.out, on_event=on_event, banner=f"spawned ig2.exe pid {{pid}}; mode {a.mode}, up to {a.seconds}s",
           config={"mode": a.mode, "scenario": a.scenario, "count": a.count,
                   "ticks": a.ticks, "fuzz": a.fuzz, "every": a.every, "seed": a.seed})
try:
    t.start()
    t.wait(a.seconds)
finally:
    t.stop()
    print(f"events: {t.counts} -> {a.out}")
