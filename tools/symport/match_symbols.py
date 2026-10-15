"""Match functions of the unstripped ELF `ig2` onto the shipped PE `ig2.exe`.

Inputs are produced by tools/ghidra/ExportFunctionFeatures.java (one JSONL per program).
Strategy (each match carries the method that produced it):
  1. string anchors: a string referenced by exactly one function on each side votes for that pair;
  2. constant anchors: same, with distinctive immediates (>= 0x10000);
  3. call-graph gaps: inside matched pairs, align the ordered callee lists on already-matched
     callees; a gap with exactly one unmatched callee on each side is matched;
  4. the same over caller lists.
Conflicts (one side wanting two partners) are dropped, never guessed.

Usage: python -I match_symbols.py <features_ig2.jsonl> <features_ig2.exe.jsonl> <out.tsv>
"""
import collections
import json
import sys

SKIP_NAME = ("std::", "__gnu_cxx", "__cxx", "_GLOBAL__", "__static_initialization", "FUN_", "thunk_", "_init", "_fini",
             "operator", "__libc_csu", "deregister_tm_clones", "register_tm_clones", "frame_dummy")


def load(path):
    funcs = {}
    for line in open(path, encoding="utf-8"):
        f = json.loads(line)
        f["strings"] = set(f["strings"])
        f["consts_all"] = set(f["consts"])
        f["consts"] = set(c for c in f["consts"] if c >= 0x10000)
        f["data_seq"] = list(f.get("data", []))
        f["data"] = set(f["data_seq"])
        f["ext"] = set(c for c in f["callees"] if c.startswith("ext:"))
        funcs[f["entry"]] = f
    return funcs


def interesting_elf(f):
    n = f["name"]
    return not any(s in n for s in SKIP_NAME) and f["source"] != "DEFAULT"


def unique_index(funcs, key, keep=lambda f: True):
    idx = collections.defaultdict(set)
    for e, f in funcs.items():
        if keep(f):
            for v in f[key]:
                idx[v].add(e)
    return {v: next(iter(s)) for v, s in idx.items() if len(s) == 1}


