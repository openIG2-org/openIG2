"""Count calls to a few ig2.exe functions for N seconds (diagnostic for trace hook choice).

Usage: _local/venv/Scripts/python.exe tools/trace/probe_calls.py <install-dir> [seconds]
"""
import os
import sys
import time

import frida

install = sys.argv[1]
seconds = float(sys.argv[2]) if len(sys.argv) > 2 else 15
targets = {
    "IG2_gameUpdate": 0x4E0440,
    "Fake_FRISSITES": 0x4DE720,
    "rndlog": 0x542DA0,
    "IdoNapValtozott": 0x53E970,
    "SetIdoSpeed": 0x541330,
}
agent = """
const mod = Process.getModuleByName('ig2.exe');
const T = %s;
const counts = {};
for (const [name, va] of Object.entries(T)) {
  counts[name] = 0;
  Interceptor.attach(mod.base.add(va - 0x400000), { onEnter() { counts[name]++; } });
}
setInterval(() => send(counts), 2000);
""" % ({k: v for k, v in targets.items()},)
agent = agent.replace("'", '"')

dev = frida.get_local_device()
pid = dev.spawn([os.path.join(install, "ig2.exe")], cwd=install)
s = dev.attach(pid)
sc = s.create_script(agent)
sc.on("message", lambda m, d: print(m.get("payload", m)))
sc.load()
dev.resume(pid)
time.sleep(seconds)
dev.kill(pid)
