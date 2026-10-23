"""Spawn the user's ig2.exe under Frida, run tools/trace/newgame.js: the state right after a new campaign starts.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_newgame.py <install-dir> <out-dir>
          [--campaign 1|2|3] [--difficulty 1|2|4] [--seed 4242] [--registry none|N] [--storyline N] [--storymap N]
          [--slot 97] [--tag NAME] [--seconds 300]
       or  ... --scenario scenario/quick.scn  (the scenario menu's start: Betolt_Scenario, InitNewGame mode 1)
       or  ... --skirmish '{"en": 1, "ais": 7, "tech": 0, "planets": 80, "money": 50000, "difficulty": 2, "population": 0}'
           (SetScirmishGameOptions, InitNewGame mode 3; then tools/trace/scenario_seeds.py for the seeds)
Writes <out-dir>/<tag>.sav (the exe's own save at tick 0), <tag>-pre.bin / <tag>-post.bin (the .data section,
image VA 0x8de000.., before and after CampaignGameStart) and <tag>.json (InitNewGame's arguments, the srand seeds,
the config/registry reads). The output holds game state derived from the user's install: keep it under _local/,
never commit it. Run in the foreground. Spec: re/rules/new-game.md.
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
ap.add_argument("--campaign", type=int, default=1)
ap.add_argument("--difficulty", type=int, default=2)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--registry", default="none")
ap.add_argument("--storyline", type=int)
ap.add_argument("--storymap", type=int)
ap.add_argument("--scenario")
ap.add_argument("--mp", help='the multiplayer galaxy (mode 2), JSON: {"seed": S, "planets": 20|40|60|80, "en": 1, '
                '"humans": [1, ...], "alive": [AI empires 1..8], "speed": 4, "races": {"slot": race}}')
ap.add_argument("--skirmish", help='the skirmish (mode 3), JSON: {"en", "ais" 1..7, "randomId", "tech", "planets", '
                '"money", "difficulty" 1..3 (3 = hard, stored as 4), "population" 0..3}')
ap.add_argument("--slot", type=int, default=97)
ap.add_argument("--tag")
ap.add_argument("--seconds", type=float, default=300)
a = ap.parse_args()
mp = json.loads(a.mp) if a.mp else None
sk = json.loads(a.skirmish) if a.skirmish else None
tag = a.tag or (f"skirmish-e{sk['en']}-a{sk['ais']}-t{sk['tech']}-p{sk['planets']}-d{sk['difficulty']}-n{sk['population']}" if sk else f"mp-p{mp['planets']}-s{mp['seed']}" if mp else f"scenario-{os.path.splitext(os.path.basename(a.scenario))[0]}" if a.scenario else f"newgame-c{a.campaign}-d{a.difficulty}-s{a.seed}")

data_dir = os.path.join(os.environ["LOCALAPPDATA"], "GyroscopeGames", "ig2")
os.makedirs(a.out, exist_ok=True)
exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "newgame.js"), encoding="utf-8").read()
finished = False


def on_message(msg, data):
    global finished
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    kind = p.get("type")
    if kind == "dump":
        with open(os.path.join(a.out, f"{tag}-{p['tag']}.bin"), "wb") as f:
            f.write(data)
        print(f"dump {p['tag']}: {len(data)} B")
    elif kind == "save":
        src = os.path.join(data_dir, f"save{p['slot']}.sav")
        if os.path.exists(src):
            shutil.move(src, os.path.join(a.out, tag + ".sav"))
        else:
            print("no save file", src, file=sys.stderr)
        with open(os.path.join(a.out, tag + ".json"), "w", encoding="utf-8") as f:
            json.dump({"campaign": a.campaign, "difficulty": a.difficulty, "seed": a.seed, "registry": a.registry,
                       "storyline": a.storyline, "storymap": a.storymap, "scenario": a.scenario, "mp": mp, "skirmish": sk, **p}, f)
        print("save:", p["log"], "ido", p["ido"])
    elif kind == "done":
        finished = True
    else:
        print("agent:", p)


reg = a.registry if a.registry == "none" else int(a.registry)
device = frida.get_local_device()
pid = device.spawn([exe], cwd=a.install)
session = device.attach(pid)
script = session.create_script(agent)
script.on("message", on_message)
script.load()
script.post({"type": "config", "payload": {"campaign": a.campaign, "difficulty": a.difficulty, "seed": a.seed, "slot": a.slot,
                                           "registry": reg, "storyline": a.storyline, "storymap": a.storymap,
                                           "scenario": a.scenario, "mp": mp, "skirmish": sk}})
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
    print(f"-> {a.out}/{tag}.*")
