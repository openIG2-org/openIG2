"""Guard against game data in the repository (CONTRIBUTING.md -> Ground rules).

    python -I tools/check_tree.py [--base REV]

Lists tracked files (or, with --base, the files added or changed since REV) that have a game-asset or binary
extension or are larger than 1 MB, and exits 1 if there are any. The site's own screenshots are allowed.
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LIMIT = 1_000_000
ASSET = re.compile(r"\.(pvr|ogg|mp4|mp2|wav|xv|xvb|xvm|xvo|ani|scn|mou|map|res|dsc|eng|tut|raw|sav|ig2s|psd|blend|"
                   r"sgmodel|bmp|tga|dds|exe|dll|so|dylib|gpr|zip|7z|rar|bin)$", re.I)
ALLOWED = re.compile(r"^site/assets/")


def files(base: str | None) -> list[str]:
    if base:
        cmd = ["git", "-C", str(ROOT), "diff", "--name-only", "--diff-filter=AM", f"{base}...HEAD"]
    else:
        cmd = ["git", "-C", str(ROOT), "ls-files"]
    return [f for f in subprocess.run(cmd, check=True, capture_output=True, text=True).stdout.splitlines() if f]


def main() -> int:
    base = sys.argv[sys.argv.index("--base") + 1] if "--base" in sys.argv else None
    bad = []
    for f in files(base):
        p = ROOT / f
        if not p.is_file():
            continue
        if ASSET.search(f) and not ALLOWED.match(f):
            bad.append(f"{f}: asset or binary file type")
        elif p.stat().st_size > LIMIT:
            bad.append(f"{f}: {p.stat().st_size / 1e6:.1f} MB (limit 1 MB)")
    for b in bad:
        print(b)
    print(f"{len(bad)} problem(s)" if bad else "ok: no game data or large files")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
