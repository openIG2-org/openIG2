"""Survey every file in the IG2 install: container type and decoded-header fingerprint per extension.

Usage: python -I survey_install.py <install-dir> [--extract <out-dir>]
Outputs a Markdown table on stdout. --extract writes decoded files OUTSIDE the repo for local study.
"""
import collections
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ecgs_decode as ec  # noqa: E402

MAGICS = {ec.MAGIC_EC: "ecGs", ec.MAGIC_NE: "neGs", ec.MAGIC_PC: "pcGs"}


def fingerprint(b: bytes) -> str:
    head = b[:16]
    if head[:4] == b"PVR\x03":
        return "PVR3"
    if head[:4] in (b"OggS", b"RIFF", b"PK\x03\x04", b"\x7fELF", b"MZ\x90\x00", b"%PDF"):
        return head[:4].decode("latin-1").strip("\x00\x03\x04\x90\x7f")
    if head[4:8] == b"ftyp":
        return "MP4"
    if head and all(32 <= c < 127 or c in (9, 10, 13) for c in b[:64]):
        return "text"
    return "bin:" + head[:8].hex()


def main() -> None:
    root = sys.argv[1]
    extract = sys.argv[sys.argv.index("--extract") + 1] if "--extract" in sys.argv else None
    stats = collections.defaultdict(lambda: collections.Counter())
    sizes = collections.Counter()
    failures = []
    for dirpath, _, files in os.walk(root):
        for fn in files:
            p = os.path.join(dirpath, fn)
            rel = os.path.relpath(p, root).replace("\\", "/")
            ext = os.path.splitext(fn)[1].lower() or "(none)"
            data = open(p, "rb").read()
            magic = MAGICS.get(struct.unpack_from("<I", data)[0] if len(data) >= 4 else 0, "plain")
            try:
                dec = ec.decode(fn, data) if magic != "plain" else data
                fp = fingerprint(dec)
            except Exception as e:  # noqa: BLE001
                failures.append((rel, repr(e)))
                fp, dec = "FAIL", b""
            stats[ext][f"{magic}->{fp}"] += 1
            sizes[ext] += len(dec)
            if extract and magic != "plain" and dec:
                out = os.path.join(extract, rel)
                os.makedirs(os.path.dirname(out), exist_ok=True)
                open(out, "wb").write(dec)
    print("| ext | files | decoded MB | container -> content |")
    print("|---|---|---|---|")
    for ext in sorted(stats, key=lambda e: -sum(stats[e].values())):
        kinds = ", ".join(f"{k} x{v}" for k, v in stats[ext].most_common(4))
        print(f"| {ext} | {sum(stats[ext].values())} | {sizes[ext] / 1e6:.1f} | {kinds} |")
    print(f"\nfailures: {len(failures)}")
    for f in failures[:20]:
        print("  ", f)


if __name__ == "__main__":
    main()
