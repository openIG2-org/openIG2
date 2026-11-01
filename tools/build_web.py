"""Builds the browser client (ADR 0047) into web/pkg: `python -I tools/build_web.py [--debug]`.

Needs the wasm32-unknown-unknown target and wasm-bindgen-cli at the version of the wasm-bindgen crate in
engine/Cargo.lock. Serve web/ on localhost only to try it (docs/BUILDING.md).
"""
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ENGINE = ROOT / "engine"
OUT = ROOT / "web" / "pkg"


def main():
    debug = "--debug" in sys.argv[1:]
    lock = (ENGINE / "Cargo.lock").read_text(encoding="utf-8").replace("\r\n", "\n")
    want = re.search(r'name = "wasm-bindgen"\nversion = "([^"]+)"', lock).group(1)
    try:
        have = subprocess.run(["wasm-bindgen", "--version"], capture_output=True, text=True).stdout.split()[-1:]
    except FileNotFoundError:
        have = []
    if have != [want]:
        sys.exit(f"wasm-bindgen {want} needed (found {have or 'none'}): cargo install wasm-bindgen-cli --version {want} --locked")
    profile = [] if debug else ["--release"]
    subprocess.run(["cargo", "build", "--target", "wasm32-unknown-unknown", "-p", "ig2-client", *profile], cwd=ENGINE, check=True)
    wasm = ENGINE / "target" / "wasm32-unknown-unknown" / ("debug" if debug else "release") / "ig2-client.wasm"
    shutil.rmtree(OUT, ignore_errors=True)
    subprocess.run(["wasm-bindgen", "--target", "web", "--no-typescript", "--out-name", "ig2_client", "--out-dir", str(OUT), str(wasm)], check=True)
    print(f"built {OUT}; try it with: python -m http.server 8047 --bind 127.0.0.1 --directory web")


if __name__ == "__main__":
    main()
