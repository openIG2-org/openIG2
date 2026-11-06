"""Align another ig2.exe build's data sections with the GOG 2017 build and print the Rust segment table.

Usage: python -I tools/exe_builds/align.py <gog ig2.exe> <other ig2.exe> <name> [--rs out.rs]

Both images' relocated dwords (pointers) are masked, unique 16-byte windows of the GOG .rdata/.data are found in
the other build, and runs of equal offset become segments (gog_va, other_va, len). Pointer values are then checked:
a pointer in a mapped segment must map back to the GOG pointer at the same place. Prints coverage and mismatches.
Spec: re/formats/exe-builds.md.
"""
import struct, sys
from collections import defaultdict

DATA = ('.rdata', '.data', '.rodata', '_RDATA')


def pe(path):
    b = open(path, 'rb').read()
    p = struct.unpack_from('<I', b, 0x3c)[0]
    n, = struct.unpack_from('<H', b, p + 6)
    opt, = struct.unpack_from('<H', b, p + 20)
    base, = struct.unpack_from('<I', b, p + 24 + 28)
    reloc_rva, reloc_size = struct.unpack_from('<II', b, p + 24 + 96 + 5 * 8)
    secs = {}
    for i in range(n):
        o = p + 24 + opt + 40 * i
        name = b[o:o + 8].rstrip(b'\0').decode()
        vs, va, rs, ro = struct.unpack_from('<IIII', b, o + 8)
        secs[name] = (base + va, vs, ro, rs)

    def off(va):
        for (sva, vs, ro, rs) in secs.values():
            if sva <= va < sva + rs:
                return ro + va - sva
        return None
    relocs = set()
    at, end = off(base + reloc_rva), off(base + reloc_rva) + reloc_size
    while at < end:
        page, size = struct.unpack_from('<II', b, at)
        if size == 0:
            break
        for k in range((size - 8) // 2):
            e, = struct.unpack_from('<H', b, at + 8 + 2 * k)
            if e >> 12 == 3:
                relocs.add(base + page + (e & 0xfff))
        at += size
    return b, base, secs, off, relocs


def masked(img, name):
    b, base, secs, off, relocs = img
    va, vs, ro, rs = secs[name]
    m = bytearray(b[ro:ro + rs])
    for r in relocs:
        if va <= r < va + rs - 3:
            m[r - va:r - va + 4] = b'\xa5\xa5\xa5\xa5'
    return va, bytes(m)


def align(g, o, name, W=16):
    gva, gm = masked(g, name)
    ova, om = masked(o, name)
    index = defaultdict(list)
    for i in range(0, len(om) - W + 1, 4):
        index[om[i:i + W]].append(i)
    delta = [None] * len(gm)
    for i in range(0, len(gm) - W + 1, 4):
        w = gm[i:i + W]
        if len(set(w)) < 4:  # zero runs and fills match anywhere
            continue
        hits = index.get(w)
        if hits and len(hits) == 1:
            d = hits[0] - i
            for k in range(W):
                delta[i + k] = d
    # extend each anchored run byte by byte while bytes still agree
    for direction in (1, -1):
        rng = range(len(gm)) if direction == 1 else range(len(gm) - 1, -1, -1)
        prev = None
        for i in rng:
            if delta[i] is not None:
                prev = delta[i]
            elif prev is not None and 0 <= i + prev < len(om) and gm[i] == om[i + prev]:
                delta[i] = prev
            else:
                prev = None
    segs = []
    for i, d in enumerate(delta):
        if d is None:
            continue
        if segs and segs[-1][2] == d and segs[-1][0] + segs[-1][1] == i:
            segs[-1][1] += 1
        else:
            segs.append([i, 1, d])
    return [(gva + s, ova + s + d, n) for s, n, d in segs], len(gm)


# Code the engine decodes (GOG VA, length): the speech init functions (ig2-rules speech.rs) and Spy_New (lib.rs
# SPY_NEW_CODE). Their callees need no mapping: the call fixups name the GOG callee.
CODE = [(0x47f400, 0x37ee), (0x55e8b0, 0x2cfb), (0x5ad820, 0x1055), (0x58f830, 0x4c0)]


def code_map(g, o):
    """Each CODE range in the other build: segments, call-operand fixups (GOG operand VA, GOG target VA) and the
    (GOG, other) pairs of the globals its relocated operands name that lie outside the data map (BSS)."""
    gb, ob = g[0], o[0]
    def calls_masked(m):
        # for the search only: any E8 byte's next four bytes (a call's rel32 changes between builds)
        m = bytearray(m)
        for i in range(len(m) - 5):
            if m[i] == 0xe8:
                m[i + 1:i + 5] = b'ZZZZ'
        return bytes(m)
    _, gm = masked(g, '.text')
    tva, om = masked(o, '.text')
    gs, os_ = calls_masked(gm), calls_masked(om)
    gva = g[2]['.text'][0]
    segs, calls, pairs = [], [], []
    for start, n in CODE:
        a = gm[start - gva:start - gva + n]
        sa = gs[start - gva:start - gva + n]
        votes = defaultdict(int)
        for k in range(0, max(1, n - 24), 8):
            w = sa[k:k + 24]
            hit, seen = os_.find(w), 0
            while hit >= 0 and seen < 64:  # every hit votes; the true offset collects one per window
                votes[hit - k] += 1
                hit, seen = os_.find(w, hit + 1), seen + 1
        if not votes:
            sys.exit(f'code {start:#x} not found')
        at = max(votes, key=votes.get)
        b = om[at:at + n]
        other = tva + at
        segs.append((start, other, n))
        for i in range(n):
            if a[i] == b[i]:
                continue
            # a differing byte must be inside a call's rel32 operand
            # the earliest E8 whose rel32 covers byte i and aims into the code in both builds
            def aims(k):
                if i - k < 1 or gb[g[3](start + i - k) - 1] != 0xe8:
                    return False
                v, w = start + i - k, other + i - k
                tg = v + 4 + struct.unpack_from('<i', gb, g[3](v))[0]
                to = w + 4 + struct.unpack_from('<i', ob, o[3](w))[0]
                return g[2]['.text'][0] <= tg < g[2]['.text'][0] + g[2]['.text'][3] and o[2]['.text'][0] <= to < o[2]['.text'][0] + o[2]['.text'][3]
            op = next((i - k for k in range(3, -1, -1) if aims(k)), None)
            if op is None:
                sys.exit(f'code {start:#x}+{i:#x}: differs outside a call operand')
            v = start + op
            rel = struct.unpack_from('<i', gb, g[3](v))[0]
            if (v, v + 4 + rel) not in calls:
                calls.append((v, v + 4 + rel))
        for r in g[4]:
            if start <= r <= start + n - 4:
                pairs.append((struct.unpack_from('<I', gb, g[3](r))[0], struct.unpack_from('<I', ob, o[3](other + r - start))[0]))
        print(f'code {start:#x}: at {other:#x} ({votes[at]} anchors), {sum(1 for c in calls if start <= c[0] < start + n)} call fixups', file=sys.stderr)
    return segs, calls, sorted(set(pairs))


def main():
    gp, op, name = sys.argv[1:4]
    g, o = pe(gp), pe(op)
    code_segs, calls, pairs = code_map(g, o)
    table = []
    for sec in DATA:
        segs, size = align(g, o, sec)
        covered = sum(n for _, _, n in segs)
        print(f'{sec}: {len(segs)} segments, {covered}/{size} bytes mapped', file=sys.stderr)
        table += segs
    # one GOG place per other-build byte: longest runs first, shorter ones trimmed to the bytes still free
    kept = []
    for gv, ov, n in sorted(table, key=lambda s: -s[2]):
        pieces = [(ov, ov + n)]
        for a, b in ((k[1], k[1] + k[2]) for k in kept):
            pieces = [q for (s, e) in pieces for q in ((s, min(e, a)), (max(s, b), e)) if q[1] - q[0] > 0]
        kept += [(gv + s - ov, s, e - s) for s, e in pieces if e - s >= 4]
    table = sorted(kept + code_segs, key=lambda s: s[1])
    print(f'{len(table)} segments after removing overlaps, {sum(n for _, _, n in table)} bytes', file=sys.stderr)

    def to_gog(v):
        lo, hi = 0, len(table)
        while lo < hi:
            mid = (lo + hi) // 2
            gv, ov, n = table[mid]
            if v < ov:
                hi = mid
            elif v >= ov + n:
                lo = mid + 1
            else:
                return gv + v - ov
        return None
    # pointer check: every relocated dword of the other build in a mapped place
    gb, ob = g[0], o[0]
    good = bad = unmapped_target = 0
    for (gv, ov, n) in table:
        for r in o[4]:
            if ov <= r <= ov + n - 4:
                gr = gv + r - ov
                want = struct.unpack_from('<I', gb, g[3](gr))[0]
                have = to_gog(struct.unpack_from('<I', ob, o[3](r))[0])
                if have is None:
                    unmapped_target += 1
                elif have == want:
                    good += 1
                else:
                    bad += 1
    print(f'pointers: {good} agree, {bad} disagree, {unmapped_target} point outside the map (code)', file=sys.stderr)
    # plain bytes that differ inside mapped segments (real value changes)
    changed = []
    for (gv, ov, n) in table:
        a = gb[g[3](gv):g[3](gv) + n]
        c = ob[o[3](ov):o[3](ov) + n]
        changed += [gv + i for i in range(n) if a[i] != c[i] and not any(gv + i - k in g[4] for k in range(4))]
    print(f'non-pointer bytes that differ in mapped places: {len(changed)}', file=sys.stderr)
    if '--changed' in sys.argv:
        for v in changed:
            print(hex(v), file=sys.stderr)
    globals_ = [(gv, ov) for gv, ov in pairs if to_gog(ov) is None]
    print(f'{len(calls)} call fixups, {len(globals_)} globals outside the map: {[(hex(a), hex(c)) for a, c in globals_]}', file=sys.stderr)
    if '--rs' in sys.argv:
        out = sys.argv[sys.argv.index('--rs') + 1]
        with open(out, 'w', newline='\n') as f:
            f.write(f'// Generated by tools/exe_builds/align.py ({name}); addresses only. Spec: re/formats/exe-builds.md.\n')
            f.write(f'/// (GOG VA, {name} VA, length) of each matching run of .rdata/.data and of the decoded code, sorted by the {name} VA.\n')
            f.write(f'pub const SEGMENTS: [(u32, u32, u32); {len(table)}] = [\n')
            for gv, ov, n in table:
                f.write(f'    ({gv:#x}, {ov:#x}, {n:#x}),\n')
            f.write('];\n')
            f.write('/// Call operands in the code segments: (GOG VA of the rel32, GOG VA of the callee).\n')
            f.write(f'pub const CALLS: [(u32, u32); {len(calls)}] = [\n')
            for v, t in calls:
                f.write(f'    ({v:#x}, {t:#x}),\n')
            f.write('];\n')
            f.write(f'/// Globals outside the file-backed data that the code segments name: (GOG VA, {name} VA).\n')
            f.write(f'pub const GLOBALS: [(u32, u32); {len(globals_)}] = [\n')
            for gv, ov in globals_:
                f.write(f'    ({gv:#x}, {ov:#x}),\n')
            f.write('];\n')


main()