class Matcher:
    def __init__(self, elf, exe):
        self.elf, self.exe = elf, exe
        self.e2x, self.x2e, self.how = {}, {}, {}
        self.gmap = {}
        self.callers_x = collections.defaultdict(list)
        self.callers_e = collections.defaultdict(list)
        for funcs, callers in ((elf, self.callers_e), (exe, self.callers_x)):
            for e, f in funcs.items():
                for c in f["callees"]:
                    if c in funcs:
                        callers[c].append(e)

    def add(self, e, x, how):
        if e in self.e2x or x in self.x2e:
            return False
        self.e2x[e], self.x2e[x], self.how[e] = x, e, how
        return True

    def anchors(self, key, how, min_votes):
        ue = unique_index(self.elf, key, interesting_elf)
        ux = unique_index(self.exe, key)
        votes = collections.Counter()
        for v, e in ue.items():
            if v in ux:
                votes[(e, ux[v])] += 1
        best_e = collections.defaultdict(list)
        best_x = collections.defaultdict(list)
        for (e, x), n in votes.items():
            best_e[e].append((n, x))
            best_x[x].append((n, e))
        added = 0
        for (e, x), n in votes.items():
            if n < min_votes:
                continue
            # Only accept if this pair is the unique top choice on both sides.
            top_e = sorted(best_e[e], reverse=True)
            top_x = sorted(best_x[x], reverse=True)
            if top_e[0][1] != x or (len(top_e) > 1 and top_e[1][0] == top_e[0][0]):
                continue
            if top_x[0][1] != e or (len(top_x) > 1 and top_x[1][0] == top_x[0][0]):
                continue
            if self.compatible(e, x):
                added += self.add(e, x, f"{how}:{n}")
        return added

    def gaps(self, lists_e, lists_x, how):
        added = 0
        for e, x in list(self.e2x.items()):
            le = [c for c in lists_e(e) if c in self.elf and (c in self.e2x or interesting_elf(self.elf[c]))]
            lx = [c for c in lists_x(x) if c in self.exe]
            # Anchor positions: callees already matched to each other, in increasing order on both sides.
            pos_x = {c: i for i, c in enumerate(lx)}
            anchors = [(-1, -1)]
            for i, c in enumerate(le):
                j = pos_x.get(self.e2x.get(c))
                if j is not None and j > anchors[-1][1]:
                    anchors.append((i, j))
            anchors.append((len(le), len(lx)))
            for (i0, j0), (i1, j1) in zip(anchors, anchors[1:]):
                ge = [c for c in le[i0 + 1:i1] if c not in self.e2x]
                gx = [c for c in lx[j0 + 1:j1] if c not in self.x2e]
                if len(ge) == 1 and len(gx) == 1 and self.compatible(ge[0], gx[0]):
                    added += self.add(ge[0], gx[0], how)
        return added

    def compatible(self, e, x):
        fe, fx = self.elf[e], self.exe[x]
        # Shared strings or constants must not contradict: if both have strings, require overlap.
        if fe["strings"] and fx["strings"] and not (fe["strings"] & fx["strings"]):
            return False
        shared_strings = fe["strings"] & fx["strings"]
        if fe["consts"] and fx["consts"] and not (fe["consts"] & fx["consts"]) and not shared_strings:
            return False
        # -O0 code is larger than MSVC /O2, unless MSVC inlined a lot into it. Reject wild ratios.
        ratio = fx["size"] / max(fe["size"], 1)
        return 0.05 <= ratio <= 4.0

    def map_globals(self):
        """Vote ELF→exe global correspondences from matched pairs."""
        votes = collections.Counter()
        for e, x in self.e2x.items():
            ge, gx = self.elf[e]["data"], self.exe[x]["data"]
            if not ge or not gx or len(ge) * len(gx) > 2500:
                continue
            for a in ge:
                for b in gx:
                    votes[(a, b)] += 1
            # Same number of globals touched: first-use order usually survives optimisation.
            se, sx = self.elf[e]["data_seq"], self.exe[x]["data_seq"]
            if len(se) == len(sx) <= 12:
                for a, b in zip(se, sx):
                    votes[(a, b)] += 2
        best = {}
        for (a, b), v in votes.items():
            best.setdefault(a, []).append((v, b))
        rev = collections.defaultdict(list)
        g = {}
        for a, lst in best.items():
            lst.sort(reverse=True)
            if lst[0][0] >= 2 and (len(lst) == 1 or lst[0][0] >= 2 * lst[1][0]):
                g[a] = lst[0][1]
                rev[lst[0][1]].append(a)
        # Drop exe globals claimed by several ELF globals.
        self.gmap = {a: b for a, b in g.items() if len(rev[b]) == 1}
        return len(self.gmap)

    def tokens_e(self, e):
        f = self.elf[e]
        t = {("s", v) for v in f["strings"]} | {("c", v) for v in f["consts_all"] if v >= 0x400}
        t |= {("g", self.gmap[d]) for d in f["data"] if d in self.gmap}
        t |= {("f", self.e2x[c]) for c in f["callees"] if c in self.e2x}
        t |= {("x", c) for c in f["ext"]}
        return t

    def tokens_x(self, x):
        f = self.exe[x]
        t = {("s", v) for v in f["strings"]} | {("c", v) for v in f["consts_all"] if v >= 0x400}
        t |= {("g", d) for d in f["data"]}
        t |= {("f", c) for c in f["callees"] if c in self.x2e}
        t |= {("x", c) for c in f["ext"]}
        return t

    def global_anchors(self):
        """Mapped globals referenced by exactly one function on each side act like unique strings."""
        ue = collections.defaultdict(set)
        ux = collections.defaultdict(set)
        for e, f in self.elf.items():
            if interesting_elf(f):
                for d in f["data"]:
                    if d in self.gmap:
                        ue[self.gmap[d]].add(e)
        for x, f in self.exe.items():
            for d in f["data"]:
                ux[d].add(x)
        votes = collections.Counter()
        for g, es in ue.items():
            if len(es) == 1 and len(ux.get(g, ())) == 1:
                votes[(next(iter(es)), next(iter(ux[g])))] += 1
        added = 0
        for (e, x), n in votes.items():
            if self.compatible(e, x):
                added += self.add(e, x, f"glob:{n}")
        return added

    def neighbourhood(self, min_score=0.5, margin=1.5):
        """Score unmatched candidates found through matched neighbours (callers and callees)."""
        cand = collections.defaultdict(set)
        for e, x in self.e2x.items():
            for c in self.elf[e]["callees"]:
                if c in self.elf and c not in self.e2x and interesting_elf(self.elf[c]):
                    cand[c] |= {cx for cx in self.exe[x]["callees"] if cx in self.exe and cx not in self.x2e}
            for c in self.callers_e[e]:
                if c not in self.e2x and interesting_elf(self.elf[c]):
                    cand[c] |= {cx for cx in self.callers_x[x] if cx not in self.x2e}
        proposals = {}
        for e, xs in cand.items():
            te = self.tokens_e(e)
            if len(te) < 2:
                continue
            scored = []
            for x in xs:
                tx = self.tokens_x(x)
                inter = len(te & tx)
                if inter < 2:
                    continue
                scored.append((inter / len(te | tx), inter, x))
            if not scored:
                continue
            scored.sort(reverse=True)
            best = scored[0]
            if best[0] >= min_score and (len(scored) == 1 or best[0] >= margin * scored[1][0]):
                proposals[e] = best
        # Resolve exe-side collisions: keep only exe functions proposed once.
        by_x = collections.Counter(p[2] for p in proposals.values())
        added = 0
        for e, (score, inter, x) in proposals.items():
            if by_x[x] == 1 and self.compatible(e, x):
                added += self.add(e, x, f"sim:{score:.2f}")
        return added

    def run(self):
        log = []
        log.append(("strings>=2", self.anchors("strings", "str", 2)))
        log.append(("strings>=1", self.anchors("strings", "str", 1)))
        log.append(("consts>=1", self.anchors("consts", "const", 1)))
        for rnd in range(30):
            a = self.gaps(lambda e: self.elf[e]["callees"], lambda x: self.exe[x]["callees"], "callee")
            b = self.gaps(lambda e: self.callers_e[e], lambda x: self.callers_x[x], "caller")
            g = self.map_globals()
            c = self.global_anchors()
            d = self.neighbourhood()
            log.append((f"round {rnd} (globals {g})", a + b + c + d))
            if a + b + c + d == 0:
                break
        return log


def main():
    elf, exe = load(sys.argv[1]), load(sys.argv[2])
    m = Matcher(elf, exe)
    for step, n in m.run():
        print(f"{step:20s} +{n}")
    game = [e for e, f in elf.items() if interesting_elf(f)]
    print(f"matched {len(m.e2x)} / {len(game)} interesting ELF functions; exe has {len(exe)}")
    by_how = collections.Counter(h.split(":")[0] for h in m.how.values())
    print("by method:", dict(by_how))
    with open(sys.argv[3], "w", encoding="utf-8") as out:
        out.write("exe_entry\telf_entry\tname\tmethod\texe_size\telf_size\n")
        for e, x in sorted(m.e2x.items(), key=lambda kv: kv[1]):
            out.write(f"{x}\t{e}\t{elf[e]['name']}\t{m.how[e]}\t{exe[x]['size']}\t{elf[e]['size']}\n")


if __name__ == "__main__":
    main()
