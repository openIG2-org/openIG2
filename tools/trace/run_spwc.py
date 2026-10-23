"""Spawn the user's ig2.exe under Frida, run tools/trace/spwc.js, write a JSONL real-time battle trace.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_spwc.py <install-dir> <out.jsonl>
          [--steps 3000] [--battles 20] [--seed 4242] [--fuzz] [--ai-human] [--planets] [--join] [--seconds 560]
--ai-human: the battle AI plays the human's side too; --planets: half the battles are at a human planet
given defence ships and planet guns; --fuzz: random special pods, weapons, shields and experience;
--join: right after a battle starts another AI fleet joins it (SpaceWar_NewBattle's join path);
--fuzz-designs: random weapons, shields and pods in the participants' designs, big fighter wings, armour and
skipped/switched-off defence buildings.
--weapons ID,..: every slot of the participants' warship designs takes these tech ids in turn (hex or decimal), two
barrels each (captures of one weapon's look and sound).
--orders N [--order-every K]: every K ticks, N random battle-screen orders through the exe's own MULTIGET_SpaceWar
(the human's in-battle orders, spec §7.12), each an 'order' record with its byte diff.
--grab-ticks T,.. [--grab-settle F] [--state-out S.sav]: battle-screen captures (re/screens/spacewar.md): at each
battle tick T the clock pauses, F frames later the window is read back to <out>-t<T>.png and the battle's regions at
that moment go to <out>-t<T>.json ('init' shape, for ig2-client --screen spacewar --battle); the first grab also
writes the exe's own save to S (the top bar's state). Use with --battles 1.
--script S.json [--sounds OUT.jsonl] [--state-out S.sav]: an input script on the battle screen (screen.js's format and
clock, re/screens/spacewar.md §9.5): at its first frame the battle goes to <out>-entry.json (the grab shape) and the
exe's save to S.sav, then the events go in on the 10 ms UI clock and each grab t is read back to <out>-t<T>.png, its
state to <out>-frames.json; the exe's pause and speeds are left alone. --sounds writes every sound the exe starts and
every input event (sound.js) to OUT.jsonl. Implies --battles 1.
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
ap.add_argument("--weapons", default="")
ap.add_argument("--speed", type=int, default=4)
ap.add_argument("--per-battle", type=int, default=1500)
ap.add_argument("--orders", type=int, default=0)
ap.add_argument("--order-every", type=int, default=5)
ap.add_argument("--grab-ticks", default="")
ap.add_argument("--grab-settle", type=int, default=120)
ap.add_argument("--state-out")
ap.add_argument("--script")
ap.add_argument("--formation", type=int, default=0, help="N: a random formation pick on the battle screen with a chance of 1/N per driver frame ('preform'/'form' snapshots around loadfelallas)")
ap.add_argument("--sounds")
ap.add_argument("--watch", help="T:OFF[:N]: a hardware write watchpoint on the slot's byte OFF from script tick T (the first N writers)")
ap.add_argument("--watch-addr", help="VA[:N]: a hardware write watchpoint on an exe global from just before the scenario's load (the first N writers, the value read as a string)")
a = ap.parse_args()
input_script = json.load(open(a.script, encoding="utf-8")) if a.script else None
if input_script:
    a.battles = 1
sounds = open(a.sounds, "w", encoding="utf-8") if a.sounds else None
frames_meta = {"entry": None, "frames": [], "done": None}

exe = os.path.join(a.install, "ig2.exe")
here = os.path.dirname(os.path.abspath(__file__))
agent = open(os.path.join(here, "spwc.js"), encoding="utf-8").read()
sound_tables = None
if sounds:
    agent += "\n" + open(os.path.join(here, "sound.js"), encoding="utf-8").read()
    sys.path.insert(0, here)
    import sound_tables as _st
    sound_tables = _st.find(a.install)
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
    if p.get("k") in ("sound", "input", "music", "track", "fade"):
        if sounds:
            sounds.write(json.dumps(p) + "\n")
            sounds.flush()
        if p.get("k") == "sound":
            print(f"sound {p.get('fn')} {p.get('table')}[{p.get('idx')}] {p.get('name')} t={p.get('t')}", flush=True)
        return
    if p.get("type") == "entry":
        frames_meta["entry"] = {k: v for k, v in p.items() if k not in ("regions", "statics", "obj")}
        with open(f"{stem}-entry.json", "w", encoding="utf-8") as f:
            json.dump(p, f)
        print(f"entry: battle tick {p['tick']} paused {p['paused']} speed {p['speed']}/{p['battleSpeed']} -> {stem}-entry.json", flush=True)
        return
    if p.get("type") == "frame":
        path = f"{stem}-t{p['t']}.png"
        png_rgba_bottom_up(path, p["w"], p["h"], data)
        frames_meta["frames"].append(dict(p, png=os.path.basename(path)))
        print(f"frame t={p['t']} ticks={p['ticks']} battle={p['battleSteps']} paused={p['paused']} fade={p['fade']:.3f} -> {path}", flush=True)
        return
    if p.get("type") == "scriptDone":
        frames_meta["done"] = p
        print(f"script done: {p.get('scriptT')} ticks", flush=True)
        return
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
            if ev["k"] in ("write", "watch", "error", "start", "started", "populated", "left", "multi", "abandoned", "groundwar", "join", "grabbing"):
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
                                           "planets": a.planets, "join": a.join, "fuzzDesigns": a.fuzz_designs, "weapons": [int(x, 0) for x in a.weapons.split(",") if x], "speed": a.speed, "perBattle": a.per_battle,
                                           "orders": a.orders, "orderEvery": a.order_every, "grabTicks": grab_ticks,
                                           "grabSettle": a.grab_settle, "stateSlot": STATE_SLOT if a.state_out else 0,
                                           "script": input_script, "soundTables": sound_tables, "formation": a.formation,
                                           "watch": dict(zip(("t", "off", "n"), (int(x, 0) for x in a.watch.split(":")))) if a.watch else None,
                                           "watchAddr": dict(zip(("addr", "n"), (int(x, 0) for x in a.watch_addr.split(":")))) if a.watch_addr else None}})
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
    if sounds:
        sounds.close()
    if input_script:
        with open(f"{stem}-frames.json", "w", encoding="utf-8") as f:
            json.dump(frames_meta, f)
        print(f"script: {len(frames_meta['frames'])}/{len(input_script.get('grabs', []))} frames, done {frames_meta['done'] is not None}")
    if state_slot:
        if os.path.exists(state_slot):
            shutil.move(state_slot, a.state_out)
        if os.path.exists(state_slot + ".spwc-aside"):
            shutil.move(state_slot + ".spwc-aside", state_slot)
    print(f"events: {counts} -> {a.out}")
