"""Verify the rules-table catalogue against live ig2.exe memory (Frida) and the exe file.

Usage: _local/venv/Scripts/python.exe tools/trace/verify_tables.py <install-dir> <decoded quick.scn> <report.md>
Checks:
  1. every static table read from the exe file equals live memory at the main menu and after a scenario load;
  2. the live tech table (Talalmanyok) equals Talalmanyok0 at load except where the scenario overrides it,
     and the identified field offsets hold exactly the .scn TALALMANY values.
"""
import os
import struct
import sys
import time

import frida

install, scn_path, report = sys.argv[1:4]
exe_bytes = open(os.path.join(install, "ig2.exe"), "rb").read()

# exe VA → file offset via the PE section table
pe = struct.unpack_from("<I", exe_bytes, 0x3C)[0]
nsec = struct.unpack_from("<H", exe_bytes, pe + 6)[0]
opt = struct.unpack_from("<H", exe_bytes, pe + 20)[0]
ib = struct.unpack_from("<I", exe_bytes, pe + 52)[0]
secs = [struct.unpack_from("<IIII", exe_bytes, pe + 24 + opt + 40 * i + 8) for i in range(nsec)]


def file_bytes(va, size):
    for vsize, sva, rsize, roff in secs:
        if ib + sva <= va < ib + sva + rsize:
            o = roff + va - ib - sva
            return exe_bytes[o:o + size]
    raise ValueError(hex(va))


TABLES = [  # name, exe VA, size  (catalogue: re/rules/tables.md)
    ("Talalmanyok0", 0x8DE048, 53268), ("Talalmanyok2", 0x8EB060, 1737),
    ("TalalmanyokrEro", 0x8EB730, 772), ("ScirmishTalalmanyFejlesztes", 0x8EC218, 772),
    ("EpuletekLeiro", 0x8EBA38, 2016), ("TankTorony", 0x8EC538, 1064), ("TankAlap", 0x8F50D8, 280),
    ("WF_Weapons", 0x8F82C0, 224), ("WF_NonSimulateWeapons", 0x8F83A0, 224), ("WF_FortressWeapons", 0x8F8480, 224),
    ("PodTipusok", 0x8F8918, 4620), ("PodFajok", 0x8F9B28, 36288), ("HanyasFajPodTablazatatHasznalja", 0x902F10, 76),
    ("FajBolygoViszony", 0x90B9D8, 540), ("Alapok", 0x90BBF8, 900), ("Tervek0", 0x90BF80, 26532),
    ("Research_MeretSzorzok", 0x9185D0, 768), ("tipusnevek", 0x91F488, 11520), ("fegyverek", 0x91D858, 3680),
    ("TransfAlap", 0x927410, 1400),
    ("FajSpeckoGyartasKapacitasSzorzo", 0x8F8870, 60), ("FajSpeckoKutatasIdoSzorzo", 0x8F87F8, 60),
    ("NehezsegGyartasSzorzo", 0x8F8834, 20), ("NehezsegPenzSzorzo", 0x8F8848, 20), ("FejlesztesSebessegSzorzo", 0x8F885C, 20),
    ("FajBekeMogalomSzint", 0x90B924, 60), ("FajKereskedelemIgenySzint", 0x90B960, 60),
    ("FajFelelemOnmegsemmisitotolSzint", 0x90B99C, 60), ("FajMiBolygokHanyadReszeGyartas", 0x915F90, 60),
    ("FajMiBolygokMilyenKatonaiAutiBuild", 0x915FCC, 60),
    ("Talalmanyok(live)", 0xB9C0C8, 53268),
]
dumps = {}


def on_message(msg, data):
    if msg["type"] == "error":
        print("agent error:", msg.get("description"))
        return
    p = msg["payload"]
    if p.get("type") == "dump":
        dumps[(p["phase"], p["name"])] = data


agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "dump_tables.js"), encoding="utf-8").read()
dev = frida.get_local_device()
pid = dev.spawn([os.path.join(install, "ig2.exe")], cwd=install)
sess = dev.attach(pid)
sc = sess.create_script(agent)
sc.on("message", on_message)
sc.load()
sc.post({"type": "config", "payload": {"startAfterFrames": 1200, "scenario": "scenario/quick.scn",
                                       "tables": [{"name": n, "va": v, "size": s} for n, v, s in TABLES]}})
dev.resume(pid)
for _ in range(60):
    time.sleep(1)
    if len(dumps) >= 2 * len(TABLES):
        break
dev.kill(pid)
dump_dir = os.path.join(os.path.dirname(os.path.abspath(report)), "..", "..", "..", "_local", "traces", "tables")
os.makedirs(dump_dir, exist_ok=True)
for (phase, name), data in dumps.items():
    open(os.path.join(dump_dir, f"{phase}_{name}.bin"), "wb").write(data)

out = ["# Runtime verification: rules tables (Frida, ig2.exe)", "",
       "Spawned `ig2.exe`, dumped each table at the main menu and after `Betolt_Scenario(\"scenario/quick.scn\")`.", "",
       "| Table | exe VA | size | file = menu | file = after load | differing bytes after load |", "|---|---|---|---|---|---|"]
for n, v, s in TABLES:
    # .bss tables have no file bytes; the live tech table is compared with its static default.
    fb = file_bytes(0x8DE048, s) if n == "Talalmanyok(live)" else file_bytes(v, s)
    m, a = dumps.get(("menu", n)), dumps.get(("scenario", n))
    diff = sum(1 for x, y in zip(fb, a) if x != y) if a else "?"
    out.append(f"| {n} | {v:#x} | {s} | {m == fb if m else '?'} | {a == fb if a else '?'} | {diff} |")

# Field check: live Talalmanyok vs .scn TALALMANY overrides.
live = dumps.get(("scenario", "Talalmanyok(live)"))
recs, cur = [], None
for line in open(scn_path, encoding="latin-1"):
    t = line.split()
    if not t:
        continue
    if t[0].upper() == "TALALMANY":
        cur = {}
        recs.append(cur)
    elif cur is not None and t[0] in ("rNeeded", "rKutatasSzint", "rKutatasMoney", "rKutatasIdo"):
        cur[t[0]] = [int(float(x)) for x in t[1:]]
fields = {"rKutatasMoney": [(52, "<i")], "rKutatasIdo": [(56, "<i")],
          "rNeeded": [(45, "<B"), (46, "<B"), (47, "<B")], "rKutatasSzint": [(49, "<B"), (50, "<B"), (51, "<B")]}
out += ["", "## Live tech table vs `.scn` TALALMANY (192 records, tech 1..192)", "", "| scn key | offsets | matching records |", "|---|---|---|"]
if live:
    for key, offs in fields.items():
        ok = tot = 0
        for i, r in enumerate(recs, start=1):
            if key not in r:
                continue
            tot += 1
            got = [struct.unpack_from(f, live, i * 276 + o)[0] for o, f in offs]
            ok += got == r[key][:len(offs)]
        out.append(f"| {key} | {[o for o, _ in offs]} | {ok}/{tot} |")
open(report, "w", encoding="utf-8").write("\n".join(out) + "\n")
print("\n".join(out))
