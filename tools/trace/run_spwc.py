"""Spawn the user's ig2.exe under Frida, run tools/trace/spwc.js, write a JSONL real-time battle trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_spwc.py <install-dir> <out.jsonl>
          [--steps 3000] [--battles 20] [--seed 4242] [--fuzz] [--ai-human] [--planets] [--join] [--seconds 560]
--ai-human: the battle AI plays the human's side too; --planets: half the battles are at a human planet
given defence ships and planet guns; --fuzz: random special pods, weapons, shields and experience;
--join: right after a battle starts another AI fleet joins it (SpaceWar_NewBattle's join path);
--fuzz-designs: random weapons, shields and pods in the participants' designs, big fighter wings, armour and
skipped/switched-off defence buildings.
--orders N [--order-every K]: every K ticks, N random battle-screen orders through the exe's own MULTIGET_SpaceWar
(the human's in-battle orders, spec §7.12), each an 'order' record with its byte diff.
--grab-ticks T,.. [--grab-settle F] [--state-out S.sav]: battle-screen captures (re/screens/spacewar.md): at each
battle tick T the clock pauses, F frames later the window is read back to <out>-t<T>.png and the battle's regions at
that moment go to <out>-t<T>.json ('init' shape, for ig2-client --screen spacewar --battle); the first grab also
writes the exe's own save to S (the top bar's state). Use with --battles 1.
Every SpaceWarIndit call is wrapped in 'presetup'/'setup' snapshots (the battle setup, spec §7.10), and
leaving the battle screen records the end globals ('left', §7.11).
The agent starts real-time battles (human fleet vs AI fleet) and records every SpwC::SpaceWarFrissit call
(exe 0x56dff0): the full state at a battle's first step ('init'), then byte diffs per step ('step') and
between steps ('between'). Run it in the foreground (a background shell stalled the game).
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Spec: re/rules/space-combat.md §7.
"""
import argparse
import json
import os
import sys
import time

import frida
import shutil
import struct
import zlib

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--seconds", type=float, default=560)
ap.add_argument("--steps", type=int, default=3000)
ap.add_argument("--battles", type=int, default=20)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--ai-human", action="store_true")
ap.add_argument("--planets", action="store_true")
ap.add_argument("--join", action="store_true")
ap.add_argument("--fuzz-designs", action="store_true")
ap.add_argument("--speed", type=int, default=4)
ap.add_argument("--per-battle", type=int, default=1500)
ap.add_argument("--orders", type=int, default=0)
ap.add_argument("--order-every", type=int, default=5)
ap.add_argument("--grab-ticks", default="")
ap.add_argument("--grab-settle", type=int, default=120)
ap.add_argument("--state-out")
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "spwc.js"), encoding="utf-8").read()
out = open(a.out, "w", encoding="utf-8")
counts = {}
finished = False


grab_ticks = [int(t) for t in a.grab_ticks.split(",") if t]
stem = os.path.splitext(a.out)[0]
data_dir = os.path.join(os.environ["LOCALAPPDATA"], "GyroscopeGames", "ig2")
STATE_SLOT = 9
state_slot = os.path.join(data_dir, f"save{STATE_SLOT}.sav") if a.state_out else None
if state_slot and os.path.exists(state_slot):
    shutil.move(state_slot, state_slot + ".spwc-aside")


def png_rgba_bottom_up(path, w, h, rgba):
    # glReadPixels' rows, bottom first, RGBA.
    rows = b"".join(b"\0" + b"".join(rgba[((h - 1 - y) * w + px) * 4:((h - 1 - y) * w + px) * 4 + 3] for px in range(w)) for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b""))


def on_message(msg, data):
    global finished
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    if p.get("type") == "shot":
        base = f"{stem}-t{p['tick']}"
        png_rgba_bottom_up(base + ".png", p["w"], p["h"], data)
        with open(base + ".json", "w", encoding="utf-8") as f:
            json.dump(p, f)
        print(f"grab t={p['tick']} {p['w']}x{p['h']} -> {base}.png", flush=True)
        return
    if p.get("type") == "batch":
        for ev in p["events"]:
            counts[ev["k"]] = counts.get(ev["k"], 0) + 1
            if ev["k"] in ("error", "start", "started", "populated", "left", "multi", "abandoned", "groundwar", "join", "grabbing"):
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
script = session.create_script(agent)
script.on("message", on_message)
script.load()
script.post({"type": "config", "payload": {"scenario": a.scenario, "steps": a.steps, "battles": a.battles,
                                           "seed": a.seed, "fuzz": a.fuzz, "aiHuman": a.ai_human,
                                           "planets": a.planets, "join": a.join, "fuzzDesigns": a.fuzz_designs, "speed": a.speed, "perBattle": a.per_battle,
                                           "orders": a.orders, "orderEvery": a.order_every, "grabTicks": grab_ticks,
                                           "grabSettle": a.grab_settle, "stateSlot": STATE_SLOT if a.state_out else 0}})
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
    out.close()
    if state_slot:
        if os.path.exists(state_slot):
            shutil.move(state_slot, a.state_out)
        if os.path.exists(state_slot + ".spwc-aside"):
            shutil.move(state_slot + ".spwc-aside", state_slot)
    print(f"events: {counts} -> {a.out}")
