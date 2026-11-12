"""The texture pack tool's learned upscaler for the release bundles (ADR 0054 §8): Real-ESRGAN's published
`realesr-general-x4v3.pth` (BSD-3-Clause), downloaded once from its GitHub release, checked by SHA-256, and put with its
licence into the bundle's `models/`. Called by build_windows.py and build_unix.py.
"""
import hashlib
import shutil
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NAME = "realesr-general-x4v3.pth"
URL = f"https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/{NAME}"
SHA256 = "8dc7edb9ac80ccdc30c3a5dca6616509367f05fbc184ad95b731f05bece96292"
CACHE = ROOT / "_local" / "models"


def fetch() -> Path:
    path = CACHE / NAME
    if not (path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == SHA256):
        CACHE.mkdir(parents=True, exist_ok=True)
        print(f"+ download {URL}", flush=True)
        with urllib.request.urlopen(URL, timeout=120) as r:
            data = r.read()
        got = hashlib.sha256(data).hexdigest()
        if got != SHA256:
            raise SystemExit(f"{NAME}: SHA-256 {got}, expected {SHA256}")
        path.write_bytes(data)
    return path


def add_to(stage: Path) -> None:
    models = stage / "models"
    models.mkdir(exist_ok=True)
    shutil.copy2(fetch(), models / NAME)
    shutil.copy2(ROOT / "tools" / "release" / "LICENSE-Real-ESRGAN.txt", models / "LICENSE-Real-ESRGAN.txt")
