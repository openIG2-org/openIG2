"""Summarise Frida trace files (jsonl) instead of reading them into the conversation.

Every tracer writes one JSON object per line with its kind in `k`. This prints, per file: the size, the record
count, unparsable lines (a cut-off last line = the run was killed), the counts per kind (and per `from` where a
record has one), the `seq`/`ido`/`tick` range, the last records (where a run stopped) and every record whose
kind looks like a failure (`fault`, `error`, `crash`, `death`, `exception`, `timeout`, `reset`, `win`, `stop`),
each cut to one line.

  summarize.py FILE_OR_DIR... [--glob 'story*.jsonl'] [--show KIND [-n 3]] [--width 160]

`--show KIND` also prints the first N records of that kind (cut to --width).
"""
import argparse, collections, json, pathlib, re

ALERT = re.compile(r"fault|error|crash|death|die|exception|timeout|reset|win|stop|abort|desync|mismatch|stall|hang", re.I)
RANGE_KEYS = ("seq", "ido", "Ido", "ido2", "tick", "t", "frame")


def short(obj, width):
    s = json.dumps(obj, separators=(",", ":"), ensure_ascii=False)
    return s if len(s) <= width else s[:width - 3] + "..."


def summarize(path, a):
    kinds, froms = collections.Counter(), collections.Counter()
    bad, n = [], 0
    ranges = {}
    tail = collections.deque(maxlen=a.tail)
    alerts, shown = [], []
    with open(path, encoding="utf-8", errors="replace") as f:
        for ln, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                d = json.loads(line)
            except json.JSONDecodeError:
                bad.append(ln)
                continue
            n += 1
            if not isinstance(d, dict):
                kinds["<non-object>"] += 1
                continue
            k = str(d.get("k", d.get("kind", d.get("type", "?"))))
            kinds[k] += 1
            if "from" in d:
                froms[f"{k}<-{d['from']}"] += 1
            for rk in RANGE_KEYS:
                v = d.get(rk)
                if isinstance(v, (int, float)):
                    lo, hi = ranges.get(rk, (v, v))
                    ranges[rk] = (min(lo, v), max(hi, v))
            tail.append((ln, d))
            if ALERT.search(k) and len(alerts) < a.max_alerts:
                alerts.append((ln, d))
            if a.show and k == a.show and len(shown) < a.n:
                shown.append((ln, d))
    print(f"== {path}  {path.stat().st_size:,} B  {n} records" + (f"  {len(bad)} bad lines (first {bad[0]}, last {bad[-1]})" if bad else ""))
    print("   kinds: " + ", ".join(f"{k} {c}" for k, c in kinds.most_common()))
    if froms:
        print("   from:  " + ", ".join(f"{k} {c}" for k, c in froms.most_common(12)))
    if ranges:
        print("   range: " + ", ".join(f"{k} {lo}..{hi}" for k, (lo, hi) in ranges.items()))
    for ln, d in alerts:
        print(f"   ! line {ln}: {short(d, a.width)}")
    for ln, d in shown:
        print(f"   show line {ln}: {short(d, a.width)}")
    for ln, d in tail:
        print(f"   end line {ln}: {short(d, a.width)}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+", type=pathlib.Path)
    ap.add_argument("--glob", default="*.jsonl")
    ap.add_argument("--show")
    ap.add_argument("-n", type=int, default=3)
    ap.add_argument("--tail", type=int, default=2)
    ap.add_argument("--width", type=int, default=160)
    ap.add_argument("--max-alerts", type=int, default=10)
    a = ap.parse_args()
    for p in a.paths:
        files = sorted(p.glob(a.glob)) if p.is_dir() else [p]
        for f in files:
            summarize(f, a)


if __name__ == "__main__":
    main()
