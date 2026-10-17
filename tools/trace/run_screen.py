"""Reference screenshots of the original's screens (re/screens/*.md) for the client's --snapshot comparisons.

    run_screen.py <install> <save.sav> <out-prefix> [--kep 3 --planet 42] [--wait 240] [--shots 1] [--run]
                  [--cam X,Y,ZOOM,PITCH,ROTATE] [--slot 99] [--seconds 120] [--state-out STATE.sav]
                  [--fleets N] [--rop N] [--rop-fleet K] [--rop-menu X,Y] [--menu KIND@X,Y] [--mouse X,Y]
                  [--spies MODE] [--spy-frame F [--spy-closing]] [--verdict N --verdict-seed S --jsonl OUT]
                  [--info-press IDS]

Copies the save to the game's slot <slot>, spawns ig2.exe under screen.js (quick.scn, then `LoadSave`, clock paused
unless --run), opens screen <kep> (3 = the colony of <planet>; -1 stays on the loaded screen), waits <wait> frames
and grabs the game window's client area (PrintWindow) into <out-prefix>-<n>.png. --state-out also writes the exe's
own save of the grabbed moment (slot <slot>+1, last shot) to STATE.sav, for `ig2-client --snapshot`. On the starmap
(--kep 2) --rop presses one of its icons (the production view, the transfer panel; re/screens/starmap.md §12–§13),
after --fleets gave the human fleets beside --planet and --rop-fleet selected one. On the spy screen (--kep 7) --spies
stages a roster and a report, --spy-frame holds the opening (closing) at a frame, --verdict presses the verdict
buttons N times on fuzzed prisoners and writes what `Spy_Main` did to --jsonl (re/screens/spy.md §6.2, §9). On the info
screen (--kep 9) --info-press presses hot-spots in turn (a row's detail mode, the list arrows, the star log; the
state of the list, its globes, the detail mode and the star log go into STATE.sav.ui.json). The PNGs
show game imagery: write them under _local/ and never commit them.
"""
import argparse
import ctypes
import ctypes.wintypes as wt
import json
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
ap.add_argument("--debug", action="store_true", help="log the screen dispatcher's state every 120 frames")
ap.add_argument("--state-out", help="write the exe's own save of the grabbed moment (slot <slot>+1) to this file")
ap.add_argument("--fleets", type=int, default=0, help="first give the human N fleets beside --planet (ships + a vehicle entry)")
ap.add_argument("--rop", type=lambda v: int(v, 0), default=0, help="starmap: press icon N once (1 transfer, 2 production, 0xd split, 0xe fleet transfer)")
ap.add_argument("--rop-fleet", type=int, default=0, help="select the K-th made fleet before --rop")
ap.add_argument("--spies", choices=["roster", "report", "verdict", "applicant"], help="stage a spy roster (and a report) for the spy screen (--kep 7)")
ap.add_argument("--ai-empire", type=int, default=3, help="the AI empire --spies stages against")
ap.add_argument("--rop-menu", help="starmap: hold the radial icon menu open at the virtual point X,Y")
ap.add_argument("--menu", help="starmap: hold a select menu open: tax|autobuild|priority|sale:N|planets@X,Y")
ap.add_argument("--mouse", help="the pointer's virtual point X,Y while --rop-menu/--menu are held (the hover)")
ap.add_argument("--spy-frame", type=float, help="spy screen: hold spyframe at F (its opening; with --spy-closing its closing)")
ap.add_argument("--spy-closing", action="store_true")
ap.add_argument("--res-frame", type=float, help="research screen: hold resframe at F (its opening; with --res-closing its closing)")
ap.add_argument("--res-closing", action="store_true")
ap.add_argument("--res-keringeto", type=float, default=0.0, help="research screen (--res-frame): pin the side lights' clock reskeringeto at K")
ap.add_argument("--light-probe", action="store_true", help="research screen: record the engine's per-vertex lighting of the first lit frames to --jsonl")
ap.add_argument("--verdict", type=int, default=0, help="spy screen (--spies verdict): press the verdict buttons N times, fuzzed")
ap.add_argument("--verdict-seed", type=int, default=1)
ap.add_argument("--particles", help="starmap: write the exe's background particle sets (`FStars` 0xF16F38) of the grabbed moment to this JSON file")
ap.add_argument("--hide", choices=["clouds", "planet"], help="starmap long info: hide the planet's clouds, or the planet under them")
ap.add_argument("--jsonl", help="write the 'verdict' records here")
ap.add_argument("--info-starlog", type=int, default=0, help="info screen: stage N lines in the (unsaved) star log first")
ap.add_argument("--info-select", help="info screen: hold the planet-settings menu K (26 tax, 27 auto-build, 28 priority) open at X,Y on item I: K,X,Y,I")
ap.add_argument("--info-press", help="info screen (--kep 9): press these hot-spots in turn, e.g. 9 (a row's detail), 31,9, 15 (the star log)")
a = ap.parse_args()


