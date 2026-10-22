"""The sound tables (`Hang` records) of ig2.exe, found through the unstripped ELF (re/screens/sound.md §1).

    sound_tables.py <install> [--json OUT] [--names]

Each `<Screen>_Hangok` array in the ELF (`nm -S`) is a run of 0x11c-byte `Hang` records (file name at +0, the first
voice's handle +0x100, loop +0x104, voices 2 and 3 +0x108/+0x10c, volume % +0x110, load-on-demand +0x114). The
exe's copy is found by searching its .data for the same names at the same stride. Prints (and with --json writes)
{table: {"va": exe VA, "n": records}}, the input of sound.js; --names also prints each record's file name (they come
from the user's binaries: keep the output under _local/).
"""
import argparse
import json
import os
import re
import struct
import subprocess
import sys

REC = 0x11c


def elf_tables(elf):
    out = subprocess.run(["nm", "-S", "-C", "--defined-only", elf], capture_output=True, text=True, check=True).stdout
    tabs = {}
    for line in out.splitlines():
        m = re.match(r"([0-9a-f]+) ([0-9a-f]+) [dD] (\w+_Hangok|MessagesHang)$", line)
        if m:
            tabs[m.group(3)] = (int(m.group(1), 16), int(m.group(2), 16))
    return tabs


def elf_sections(data):
    # ELF64: section headers → (addr, offset, size) of the loaded PROGBITS sections.
    shoff, = struct.unpack_from("<Q", data, 0x28)
    shentsize, shnum = struct.unpack_from("<HH", data, 0x3a)
    secs = []
    for i in range(shnum):
        _, typ, _, addr, off, size = struct.unpack_from("<IIQQQQ", data, shoff + i * shentsize)
        if typ == 1 and addr:
            secs.append((addr, off, size))
    return secs


def pe_data(data):
    pe, = struct.unpack_from("<I", data, 0x3c)
    nsec, = struct.unpack_from("<H", data, pe + 6)
    opt, = struct.unpack_from("<H", data, pe + 20)
    base, = struct.unpack_from("<I", data, pe + 24 + 28)
    for i in range(nsec):
        s = pe + 24 + opt + 40 * i
        name = data[s:s + 8].rstrip(b"\0")
        vsize, va, rsize, roff = struct.unpack_from("<IIII", data, s + 8)
        if name == b".data":
            return base + va, roff, rsize
    sys.exit("no .data in the exe")


def name_at(buf, off):
    return buf[off:off + 0x100].split(b"\0")[0]


def find(install, names_out=None):
    """{table: {"va", "n"}} of the exe; names_out (a dict) gets each table's file names."""
    elf_path = os.path.join(install, "ig2")
    elf = open(elf_path, "rb").read()
    exe = open(os.path.join(install, "ig2.exe"), "rb").read()
    secs = elf_sections(elf)
    dva, doff, dsize = pe_data(exe)
    ddata = exe[doff:doff + dsize]
    res = {}
    for tab, (va, size) in sorted(elf_tables(elf_path).items()):
        off = next(o + va - s for s, o, n in secs if s <= va < s + n)
        n = size // REC
        names = [name_at(elf, off + k * REC) for k in range(n)]
        if not names[0]:
            # `MessagesHang`: one record filled at run time (`PlayMessagesHang`), nothing to search for.
            continue
        # The exe's table: the first name at a place where every other non-empty name follows at the stride.
        hits = [m.start() for m in re.finditer(re.escape(names[0] + b"\0"), ddata)]
        found = [h for h in hits if all(not nm or name_at(ddata, h + k * REC) == nm for k, nm in enumerate(names))]
        if len(found) != 1:
            print(f"{tab}: {len(found)} matches in the exe", file=sys.stderr)
            continue
        res[tab] = {"va": dva + found[0], "n": n}
        if names_out is not None:
            names_out[tab] = [nm.decode("latin-1") for nm in names]
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("install")
    ap.add_argument("--json")
    ap.add_argument("--names", action="store_true")
    a = ap.parse_args()
    names = {}
    res = find(a.install, names)
    for tab, t in res.items():
        print(f"{tab:<16} exe 0x{t['va']:x}  {t['n']} records")
        if a.names:
            for k, nm in enumerate(names[tab]):
                print(f"  {k:3} {nm}")
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump(res, f, indent=1)


if __name__ == "__main__":
    main()
