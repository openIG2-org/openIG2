"""The Linux and macOS release bundles (docs/BUILDING.md -> Release bundle): the binaries built from this checkout, a readme,
nothing else (no game data; the engine reads the player's own install).

    python3 -I tools/release/build_unix.py [--out DIR]

Version from `tools/release/version.py` (MAJOR.MINOR.N), passed to the build as `OPENIG2_VERSION`. Builds into the
workspace's own target (nothing is linked differently from a normal release build), lists the shared libraries each
binary needs, stages `_local/release/openig2-<version>-<os>-<arch>/` (linux-x64, macos-arm64, macos-x64) and packs it
beside as a .tar.gz. The macOS binaries are not signed or notarized. CI runs it
(.github/workflows/ci.yml), the release workflow publishes that archive.
"""
import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# (package, the name the archive gives the binary)
BINS = [("ig2-client", "openIG2"), ("ig2-check", "openIG2-check")]


def run(cmd, env=None):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    subprocess.run(cmd, check=True, env=env)


MACOS = sys.platform == "darwin"


def needed(exe: Path) -> list[str]:
    if MACOS:
        out = subprocess.run(["otool", "-L", str(exe)], check=True, capture_output=True, text=True).stdout
        return [l.split()[0] for l in out.splitlines()[1:] if l.strip()]
    out = subprocess.run(["objdump", "-p", str(exe)], check=True, capture_output=True, text=True).stdout
    return re.findall(r"NEEDED\s+(\S+)", out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=ROOT / "_local" / "release")
    a = ap.parse_args()
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from version import version as release_version
    version = os.environ.get("OPENIG2_VERSION") or release_version()
    env = dict(os.environ, OPENIG2_VERSION=version)
    manifest = ROOT / "engine" / "Cargo.toml"
    cmd = ["cargo", "build", "--release", "--locked", "-j", "4", "--manifest-path", str(manifest)]
    for b, _ in BINS:
        cmd += ["-p", b]
    run(cmd, env)
    target = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "engine" / "target")

    arch = "arm64" if platform.machine() in ("arm64", "aarch64") else "x64"
    name = f"openig2-{version}-{'macos' if MACOS else 'linux'}-{arch}"
    stage = a.out / name
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    for b, name_out in BINS:
        exe = target / "release" / b
        print(f"{b} needs: {', '.join(needed(exe))}")
        shutil.copy2(exe, stage / name_out)
    readme = (ROOT / "tools" / "release" / "README.txt").read_text().replace("{version}", version)
    (stage / "README.txt").write_text(readme)
    (stage / "VERSION.txt").write_text(version + "\n")

    archive = a.out / f"{name}.tar.gz"
    with tarfile.open(archive, "w:gz") as t:
        t.add(stage, name)
    files = ", ".join(f.name for f in sorted(stage.iterdir()))
    print(f"{archive} ({archive.stat().st_size / 1e6:.1f} MB): {files}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
