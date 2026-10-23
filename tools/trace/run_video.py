"""Frame-exact captures of ig2.exe's title video (re/screens/video.md) for the client's comparison.

    run_video.py <install> <out-prefix> --grabs 0,40,2000 [--input key:down@T,key:up@T,mouse:down@T,...]
                 [--size WxH] [--seconds 120] [--open NAME --at X,Y [--once] [--after N]]

Spawns ig2.exe under video.js: the title video plays (the clock moves 10 ms a drawn frame from `videoentity::play`
on), each grab is read back at the swap where the video's time reaches T ms → <out-prefix>-t<T>.png, and the
`OpenBink`/`PlayBink`/`CloseBink` calls, the input and the frames go to <out-prefix>-video.json. --input posts a key
(space) or a left click to the window at video time T (the skip test: press, then release). The PNGs show game
imagery: write them under _local/ and never commit them.
--open plays NAME in the screen at (X, Y) instead (the message clips: `OpenBink(NAME, 0, X, Y, 1)`, `PlayBink(0, loop,
0)`, looping unless --once) over the main menu, the title skipped; the client's side is `--video-at X,Y`.
"""
import argparse
import atexit
import json
import os
import re
import struct
import sys
import time
import zlib

import frida

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--grabs", default="0")
ap.add_argument("--input", default="")
ap.add_argument("--size")
ap.add_argument("--seconds", type=int, default=120)
ap.add_argument("--real", action="store_true", help="keep the real clock (grabs at the first swap past T)")
ap.add_argument("--open", help="a video played in the screen (the message clips) instead of the title")
ap.add_argument("--at", default="201,204", help="--open's position in the 640x480 virtual space")
ap.add_argument("--full", action="store_true", help="--open plays fullscreen (`OpenBink(NAME, 1, 0, 0, 1)`, the credits)")
ap.add_argument("--probe", help="a JS file appended to video.js (its send()s go to the log's events)")
ap.add_argument("--once", action="store_true", help="--open plays once (the end clips) instead of looping")
ap.add_argument("--after", type=int, default=120, help="--open's frame (Fake_FRISSITES calls) after the start")
ap.add_argument("--step", type=int, default=0, help="also move the clock N ms a swap")
a = ap.parse_args()


def png_rgba_bottom_up(path, w, h, rgba):
    rows = b"".join(b"\0" + b"".join(rgba[((h - 1 - y) * w + px) * 4:((h - 1 - y) * w + px) * 4 + 3] for px in range(w)) for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b""))


grabs = [int(x) for x in a.grabs.split(",") if x]
inputs = []
for item in filter(None, a.input.split(",")):
    what, t = item.split("@")
    dev, ev = what.split(":")
    inputs.append({"t": int(t), "key": dev == "key", "down": ev == "down"})
log = {"events": [], "frames": []}
done = []


def on(msg, data):
    if msg["type"] != "send":
        print(msg, flush=True)
        done.append(1)
        return
    p = msg["payload"]
    if p.get("k") == "frame":
        path = f"{a.out}-t{p['t']}.png"
        png_rgba_bottom_up(path, p["w"], p["h"], data)
        log["frames"].append(dict(p, png=os.path.basename(path)))
        print(f"frame t={p['t']} elapsed={p['elapsed']} swaps={p['swaps']} {p['w']}x{p['h']} -> {path}", flush=True)
        return
    log["events"].append(p)
    print(p, flush=True)
    if p.get("k") == "done":
        done.append(1)


if a.size:
    settings = os.path.join(a.install, "ig2.settings")
    orig = open(settings, "rb").read()
    w, h = a.size.lower().split("x")
    s = re.sub(rb"(?m)^width=\d+", b"width=" + w.encode(), orig)
    open(settings, "wb").write(re.sub(rb"(?m)^height=\d+", b"height=" + h.encode(), s))
    atexit.register(lambda: open(settings, "wb").write(orig))
device = frida.get_local_device()
pid = device.spawn([os.path.join(a.install, "ig2.exe")], cwd=a.install)
session = device.attach(pid)
here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "video.js"), encoding="utf-8") as f:
    src = f.read()
if a.probe:
    with open(a.probe, encoding="utf-8") as f:
        src += "\n" + f.read()
script = session.create_script(src)
script.on("message", on)
script.load()
opened = None
if a.open:
    x, y = (int(v) for v in a.at.split(","))
    opened = {"name": a.open, "fullscreen": 1 if a.full else 0, "x": x, "y": y, "force": 1, "loop": 0 if a.once else 1, "after": a.after}
script.post({"type": "config", "payload": {"grabs": grabs, "input": inputs, "step": a.step, "real": a.real, "open": opened}})
device.resume(pid)
t0 = time.time()
try:
    while time.time() - t0 < a.seconds and not done:
        time.sleep(0.05)
finally:
    try:
        device.kill(pid)
    except Exception:
        pass
with open(f"{a.out}-video.json", "w", encoding="utf-8") as f:
    json.dump(log, f, indent=1)
print(f"video: {len(log['frames'])}/{len(grabs)} frames", flush=True)
sys.exit(0 if len(log["frames"]) == len(grabs) else 1)