def point(v):
    return [int(x) for x in v.split(",")] if v else None


def menu(v):
    if not v:
        return None
    kind, xy = v.split("@")
    kind, _, n = kind.partition(":")
    x, y = point(xy)
    return {"kind": kind, "n": int(n or 1), "x": x, "y": y}


data = os.path.join(os.environ["LOCALAPPDATA"], "GyroscopeGames", "ig2")
slot = os.path.join(data, f"save{a.slot}.sav")
aside = slot + ".screen-aside"
if os.path.exists(slot):
    shutil.move(slot, aside)
shutil.copyfile(a.save, slot)
state_slot = os.path.join(data, f"save{a.slot + 1}.sav") if a.state_out else None
if state_slot and os.path.exists(state_slot):
    shutil.move(state_slot, state_slot + ".screen-aside")

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
records = open(a.jsonl, "w", encoding="utf-8") if a.jsonl else None


def on(msg, _):
    if msg["type"] == "send":
        p = msg["payload"]
        if p.get("k") == "particles" and a.particles:
            with open(a.particles, "w", encoding="utf-8") as f:
                json.dump(p, f)
        elif p.get("k") in ("verdict", "light") and records:
            records.write(json.dumps(p) + "\n")
            records.flush()
        else:
            print(p, flush=True)
        if p.get("k") == "grab":
            grabs.append(p)
            # The UI clocks a state save doesn't hold, beside it for the client (`--clouds`).
            if a.state_out:
                with open(a.state_out + ".ui.json", "w", encoding="utf-8") as f:
                    json.dump({"clouds": p.get("clouds"), "futofeny": p.get("futofeny"), "acttime": p.get("acttime"), "info_turn": p.get("infoTurn"),
                               "info_list": p.get("infoList")}, f)
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
                                           "cam": [float(v) for v in a.cam.split(",")] if a.cam else None,
                                           "stateSlot": a.slot + 1 if a.state_out else 0, "debug": a.debug,
                                           "fleets": a.fleets, "rop": a.rop, "ropFleet": a.rop_fleet,
                                           "spies": a.spies, "aiEmpire": a.ai_empire,
                                           "ropMenu": point(a.rop_menu), "menu": menu(a.menu), "mouse": point(a.mouse),
                                           "spyFrame": a.spy_frame, "spyClosing": a.spy_closing,
                                           "resFrame": a.res_frame, "resClosing": a.res_closing,
                                           "resKeringeto": a.res_keringeto, "lightProbe": a.light_probe,
                                           "verdict": a.verdict, "verdictSeed": a.verdict_seed, "particles": bool(a.particles), "hide": a.hide,
                                           "infoStarlog": a.info_starlog, "infoPress": [int(x, 0) for x in a.info_press.split(",")] if a.info_press else [],
                                           "infoSelect": [int(x, 0) for x in a.info_select.split(",")] if a.info_select else None}})
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
    if state_slot:
        if os.path.exists(state_slot):
            shutil.move(state_slot, a.state_out)
        if os.path.exists(state_slot + ".screen-aside"):
            shutil.move(state_slot + ".screen-aside", state_slot)
sys.exit(0 if n >= a.shots else 1)
