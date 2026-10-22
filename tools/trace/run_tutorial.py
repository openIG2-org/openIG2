"""Record the tutorial interpreter in the GOG ig2.exe (tutorial.js) as JSON lines.

usage: run_tutorial.py <install> <out.jsonl> [--chapters 1,2] [--seconds 560] [--start-after F]
                       [--force-after T] [--battle-after T] [--next-after F] [--debug]

Starts the chapters one after another the way the main menu's tutorial choice does (Mode_Tutorial_Start,
TutorialStartMelyik) and records every TutorialFrissit tick, command and event. An event the script waits for
longer than --force-after UI ticks is fired by the tracer (the player's action); WAITFORBATTLE is cleared
after --battle-after ticks. Run it in the foreground (a background shell stalls the game).
Spec: re/rules/tutorial.md.
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
ap.add_argument("--chapters", default="1")
ap.add_argument("--seconds", type=int, default=560)
ap.add_argument("--start-after", type=int, default=1200)
ap.add_argument("--force-after", type=int, default=300)
ap.add_argument("--battle-after", type=int, default=600)
ap.add_argument("--next-after", type=int, default=400)
ap.add_argument("--debug", action="store_true")
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "tutorial.js"), encoding="utf-8").read()
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
            if ev["k"] in ("start", "battle", "done") or (ev["k"] == "ev" and ev.get("forced")):
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
script.post({"type": "config", "payload": {"startAfterFrames": a.start_after, "forceAfter": a.force_after,
                                           "battleAfter": a.battle_after, "nextAfterFrames": a.next_after,
                                           "chapters": [int(x) for x in a.chapters.split(",") if x],
                                           "debug": a.debug}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; chapters {a.chapters}, up to {a.seconds}s")
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
