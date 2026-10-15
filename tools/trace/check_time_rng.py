"""Check a time/RNG trace (tools/trace/run_trace.py output) against re/rules/time-and-rng.md.

Usage: python -I tools/trace/check_time_rng.py <trace.jsonl> [<scenario.scn decoded, for RandSeed>]
Prints a Markdown summary. Exit code 1 on any mismatch.
"""
import collections
import json
import statistics
import sys

M = 0xFFFFFFFF
evs = [json.loads(l) for l in open(sys.argv[1], encoding="utf-8")]
fail = 0


def rndold_step(s):
    s = (s * 214013 + 2531011) & M
    return ((s << 21) | (s >> 11)) & M


def rndold_value(state, lo, hi):
    return lo if lo > hi else ((state >> 16) & 0x7FFF) % (hi - lo + 1) + lo


# --- RNG -----------------------------------------------------------------------------------
rnd = [e for e in evs if e["k"] == "rnd"]
bad_state = [e for e in rnd if e["before"] is not None and rndold_step(e["before"]) != e["after"]]
bad_value = [e for e in rnd if e["after"] is not None and rndold_value(e["after"], e["lo"], e["hi"]) != e["r"]]
per_stream = collections.Counter(e["s"] for e in rnd)
print("## RNG (`rndlog` → inlined `rndold`)")
print(f"- calls traced: {len(rnd)}; per stream: {dict(sorted(per_stream.items()))}")
print(f"- state transition mismatches: {len(bad_state)}")
print(f"- return value mismatches: {len(bad_value)}")
for e in (bad_state + bad_value)[:5]:
    print("  -", e)
fail |= bool(bad_state or bad_value)

# --- Scenario seeds ------------------------------------------------------------------------
loaded = next((e for e in evs if e["k"] == "loaded"), None)
if loaded:
    print("\n## Scenario load")
    print(f"- Ido after load: {loaded['ido']} (screen {loaded['kep']})")
    if len(sys.argv) > 2:
        line = next(l for l in open(sys.argv[2], encoding="latin-1") if l.startswith("RandSeed"))
        vals = [int(x) & M for x in line.split()[1:]]
        # The first rndlog calls after load may already have advanced some streams; compare
        # each stream's earliest 'before' state if present, else the 'loaded' snapshot.
        first_before = {}
        for e in rnd:
            if e["t"] >= 0 and e["s"] not in first_before:
                first_before[e["s"]] = e["before"]
        ok = mismatch = 0
        for i, v in enumerate(vals, start=1):
            seen = loaded["seeds"][i]
            if seen == v or first_before.get(i) == v:
                ok += 1
            else:
                # A stream may have been advanced during load: walk forward up to 10k steps.
                s, steps = v, 0
                while s != seen and steps < 10000:
                    s, steps = rndold_step(s), steps + 1
                if s == seen:
                    ok += 1
                else:
                    mismatch += 1
        print(f"- `.scn` RandSeed → RandSeed[1..=20]: {ok} streams consistent (equal, or reached by "
              f"stepping rndold during load), {mismatch} inconsistent; stream 0 after load = {loaded['seeds'][0]}")
        fail |= mismatch > 0

# --- Clock -----------------------------------------------------------------------------------
ticks = [e for e in evs if e["k"] == "tick"]
print("\n## Simulation clock (`IdoNapValtozott` = once per running sim tick)")
by_mult = collections.defaultdict(list)
for a, b in zip(ticks, ticks[1:]):
    if a["mult"] == b["mult"]:
        by_mult[a["mult"]].append((b["t"] - a["t"], b["ido"] - a["ido"]))
for mult, pairs in sorted(by_mult.items()):
    dts = [p[0] for p in pairs]
    steps = collections.Counter(p[1] for p in pairs)
    span = sum(dts) / 1000
    rate = len(dts) / span if span else 0
    print(f"- IdoSpeedSzorzo {mult:2d}: {len(pairs)} intervals, {rate:.2f} ticks/s, "
          f"median Δt {statistics.median(dts)} ms, Ido step histogram {dict(steps)}")
    if steps and set(steps) - {mult}:
        print(f"  ! Ido steps other than {mult}: {dict(steps)}")
        fail = 1
date_bad = [t for t in ticks if (t["ev"], t["honap"], t["nap"]) != (t["ido"] // 18000, (t["ido"] // 1500) % 12 + 1, (t["ido"] // 50) % 30 + 1)]
print(f"- calendar formula mismatches (Ev/Honap/Nap vs Ido): {len(date_bad)} of {len(ticks)}")
fail |= bool(date_bad)
speeds = [e for e in evs if e["k"] == "speed"]
print(f"- SetIdoSpeed(speed) → IdoSpeedSzorzo: {[(s['speed'], s['mult']) for s in speeds]}")
fail |= any({1: 1, 2: 4, 3: 16}[s["speed"]] != s["mult"] for s in speeds)
sys.exit(1 if fail else 0)
