"""The release version, MAJOR.MINOR.N (docs/VERSIONING.md).

    python -I tools/release/version.py

`VERSION` holds MAJOR.MINOR; N is the number of commits on main's first-parent line since the commit that last
changed `VERSION` (0 for that commit itself). Needs the full history (CI: `fetch-depth: 0`).
"""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def git(*args: str) -> str:
    return subprocess.run(["git", "-C", str(ROOT), *args], check=True, capture_output=True, text=True).stdout.strip()


def version() -> str:
    base = (ROOT / "VERSION").read_text().strip()
    if base.count(".") != 1 or not all(p.isdigit() for p in base.split(".")):
        raise SystemExit(f"VERSION must be MAJOR.MINOR, not {base!r}")
    since = git("log", "-1", "--format=%H", "--", "VERSION")
    n = git("rev-list", "--count", "--first-parent", f"{since}..HEAD") if since else "0"
    return f"{base}.{n}"


if __name__ == "__main__":
    print(version())
    sys.exit(0)
