"""Spawn the user's ig2.exe under Frida, run tools/trace/diplomacy.js, write a JSONL diplomacy trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_diplomacy.py <install-dir> <out.jsonl>
          [--count N] [--fuzz] [--edges] [--mode daily|human] [--every 3] [--seconds 300] [--seed 4242]
--mode daily (default): before/after every FRISSIT_Viszony (exe 0x50df60) and MI_DIPLOMACY (0x4c7060) call,
with the libc rand() draws made inside; --edges adds boundary fuzz (too-scared bounds, main-fleet /4, friendly
wars, talk power ratios, the countdown-97 death).
--mode human (use with --fuzz): every --every frames, a staged call of the exe's FeldolgozAjanlat (0x4c1240),
FeldolgozValasz (0x4c1480) or FRISSIT_Diplomacy (0x4c0f90): the human's messages and deals, AI messages,
the human's answers and counter-proposals, the auto-cancel; libc rand() pinned by srand (diplomacy.js).
Records hold byte-region diffs ('init' = full dump, then 'pre'/'post' changes per record).
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Record to _local/traces/pending-diplo-*.jsonl and rename to diplo-*.jsonl when complete (the replay globs diplo-*).
Spec: re/rules/diplomacy.md.
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
ap.add_argument("--seconds", type=float, default=300)
ap.add_argument("--count", type=int, default=2000)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--debug", action="store_true")
ap.add_argument("--mode", choices=["daily", "human"], default="daily")
ap.add_argument("--edges", action="store_true")
ap.add_argument("--every", type=int, default=3)
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "diplomacy.js"), encoding="utf-8").read()
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
            if ev["k"] == "error":
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
try:
    script = session.create_script(agent, runtime="v8")
except frida.InvalidArgumentError:
    script = session.create_script(agent)
script.on("message", on_message)
script.load()
script.post({"type": "config", "payload": {"scenario": a.scenario, "count": a.count, "fuzz": a.fuzz, "seed": a.seed,
                                           "mode": a.mode, "edges": a.edges, "every": a.every,
                                           "debug": a.debug}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; fuzz {a.fuzz}, up to {a.seconds}s")
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
