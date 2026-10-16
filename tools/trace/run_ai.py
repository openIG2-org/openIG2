"""Spawn the user's ig2.exe under Frida, run tools/trace/ai.js, write a JSONL trace of the AI empire's daily pass.

Usage (project venv): _local/venv/Scripts/python.exe tools/trace/run_ai.py <install-dir> <out.jsonl>
          [--count N] [--fuzz] [--ships] [--seconds 560] [--seed 4242]
One record per FRISSIT_MI (exe 0x50a6e0) call: byte-region diffs ('init' = full dump, 'pre' = changes before the
call, then a checkpoint diff when entering and leaving FRISSIT_MI_SZAMOLAS and, per AI empire, FRISSIT_MI_GYARTAS,
FRISSIT_MI_KEMKEDES and FRISSIT_MI_FEJLESZTES, and at the end), with the callees reached (coverage). --fuzz
revives empires 1..8 as AIs and perturbs the AI's inputs before each call; --ships adds warships and hands the
human's empire to the AI (otherwise real-time battles stop the clock on the battle screen).
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Record to _local/traces/pending-ai-*.jsonl and rename to ai-*.jsonl when complete (the replay globs ai-*).
Run in the foreground with --seconds 560 (the tool limit is 600 s): from a background shell the game stalled.
Spec: re/rules/ai-empire.md.
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
ap.add_argument("--seconds", type=float, default=560)
ap.add_argument("--count", type=int, default=400)
ap.add_argument("--seed", type=int, default=4242)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--fuzz", action="store_true")
ap.add_argument("--debug", action="store_true")
ap.add_argument("--ships", action="store_true", help="add warships; hands the human's empire to the AI")
a = ap.parse_args()

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "ai.js"), encoding="utf-8").read()
out = open(a.out, "w", encoding="utf-8")
counts = {}
finished = False
last_event = time.time()
PROBE = """
const mod = Process.getModuleByName('ig2.exe');
const rel = (p) => { const o = p.sub(mod.base).toInt32(); return o >= 0 && o < mod.size ? '0x' + (o + 0x400000).toString(16) : String(p); };
const raw = (sp) => { for (const n of [0x10000, 0x4000, 0x1000, 0x400]) { try { const w = new Uint32Array(sp.readByteArray(n)); const out = []; for (const x of w) { const o = x - mod.base.toUInt32(); if (o > 0x1000 && o < 0x430000) out.push('0x' + (o + 0x400000).toString(16)); } return out.slice(0, 60); } catch (e) {} } return []; };
const sym = (p) => { const m = Process.findModuleByAddress(p); return m ? m.name + '+0x' + p.sub(m.base).toString(16) : String(p); };
const g = (va) => mod.base.add(va - 0x400000);
send({ kep: g(0xbae0fc).readS32(), kepkell: g(0xbae250).readS32(), ido: g(0xbae04c).readS32(), speed: g(0xbae25c).readS32(),
  threads: Process.enumerateThreads().slice(0, 4).map((t) => ({ id: t.id, pc: sym(t.context.pc), bt: Thread.backtrace(t.context, Backtracer.FUZZY).slice(0, 20).map(sym), raw: raw(t.context.sp) })) });
"""


def on_message(msg, _data):
    global finished, last_event
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    last_event = time.time()
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
                                           "debug": a.debug, "ships": a.ships, "aiHuman": a.ships}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; fuzz {a.fuzz}, up to {a.seconds}s")
t0 = time.time()
try:
    probed = False
    while time.time() - t0 < a.seconds and not finished:
        time.sleep(1)
        if a.debug and not probed and time.time() - last_event > 25:
            # The game thread is stuck (possibly inside the tracer's own callbacks): sample every stack from a
            # second script, which has its own lock.
            probed = True
            probe = session.create_script(PROBE)
            probe.on("message", lambda m, _d: out.write(json.dumps({"k": "hang", "threads": m.get("payload")}) + chr(10)))
            probe.load()
            time.sleep(2)
    time.sleep(1)
finally:
    try:
        script.unload()
    except frida.InvalidOperationError:
        pass
    device.kill(pid)
    out.close()
    print(f"events: {counts} -> {a.out}")
