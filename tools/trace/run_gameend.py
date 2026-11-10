"""Stage a game end on the user's ig2.exe under Frida (tools/trace/gameend.js; spec re/rules/game-end.md).

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_gameend.py <install-dir> <out.jsonl>
          [--mode win|lose|quiet] [--seconds 120] [--plant]
--plant writes high-score tables into the options registry for the run: `swarm` with a valid `_crc`,
`lightheart` with a wrong one (the user's options.dat is backed up first and put back afterwards).
The game is killed after the trace's `done` (or --seconds). Nothing in the install is modified.
"""
import argparse
import os
import shutil

from frida_run import Tracer, agent_source

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


t = Tracer(a.install, agent_source("gameend.js"), a.out, banner=f"spawned ig2.exe pid {{pid}}; mode {a.mode}",
           config={"mode": a.mode})
try:
    t.start()
    t.wait(a.seconds, poll=0.5)
finally:
    t.stop()
    if a.plant:
        shutil.move(backup, opts)
    print("events:", t.counts)
