"""The Windows release bundle (docs/BUILDING.md -> Release bundle): the binaries built from this checkout, a readme,
nothing else (no game data; the engine reads the player's own install).

    python -I tools/release/build_windows.py [--out DIR] [--keep-target]

Version from `tools/release/version.py` (MAJOR.MINOR.N), passed to the build as `OPENIG2_VERSION`. Builds into `_local/release-target` (its own target: the release links the C++ runtime
statically, which would rebuild the shared target), checks that every binary imports only Windows' own DLLs,
stages `_local/release/openig2-<version>-windows-x64/` and zips it beside. The release workflow (.github/workflows/release.yml)
runs it and publishes the zip.
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
# The binaries a player gets (`ig2-check` once it exists).
BINS = [("ig2-client", "ig2-client"), ("ig2-check", "ig2-check")]
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=ROOT / "_local" / "release")
    ap.add_argument("--keep-target", action="store_true", help="keep _local/release-target (else removed: disk)")
    a = ap.parse_args()
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from version import version as release_version
    version = os.environ.get("OPENIG2_VERSION") or release_version()
    target = ROOT / "_local" / "release-target"
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target)
    env["OPENIG2_VERSION"] = version
    # The gnu toolchain's C++ (openh264) and C runtimes linked in, so the exe needs no MinGW DLLs; libstdc++.a from
    # the g++ that cc builds openh264 with.
    stdcpp = Path(subprocess.run(["g++", "-print-file-name=libstdc++.a"], check=True, capture_output=True, text=True).stdout.strip())
    if not stdcpp.is_file():
        print(f"no static libstdc++ ({stdcpp}): install a MinGW-w64 g++ with static libraries", file=sys.stderr)
        return 1
    env["CXXSTDLIB"] = "static=stdc++"
    env.pop("RUSTFLAGS", None)
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(["-C", "target-feature=+crt-static", "-L", f"native={stdcpp.resolve().parent}"])
    have = [(pkg, name) for pkg, name in BINS if (ROOT / "engine" / "crates" / pkg).is_dir()]
    cmd = ["cargo", "build", "--release", "-j", "4", "--manifest-path", str(ROOT / "engine" / "Cargo.toml")]
    for pkg, _ in have:
        cmd += ["-p", pkg]
    run(cmd, env)

    name = f"openig2-{version}-windows-x64"
    stage = a.out / name
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    bad = []
    for _, binary in have:
        exe = target / "release" / f"{binary}.exe"
        foreign = [d for d in imports(exe) if not SYSTEM_DLL.match(d)]
        if foreign:
            bad.append(f"{exe.name}: {', '.join(foreign)}")
        shutil.copy2(exe, stage / exe.name)
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
    if not a.keep_target:
        shutil.rmtree(target, ignore_errors=True)
    files = ", ".join(f.name for f in sorted(stage.iterdir()))
    print(f"{zip_path} ({zip_path.stat().st_size / 1e6:.1f} MB): {files}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
