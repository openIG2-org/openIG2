"""Spawn the user's ig2.exe under Frida, load an original save and record a save at every sim tick boundary
(tools/trace/replay.js), for the port's save replay (engine/crates/ig2-sim/tests/save_replay.rs).

Usage (project venv): _local/venv/Scripts/python.exe -u tools/trace/run_replay.py <install-dir> <save> <out-dir>
          [--ticks 40] [--speed 3] [--seed 4242] [--slot 99] [--seconds 400]
The input save is copied to the game's data directory (%LOCALAPPDATA%/GyroscopeGames/ig2) as save<slot>.sav and
loaded with the exe's `LoadSave(slot, name, 1)`; the game then writes save<100+k>.sav per tick, which this script
moves to <out-dir>/<k>.sav, with <out-dir>/replay.jsonl (one line per save: tick, seed, non-saved globals). Files
the game had in those slots are moved aside and put back afterwards.
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Run in the foreground: launched from a background shell the game stalled after loading.
"""
import argparse
import json
import os
import shutil
import sys
import time

import frida

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("save")
ap.add_argument("out")
ap.add_argument("--ticks", type=int, default=40)
ap.add_argument("--speed", type=int, default=3, help="SetIdoSpeed level: 1/2/3 = x1/x4/x16 in single player")
ap.add_argument("--seed", type=int, default=4242, help="libc rand() is reseeded with seed + Ido2 every tick")
ap.add_argument("--slot", type=int, default=99)
ap.add_argument("--seconds", type=float, default=400)
ap.add_argument("--scenario", default="scenario/quick.scn")
a = ap.parse_args()

data_dir = os.path.join(os.environ["LOCALAPPDATA"], "GyroscopeGames", "ig2")
os.makedirs(a.out, exist_ok=True)
aside = os.path.join(a.out, "aside")
os.makedirs(aside, exist_ok=True)


def slot_file(n):
    return os.path.join(data_dir, f"save{n}.sav")


moved = []
for n in [a.slot] + list(range(100, 100 + a.ticks + 2)):
    if os.path.exists(slot_file(n)):
        shutil.move(slot_file(n), os.path.join(aside, f"save{n}.sav"))
        moved.append(n)
shutil.copyfile(a.save, slot_file(a.slot))

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "replay.js"), encoding="utf-8").read()
log = open(os.path.join(a.out, "replay.jsonl"), "w", encoding="utf-8")
finished = False
saved = 0


def on_message(msg, _data):
    global finished, saved
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    if p.get("type") != "batch":
        print("agent:", p)
        return
    for ev in p["events"]:
        if ev["k"] == "save":
            src = slot_file(ev["slot"])
            if not os.path.exists(src):
                print("no save file", src, file=sys.stderr)
                continue
            shutil.move(src, os.path.join(a.out, f"{ev['idx']}.sav"))
            log.write(json.dumps(ev) + "\n")
            log.flush()
            saved += 1
            if saved % 10 == 1:
                print(f"save {ev['idx']} ({ev['where']}): Ido {ev['ido']} Ido2 {ev['ido2']}")
        elif ev["k"] == "done":
            finished = True
        else:
            print("agent:", ev)


device = frida.get_local_device()
pid = device.spawn([exe], cwd=a.install)
session = device.attach(pid)
script = session.create_script(agent)
script.on("message", on_message)
script.load()
script.post({"type": "config", "payload": {"scenario": a.scenario, "slot": a.slot, "speed": a.speed, "ticks": a.ticks,
                                           "seed": a.seed}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; up to {a.seconds}s")
t0 = time.time()
try:
    while time.time() - t0 < a.seconds and not finished:
        time.sleep(1)
    time.sleep(2)
finally:
    try:
        script.unload()
    except frida.InvalidOperationError:
        pass
    device.kill(pid)
    log.close()
    time.sleep(1)
    if os.path.exists(slot_file(a.slot)):
        os.remove(slot_file(a.slot))
    for n in moved:
        shutil.move(os.path.join(aside, f"save{n}.sav"), slot_file(n))
    os.rmdir(aside)
    print(f"{saved} saves -> {a.out}")
