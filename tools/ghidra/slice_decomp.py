"""Print one piece of a big decompile or disassembly instead of the whole file.

The story scripts (`st_*_include`) decompile to 100-300 KB files; a port needs only the blocks it uses.
A block starts at a marker line (default: the `st_Story(` test that opens every story block) and runs to the
next marker. Both binaries keep the blocks in the same order, so the n-th marker of the exe's output (where
`st_Story` is a `FUN_00602f80()` call without visible arguments) pairs with the n-th of the ELF's.

  slice_decomp.py FILE --list                       markers with their ordinals, ids and line numbers
  slice_decomp.py ELF.c --story 0x32e [--pair EXE]  that story's block (and the exe's block at the same ordinal)
  slice_decomp.py FILE --block N [--pair OTHER]     the block opened by marker N (0-based)
  slice_decomp.py FILE --lines 600-700              a line range
  slice_decomp.py FILE --grep REGEX [-C 3]          matching lines with context
  slice_decomp.py DIR --list                        the files of a decompile folder with sizes

Markers: `--marker REGEX` (default matches `st_Story(`, `FUN_00602f80(` and `call 0x602f80`).
"""
import argparse, pathlib, re, sys

DEFAULT_MARKER = r"\bst_Story\(|FUN_00602f80\(|call\s+0x602f80\b"


def read(path):
    return path.read_text(encoding="utf-8", errors="replace").splitlines()


def markers(lines, rx):
    return [i for i, l in enumerate(lines) if rx.search(l)]


def story_id(line):
    m = re.search(r"st_Story\((0x[0-9a-fA-F]+|\d+)", line)
    return int(m.group(1), 0) if m else None


def asm_id(lines, i):
    """The id pushed for a call in a disassembly: the nearest `push imm` (or `mov reg, imm`) above the call."""
    for l in reversed(lines[max(0, i - 6):i]):
        m = re.search(r"\b(push|mov)\s+(?:\w+,\s*)?(0x[0-9a-f]+)\s*$", l)
        if m:
            v = int(m.group(2), 16)
            if 0x300 <= v < 0x400:
                return v
    return None


def starts(lines, marks):
    """Whether each marker opens a block: `st_Story(id, 1)` does, a query like `st_Story(0x195, 0)` inside a
    block doesn't. Markers without visible arguments (the exe's decompile) all count."""
    out = []
    for i in marks:
        m = re.search(r"st_Story\((?:0x[0-9a-fA-F]+|\d+)\s*,\s*(\d+)", lines[i])
        out.append(m is None or m.group(1) != "0")
    return out


def end_ordinal(opens, n):
    k = n + 1
    while k < len(opens) and not opens[k]:
        k += 1
    return k


def span(lines, marks, n, k):
    return marks[n], marks[k] if k < len(marks) else len(lines)


def show(path, lines, a, b, label):
    print(f"==== {path} lines {a + 1}-{b} ({label})")
    for i in range(a, b):
        print(f"{i + 1:6}: {lines[i]}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("file", type=pathlib.Path)
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--story", type=lambda s: int(s, 0))
    ap.add_argument("--block", type=int)
    ap.add_argument("--pair", type=pathlib.Path)
    ap.add_argument("--lines")
    ap.add_argument("--grep")
    ap.add_argument("-C", type=int, default=2)
    ap.add_argument("--marker", default=DEFAULT_MARKER)
    a = ap.parse_args()

    if a.file.is_dir():
        for p in sorted(a.file.iterdir(), key=lambda p: -p.stat().st_size):
            print(f"{p.stat().st_size:9}  {p.name}")
        return
    lines = read(a.file)
    rx = re.compile(a.marker)
    marks = markers(lines, rx)

    if a.list:
        for n, i in enumerate(marks):
            sid = story_id(lines[i]) or asm_id(lines, i)
            print(f"{n:3}  line {i + 1:6}  id {hex(sid) if sid else '?':6}  {lines[i].strip()[:80]}")
        return
    if a.lines:
        lo, hi = (int(x) for x in a.lines.split("-"))
        show(a.file, lines, lo - 1, min(hi, len(lines)), "range")
        return
    if a.grep:
        g = re.compile(a.grep)
        last = -1
        for i, l in enumerate(lines):
            if g.search(l):
                lo, hi = max(0, i - a.C, last + 1), min(len(lines), i + a.C + 1)
                if lo > last + 1:
                    print("--")
                for j in range(lo, hi):
                    print(f"{j + 1:6}: {lines[j]}")
                last = hi - 1
        return

    n = a.block
    if a.story is not None:
        ids = [story_id(lines[i]) or asm_id(lines, i) for i in marks]
        if a.story not in ids:
            sys.exit(f"story {a.story:#x} has no marker in {a.file}")
        n = ids.index(a.story)
    if n is None:
        sys.exit("give --list, --story, --block, --lines or --grep")
    k = end_ordinal(starts(lines, marks), n)
    lo, hi = span(lines, marks, n, k)
    show(a.file, lines, lo, hi, f"markers {n}..{k - 1}")
    if a.pair:
        other = read(a.pair)
        om = markers(other, rx)
        if len(om) != len(marks):
            print(f"warning: {len(marks)} markers here, {len(om)} in {a.pair}: pairing by ordinal may be off",
                  file=sys.stderr)
        if n < len(om):
            lo, hi = span(other, om, n, k)
            show(a.pair, other, lo, hi, f"markers {n}..{k - 1}")


if __name__ == "__main__":
    main()
