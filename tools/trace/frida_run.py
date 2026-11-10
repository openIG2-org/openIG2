"""Shared start-up for the run_*.py tracers: agent source, spawn/attach/post/resume/kill, the batch-of-events
message handler, the stall probe and the PNG writer. A runner keeps its own arguments, config and per-event prints."""
import json
import os
import struct
import sys
import time
import zlib

import frida

HERE = os.path.dirname(os.path.abspath(__file__))
# Prepended to every agent that uses it (agents no longer declare these themselves).
HEADER = """const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);
"""
# Samples every thread's stack from a second script (own lock) when the game thread hangs inside the tracer.
PROBE = """
const mod = Process.getModuleByName('ig2.exe');
const rel = (p) => { const o = p.sub(mod.base).toInt32(); return o >= 0 && o < mod.size ? '0x' + (o + 0x400000).toString(16) : String(p); };
const raw = (sp) => { for (const n of [0x10000, 0x4000, 0x1000, 0x400]) { try { const w = new Uint32Array(sp.readByteArray(n)); const out = []; for (const x of w) { const o = x - mod.base.toUInt32(); if (o > 0x1000 && o < 0x430000) out.push('0x' + (o + 0x400000).toString(16)); } return out.slice(0, 60); } catch (e) {} } return []; };
const sym = (p) => { const m = Process.findModuleByAddress(p); return m ? m.name + '+0x' + p.sub(m.base).toString(16) : String(p); };
const g = (va) => mod.base.add(va - 0x400000);
send({ kep: g(0xbae0fc).readS32(), kepkell: g(0xbae250).readS32(), ido: g(0xbae04c).readS32(), speed: g(0xbae25c).readS32(),
  threads: Process.enumerateThreads().slice(0, 4).map((t) => ({ id: t.id, pc: sym(t.context.pc), bt: Thread.backtrace(t.context, Backtracer.FUZZY).slice(0, 20).map(sym), raw: raw(t.context.sp) })) });
"""


def agent_source(name):
    """tools/trace/<name> with the common header in front."""
    with open(os.path.join(HERE, name), encoding="utf-8") as f:
        return HEADER + f.read()


def png_rgba_bottom_up(path, w, h, rgba):
    # glReadPixels' rows, bottom first, RGBA.
    rows = b"".join(b"\0" + b"".join(rgba[((h - 1 - y) * w + px) * 4:((h - 1 - y) * w + px) * 4 + 3] for px in range(w)) for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b""))


class Tracer:
    """One ig2.exe run under one agent.

    on_event(ev): called for each record of a {"type": "batch"} message (the record is written to `out` as a JSON line
    afterwards, `counts[ev["k"]]` is kept, a "done" record sets `finished`). on_payload(p, data): replaces that
    handling for runners whose agents send other shapes. Without `out_path` nothing is written."""

    def __init__(self, install, agent, out_path=None, *, config=None, banner="", on_event=None, on_payload=None,
                 v8=False, exe_args=()):
        self.install, self.banner, self.v8 = install, banner, v8
        self.agent, self.config, self.exe_args = agent, config, list(exe_args)
        self.out = open(out_path, "w", encoding="utf-8") if out_path else None
        self.on_event, self.on_payload = on_event, on_payload
        self.counts, self.finished, self.last_event = {}, False, time.time()
        self.device = self.pid = self.session = self.script = None

    def _message(self, msg, data):
        if msg["type"] == "error":
            print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
            return
        p = msg["payload"]
        self.last_event = time.time()
        if self.on_payload:
            self.on_payload(p, data)
        elif p.get("type") == "batch":
            for ev in p["events"]:
                self.counts[ev["k"]] = self.counts.get(ev["k"], 0) + 1
                if self.on_event:
                    self.on_event(ev)
                if ev["k"] == "done":
                    self.finished = True
                if self.out:
                    self.out.write(json.dumps(ev) + "\n")
            if self.out:
                self.out.flush()
        else:
            print("agent:", p)

    def start(self):
        self.device = frida.get_local_device()
        self.pid = self.device.spawn([os.path.join(self.install, "ig2.exe")] + self.exe_args, cwd=self.install)
        self.session = self.device.attach(self.pid)
        if self.v8:
            try:
                self.script = self.session.create_script(self.agent, runtime="v8")
            except frida.InvalidArgumentError:
                self.script = self.session.create_script(self.agent)
        else:
            self.script = self.session.create_script(self.agent)
        self.script.on("message", self._message)
        self.script.load()
        if self.config is not None:
            self.script.post({"type": "config", "payload": self.config})
        self.device.resume(self.pid)
        print(self.banner.format(pid=self.pid))

    def wait(self, seconds, *, until_done=True, poll=1, tail=1, each=None):
        """Sleep until `seconds` pass or (until_done) a "done" record arrives; each() returning True ends it early."""
        t0 = time.time()
        while time.time() - t0 < seconds and not (until_done and self.finished):
            time.sleep(min(poll, max(0, seconds - (time.time() - t0))))
            if each and each():
                break
        time.sleep(tail)

    def probe_hang(self):
        """Samples every thread's stack into the output as a "hang" record."""
        probe = self.session.create_script(PROBE)
        probe.on("message", lambda m, _d: self.out.write(json.dumps({"k": "hang", "threads": m.get("payload")}) + chr(10)))
        probe.load()
        time.sleep(2)

    def stop(self):
        """Unload the agent, kill the game, close the output (safe when start() failed or the game already died)."""
        if self.script:
            try:
                self.script.unload()
            except frida.InvalidOperationError:
                pass
        if self.pid is not None:
            try:
                self.device.kill(self.pid)
            except frida.ProcessNotFoundError:
                print("game process already gone (crashed?)", file=sys.stderr)
        if self.out:
            self.out.close()
