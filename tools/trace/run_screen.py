"""Reference screenshots of the original's screens (re/screens/*.md) for the client's --snapshot comparisons.

    run_screen.py <install> <save.sav> <out-prefix> [--kep 3 --planet 42] [--wait 240] [--shots 1] [--run]
                  [--cam X,Y,ZOOM,PITCH,ROTATE] [--slot 99] [--seconds 120]

Copies the save to the game's slot <slot>, spawns ig2.exe under screen.js (quick.scn, then `LoadSave`, clock paused
unless --run), opens screen <kep> (3 = the colony of <planet>; -1 stays on the loaded screen), waits <wait> frames
and grabs the game window's client area (PrintWindow) into <out-prefix>-<n>.png. The PNGs show game imagery: write
them under _local/ and never commit them.
"""
import argparse
import ctypes
import ctypes.wintypes as wt
import os
import shutil
import struct
import sys
import time
import zlib

import frida

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("save")
ap.add_argument("out")
ap.add_argument("--kep", type=int, default=3)
ap.add_argument("--planet", type=int, default=42)
ap.add_argument("--wait", type=int, default=240)
ap.add_argument("--shots", type=int, default=1)
ap.add_argument("--run", action="store_true")
ap.add_argument("--cam", help="pin the colony camera: X,Y,ZOOM,PITCH,ROTATE")
ap.add_argument("--slot", type=int, default=99)
ap.add_argument("--seconds", type=int, default=120)
a = ap.parse_args()

data = os.path.join(os.environ["LOCALAPPDATA"], "GyroscopeGames", "ig2")
slot = os.path.join(data, f"save{a.slot}.sav")
aside = slot + ".screen-aside"
if os.path.exists(slot):
    shutil.move(slot, aside)
shutil.copyfile(a.save, slot)

u32, g32 = ctypes.windll.user32, ctypes.windll.gdi32
u32.SetProcessDPIAware()


def window_of(pid):
    found = []

    @ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
    def cb(h, _):
        p = wt.DWORD()
        u32.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid and u32.IsWindowVisible(h):
            found.append(h)
        return True

    u32.EnumWindows(cb, 0)
    return found[0] if found else None


def png(path, w, h, bgra):
    rows = b"".join(b"\0" + bytes(x for px in range(w) for x in (bgra[(y * w + px) * 4 + 2], bgra[(y * w + px) * 4 + 1], bgra[(y * w + px) * 4])) for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b""))


def grab(hwnd, path):
    r = wt.RECT()
    u32.GetClientRect(hwnd, ctypes.byref(r))
    w, h = r.right, r.bottom
    hdc = u32.GetDC(hwnd)
    mem = g32.CreateCompatibleDC(hdc)
    bmp = g32.CreateCompatibleBitmap(hdc, w, h)
    g32.SelectObject(mem, bmp)
    u32.PrintWindow(hwnd, mem, 3)  # PW_CLIENTONLY | PW_RENDERFULLCONTENT
    class BIH(ctypes.Structure):
        _fields_ = [("biSize", wt.DWORD), ("biWidth", wt.LONG), ("biHeight", wt.LONG), ("biPlanes", wt.WORD), ("biBitCount", wt.WORD),
                    ("biCompression", wt.DWORD), ("biSizeImage", wt.DWORD), ("biXPelsPerMeter", wt.LONG), ("biYPelsPerMeter", wt.LONG),
                    ("biClrUsed", wt.DWORD), ("biClrImportant", wt.DWORD)]
    bih = BIH(ctypes.sizeof(BIH), w, -h, 1, 32, 0, 0, 0, 0, 0, 0)
    buf = ctypes.create_string_buffer(w * h * 4)
    g32.GetDIBits(mem, bmp, 0, h, buf, ctypes.byref(bih), 0)
    g32.DeleteObject(bmp)
    g32.DeleteDC(mem)
    u32.ReleaseDC(hwnd, hdc)
    png(path, w, h, buf.raw)
    return w, h


grabs = []
done = []


def on(msg, _):
    if msg["type"] == "send":
        p = msg["payload"]
        print(p, flush=True)
        if p.get("k") == "grab":
            grabs.append(p)
    else:
        print(msg, flush=True)
        done.append(1)


exe = os.path.join(a.install, "ig2.exe")
device = frida.get_local_device()
pid = device.spawn([exe], cwd=a.install)
session = device.attach(pid)
here = os.path.dirname(os.path.abspath(__file__))
script = session.create_script(open(os.path.join(here, "screen.js"), encoding="utf-8").read())
script.on("message", on)
script.load()
script.post({"type": "config", "payload": {"slot": a.slot, "kep": a.kep, "planet": a.planet, "wait": a.wait, "shots": a.shots, "run": a.run,
                                           "cam": [float(v) for v in a.cam.split(",")] if a.cam else None}})
device.resume(pid)
t0, n = time.time(), 0
WM_MOUSEMOVE = 0x0200
try:
    while time.time() - t0 < a.seconds and n < a.shots and not done:
        # The colony screen scrolls while the pointer sits at a window edge (`Colony_Autoscroll` reads SDL's
        # position): tell the window the pointer is in its middle, without moving the real cursor.
        hwnd = window_of(pid)
        if hwnd:
            r = wt.RECT()
            u32.GetClientRect(hwnd, ctypes.byref(r))
            u32.PostMessageW(hwnd, WM_MOUSEMOVE, 0, (r.bottom // 2) << 16 | (r.right // 2))
        if len(grabs) > n:
            hwnd = window_of(pid)
            if hwnd:
                w, h = grab(hwnd, f"{a.out}-{n}.png")
                print(f"grabbed {a.out}-{n}.png {w}x{h}", flush=True)
            n += 1
        time.sleep(0.05)
finally:
    try:
        device.kill(pid)
    except Exception:
        pass
    os.remove(slot)
    if os.path.exists(aside):
        shutil.move(aside, slot)
sys.exit(0 if n >= a.shots else 1)
