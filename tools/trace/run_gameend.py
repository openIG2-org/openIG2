"""Stage a game end on the user's ig2.exe under Frida (tools/trace/gameend.js; spec re/rules/game-end.md).

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_gameend.py <install-dir> <out.jsonl>
          [--mode win|lose|quiet] [--seconds 120] [--plant]
--plant writes high-score tables into the options registry for the run: `swarm` with a valid `_crc`,
`lightheart` with a wrong one (the user's options.dat is backed up first and put back afterwards).
The game is killed after the trace's `done` (or --seconds). Nothing in the install is modified.
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
ap.add_argument("--mode", default="win", choices=["win", "lose", "quiet"])
ap.add_argument("--seconds", type=float, default=120)
ap.add_argument("--plant", action="store_true")
a = ap.parse_args()

# The planted tables (scores in Ido units, names) and the check value as `SaveHighscores` computes it.
PLANT = {
    "swarm": ([1500, 2450, 2450, 7000, 0, 0, 0, 0, 0, 0], ["Ann", "Bob", "Cy", "Dee", "", "", "", "", "", ""], True),
    "lightheart": ([100, 0, 0, 0, 0, 0, 0, 0, 0, 0], ["Eve", "", "", "", "", "", "", "", "", ""], False),
}


def crc(scores, names):
    acc, j = [0, 0, 0], 0
    for s, n in zip(scores, names):
        for b in (s & 0xFFFFFFFF).to_bytes(4, "little") + n.encode("latin1"):
            acc[j] ^= b
            j = (j + 1) % 3
    return acc[0] + acc[1] * 0x100 + acc[2] * 0x10000


def read_pairs(b):
    o, pairs = 0, []
    while o + 4 <= len(b):
        n = int.from_bytes(b[o:o + 4], "little"); k = b[o + 4:o + 4 + n]; o += 4 + n
        n = int.from_bytes(b[o:o + 4], "little"); v = b[o + 4:o + 4 + n]; o += 4 + n
        if not k:
            break
        pairs.append((k, v))
    return pairs


def write_pairs(pairs):
    out = b""
    for k, v in sorted(pairs) + [(b"", b"")]:
        out += len(k).to_bytes(4, "little") + k + len(v).to_bytes(4, "little") + v
    return out


opts = os.path.join(os.environ["LOCALAPPDATA"], "GyroscopeGames", "ig2", "options.dat")
backup = opts + ".gameend-backup"
if a.plant:
    shutil.copyfile(opts, backup)
    pairs = dict(read_pairs(open(opts, "rb").read()))
    for key, (scores, names, good) in PLANT.items():
        for i in range(10):
            pairs[f"{key}_point_{i:02}".encode()] = str(scores[i]).encode()
            pairs[f"{key}_name_{i:02}".encode()] = names[i].encode("latin1")
        pairs[f"{key}_crc".encode()] = str(crc(scores, names) + (0 if good else 1)).encode()
    open(opts, "wb").write(write_pairs(list(pairs.items())))

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "gameend.js"), encoding="utf-8").read()
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
            out.write(json.dumps(ev) + "\n")
            if ev["k"] == "done":
                finished = True
    else:
        print("agent:", p)


device = frida.get_local_device()
pid = None
try:
    pid = device.spawn([exe], cwd=a.install)
    session = device.attach(pid)
    script = session.create_script(agent)
    script.on("message", on_message)
    script.load()
    script.post({"type": "config", "payload": {"mode": a.mode}})
    device.resume(pid)
    print(f"spawned ig2.exe pid {pid}; mode {a.mode}")
    t0 = time.time()
    while not finished and time.time() - t0 < a.seconds:
        time.sleep(0.5)
    time.sleep(1)
finally:
    if pid is not None:
        try:
            device.kill(pid)
        except frida.ProcessNotFoundError:
            pass
    out.close()
    if a.plant:
        shutil.move(backup, opts)
    print("events:", counts)
