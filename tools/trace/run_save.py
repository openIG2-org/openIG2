"""Spawn the user's ig2.exe under Frida, run tools/trace/save.js: make original save games plus snapshots.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_save.py <install-dir> <out-dir>
          [--days 3,40,150] [--slot 97] [--ships] [--seed 4242] [--seconds 300] [--tag NAME] [--mp] [--refit] [--wormholes]
At each listed day after the scenario starts the agent snapshots the save's source globals and calls the exe's
`LoadSave(slot, name, 2)` (0x543c80); the game writes `save<slot>.sav` to its data directory
(%LOCALAPPDATA%/GyroscopeGames/ig2), which this script moves to <out-dir>/<tag>-<idx>.sav next to
<tag>-<idx>.json (the snapshot); --mp also writes <tag>-<idx>.mpstate, the multiplayer state blob
(`GetSavedGame` 0x543670) of the same moment. Slots 97+ keep clear of the player's own saves (the game's
`safesave.sav` temp file is rewritten, as by every save). --ships adds warships and hands the human's empire to
the AI.
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Run in the foreground: launched from a background shell the game stalled after loading.
Spec: re/formats/save.md.
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
ap.add_argument("out")
ap.add_argument("--days", default="3,40,150")
ap.add_argument("--slot", type=int, default=97)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--seconds", type=float, default=300)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--ships", action="store_true")
ap.add_argument("--tag", default="quick")
ap.add_argument("--mp", action="store_true", help="also take the multiplayer state blob (GetSavedGame 0x543670)")
ap.add_argument("--refit", action="store_true",
                help="perturb buildings and unit cells, save, call EpuleteketJoHelyreTesz (0x53cd00), save again (2k / 2k+1)")
ap.add_argument("--wormholes", action="store_true",
                help="save, call AddWormhole (0x53aec0) with fuzzed coordinates until it returns 0, save again (2k / 2k+1)")
a = ap.parse_args()

data_dir = os.path.join(os.environ["LOCALAPPDATA"], "GyroscopeGames", "ig2")
os.makedirs(a.out, exist_ok=True)
exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "save.js"), encoding="utf-8").read()
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
            src = os.path.join(data_dir, f"save{ev['slot']}.sav")
            base = os.path.join(a.out, f"{a.tag}-{ev['idx']}")
            if not os.path.exists(src):
                print("no save file", src, file=sys.stderr)
                continue
            shutil.move(src, base + ".sav")
            mp = ev.pop("mp", "")
            if mp:
                with open(base + ".mpstate", "wb") as f:
                    f.write(bytes.fromhex(mp))
            with open(base + ".json", "w", encoding="utf-8") as f:
                json.dump(ev, f)
            saved += 1
            print(f"save {ev['idx']} (slot {ev['slot']}, frame {ev['frames']}): {os.path.getsize(base + '.sav')} B -> {base}.sav")
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
script.post({"type": "config", "payload": {"scenario": a.scenario, "days": [int(x) for x in a.days.split(",")],
                                           "slot": a.slot, "ships": a.ships, "mp": a.mp, "seed": a.seed,
                                           "refit": a.refit, "wormholes": a.wormholes}})
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
    print(f"{saved} saves -> {a.out}")
