"""Spawn the user's ig2.exe under Frida, run tools/trace/prodres.js, write a JSONL production/research trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_prodres.py <install-dir> <out.jsonl>
          --mode research|production [--count N] [--fuzz] [--seconds 200] [--seed 4242]
- research:   before/after every FRISSIT_Research call (exe 0x51ddd0).
- production: before/after the production block of FRISSIT_Starmap (exe 0x597322..0x597aac).
Records hold byte-region diffs ('init' = full dump, then 'pre'/'post' changes per record).
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Spec: re/rules/production-research.md.
"""
import argparse

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--mode", required=True, choices=["research", "production"])
ap.add_argument("--seconds", type=float, default=200)
ap.add_argument("--count", type=int, default=3000)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
a = ap.parse_args()


def on_event(ev):
    if ev["k"] in ("error",):
        print("agent:", ev)


t = Tracer(a.install, agent_source("prodres.js"), a.out, on_event=on_event, v8=True, banner=f"spawned ig2.exe pid {{pid}}; mode {a.mode}, up to {a.seconds}s",
           config={"mode": a.mode, "scenario": a.scenario, "count": a.count,
                   "fuzz": a.fuzz, "seed": a.seed})
try:
    t.start()
    t.wait(a.seconds)
finally:
    t.stop()
    print(f"events: {t.counts} -> {a.out}")
