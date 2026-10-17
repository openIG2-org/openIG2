"""Spawn the user's ig2.exe under Frida, run tools/trace/spy.js, write a JSONL espionage trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_spy.py <install-dir> <out.jsonl>
          [--count N] [--fuzz] [--seconds 300] [--seed 4242] [--orders N] [--estimate N]
Before/after every FRISSIT_Spy (exe 0x584b30) call the game makes, as byte-region diffs ('init' = full dump,
then 'pre'/'post' changes per record), with the callees reached (coverage). --fuzz revives empires 1..8 and
rewrites spy records before each call (missions due today, catch windows, double agents, ...). --ships adds
warships for the ship-strike mission and hands the human's empire to the AI (otherwise real-time battles stop
the clock on the battle screen).
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Record to _local/traces/pending-spy-*.jsonl and rename to spy-*.jsonl when complete (the replay globs spy-*).
Run in the foreground: launched from a background shell the game stalled after loading.
--estimate N records N direct calls of the spy screen's chance estimate instead (re/screens/spy.md §6.2; write them
to _local/traces/spyest-*.jsonl).
Spec: re/rules/espionage.md.
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
ap.add_argument("--count", type=int, default=600)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--debug", action="store_true")
ap.add_argument("--ships", action="store_true", help="add warships; hands the human's empire to the AI")
ap.add_argument("--orders", type=int, default=0, help="staged MULTIGET_Spy orders after each call (espionage.md §8)")
ap.add_argument("--estimate", type=int, default=0, help="direct KemEselySzamolas1 calls instead (spy.md §6.2)")
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "spy.js"), encoding="utf-8").read()
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
            if ev["k"] in ("error", "setup", "stall"):
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
                                           "debug": a.debug, "ships": a.ships, "aiHuman": a.ships,
                                           "orders": a.orders, "estimate": a.estimate}})
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
