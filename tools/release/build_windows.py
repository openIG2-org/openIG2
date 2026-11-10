"""The Windows release bundle (docs/BUILDING.md -> Release bundle): the binaries built from this checkout, a readme,
nothing else (no game data; the engine reads the player's own install).

    python -I tools/release/build_windows.py [--out DIR] [--keep-target] [--target DIR] [--env]

Version from `tools/release/version.py` (MAJOR.MINOR.N), passed to the build as `OPENIG2_VERSION`. Builds into `_local/release-target` (its own target: the release links the C++ runtime
statically, which would rebuild the shared target), checks that every binary imports only Windows' own DLLs,
stages `_local/release/openig2-<version>-windows-x64/` and zips it beside. The release workflow (.github/workflows/release.yml)
publishes the zip CI builds with it: there `--env` prints the build's settings for the whole job and `--target` builds
in the job's own target, so the tests and the zip share one build.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# The binaries a player gets: (package, the name the archive gives the exe).
BINS = [("ig2-client", "openIG2"), ("ig2-check", "openIG2-check")]
# DLLs a stock Windows 10/11 has. Anything else (libstdc++-6, libgcc_s, libwinpthread) means a runtime would be
# missing on the player's machine.
SYSTEM_DLL = re.compile(r"^(api-ms-win-.*|advapi32|bcrypt|bcryptprimitives|d3d\w*|dxgi|dwmapi|gdi32|imm32|kernel32|"
                        r"ntdll|ole32|oleaut32|opengl32|propsys|shell32|user32|userenv|uxtheme|ws2_32|winmm|"
                        r"version|setupapi|hid|cfgmgr32|dbghelp|msvcrt|ucrtbase|comctl32|crypt32|secur32|iphlpapi|"
                        r"ncrypt|mswsock|powrprof|shlwapi|avrt|mmdevapi|ksuser)\.dll$", re.I)


def run(cmd, env=None):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    subprocess.run(cmd, check=True, env=env)


def imports(exe: Path) -> list[str]:
    out = subprocess.run(["objdump", "-p", str(exe)], check=True, capture_output=True, text=True).stdout
    return sorted({m.group(1) for m in re.finditer(r"DLL Name: (\S+)", out)}, key=str.lower)


def release_env(version: str) -> dict[str, str] | None:
    """The build's settings: the version, and the gnu toolchain's C++ (openh264) and C runtimes linked in, so the
    exe needs no MinGW DLLs; libstdc++.a from the g++ that cc builds openh264 with."""
    stdcpp = Path(subprocess.run(["g++", "-print-file-name=libstdc++.a"], check=True, capture_output=True, text=True).stdout.strip())
    if not stdcpp.is_file():
        print(f"no static libstdc++ ({stdcpp}): install a MinGW-w64 g++ with static libraries", file=sys.stderr)
        return None
    return {
        "OPENIG2_VERSION": version,
        "CXXSTDLIB": "static=stdc++",
        "CARGO_ENCODED_RUSTFLAGS": "\x1f".join(["-C", "target-feature=+crt-static", "-L", f"native={stdcpp.resolve().parent}"]),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=ROOT / "_local" / "release")
    ap.add_argument("--keep-target", action="store_true", help="keep _local/release-target (else removed: disk)")
    ap.add_argument("--target", type=Path, help="build in this target folder and keep it (CI: the job's own)")
    ap.add_argument("--env", action="store_true", help="print the build's settings as NAME=value lines and stop")
    a = ap.parse_args()
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from version import version as release_version
    version = os.environ.get("OPENIG2_VERSION") or release_version()
    settings = release_env(version)
    if settings is None:
        return 1
    if a.env:
        # Bytes with plain line feeds: this goes into a CI environment file.
        sys.stdout.buffer.write("".join(f"{k}={v}\n" for k, v in settings.items()).encode())
        return 0
    target = (a.target or ROOT / "_local" / "release-target").resolve()
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target)
    env.pop("RUSTFLAGS", None)
    env.update(settings)
    have = [(pkg, name) for pkg, name in BINS if (ROOT / "engine" / "crates" / pkg).is_dir()]
    cmd = ["cargo", "build", "--release", "--locked", "-j", "4", "--manifest-path", str(ROOT / "engine" / "Cargo.toml")]
    for pkg, _ in have:
        cmd += ["-p", pkg]
    run(cmd, env)

    name = f"openig2-{version}-windows-x64"
    stage = a.out / name
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    bad = []
    for pkg, binary in have:
        exe = target / "release" / f"{pkg}.exe"
        foreign = [d for d in imports(exe) if not SYSTEM_DLL.match(d)]
        if foreign:
            bad.append(f"{exe.name}: {', '.join(foreign)}")
        shutil.copy2(exe, stage / f"{binary}.exe")
    if bad:
        print("non-system DLL imports (the player would lack them):\n  " + "\n  ".join(bad), file=sys.stderr)
        return 1
    readme = (ROOT / "tools" / "release" / "README.txt").read_text().replace("{version}", version)
    (stage / "README.txt").write_text(readme.replace("\n", "\r\n"), newline="")
    (stage / "VERSION.txt").write_text(version + "\r\n", newline="")

    zip_path = a.out / f"{name}.zip"
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for f in sorted(stage.iterdir()):
            z.write(f, f"{name}/{f.name}")
    if not a.keep_target and not a.target:
        shutil.rmtree(target, ignore_errors=True)
    files = ", ".join(f.name for f in sorted(stage.iterdir()))
    print(f"{zip_path} ({zip_path.stat().st_size / 1e6:.1f} MB): {files}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
