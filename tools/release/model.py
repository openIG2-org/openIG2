"""The texture pack tool's learned upscalers for the release bundles (ADR 0054 §8, ADR 0089 §3): Real-ESRGAN's
published `RealESRGAN_x4plus.pth` (the quality model) and `realesr-general-x4v3.pth` (the compact one, `--fast`), both
BSD-3-Clause, each downloaded once from its GitHub release, checked by SHA-256, and put with their licence into the
bundle's `models/`. Called by build_windows.py and build_unix.py.
"""
import hashlib
import shutil
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RELEASES = "https://github.com/xinntao/Real-ESRGAN/releases/download"
# (file, release tag, SHA-256)
MODELS = [
    ("RealESRGAN_x4plus.pth", "v0.1.0", "4fa0d38905f75ac06eb49a7951b426670021be3018265fd191d2125df9d682f1"),
    ("realesr-general-x4v3.pth", "v0.2.5.0", "8dc7edb9ac80ccdc30c3a5dca6616509367f05fbc184ad95b731f05bece96292"),
]
CACHE = ROOT / "_local" / "models"


def fetch(name: str, tag: str, sha256: str) -> Path:
    path = CACHE / name
    if not (path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == sha256):
        CACHE.mkdir(parents=True, exist_ok=True)
        url = f"{RELEASES}/{tag}/{name}"
        print(f"+ download {url}", flush=True)
        with urllib.request.urlopen(url, timeout=300) as r:
            data = r.read()
        got = hashlib.sha256(data).hexdigest()
        if got != sha256:
            raise SystemExit(f"{name}: SHA-256 {got}, expected {sha256}")
        path.write_bytes(data)
    return path


def add_to(stage: Path) -> None:
    models = stage / "models"
    models.mkdir(exist_ok=True)
    for name, tag, sha256 in MODELS:
        shutil.copy2(fetch(name, tag, sha256), models / name)
    shutil.copy2(ROOT / "tools" / "release" / "LICENSE-Real-ESRGAN.txt", models / "LICENSE-Real-ESRGAN.txt")
