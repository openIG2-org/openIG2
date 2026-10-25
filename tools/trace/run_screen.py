"""Reference screenshots of the original's screens (re/screens/*.md) for the client's --snapshot comparisons.

    run_screen.py <install> <save.sav> <out-prefix> [--kep 3 --planet 42] [--wait 240] [--shots 1] [--run]
                  [--cam X,Y,ZOOM,PITCH,ROTATE] [--slot 99] [--seconds 120] [--state-out STATE.sav]
                  [--fleets N] [--rop N] [--rop-fleet K] [--rop-menu X,Y] [--menu KIND@X,Y] [--mouse X,Y]
                  [--spies MODE] [--spy-frame F [--spy-closing]] [--verdict N --verdict-seed S --jsonl OUT]
                  [--info-press IDS] [--script SCRIPT.json] [--sounds OUT.jsonl]

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
show game imagery: write them under _local/ and never commit them. --size WxH runs the game in a window of that size
(it rewrites width=/height= in the install's ig2.settings for the run and puts the file back; re/screens/widescreen.md).
--script runs an input script on screen <kep> (re/screens/sound.md §3: the exe's clock moves 10 ms a drawn frame, one
UI tick a frame; events at UI tick t; each grab read back in the game at exactly t ticks) and writes
<out-prefix>-t<T>.png per grab, <out-prefix>-frames.json (each grab's tick, the screen's state) and with --state-out
the exe's save of the script's first frame. --sounds writes every sound the exe starts and every input event to
OUT.jsonl (sound.js; the tables from sound_tables.py).
"""
import argparse
import atexit
import ctypes
import ctypes.wintypes as wt
import json
import os
import re
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
ap.add_argument("--planet-tanks", type=int, default=0, help="starmap: first put N vehicle entries of the human on --planet")
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
ap.add_argument("--dip-frame", type=float, help="diplomacy screen: hold dipframe at F (its opening)")
ap.add_argument("--res-keringeto", type=float, default=0.0, help="research screen (--res-frame): pin the side lights' clock reskeringeto at K")
ap.add_argument("--trade-cheat", action="store_true", help="trade screen: set the exe's TradeCheat (every own planet is a trade port)")
ap.add_argument("--light-probe", action="store_true", help="research (--kep 4), trade (8), starmap (2) or info (9) screen: record the engine's per-vertex lighting (fenyekszamolasa) to --jsonl")
ap.add_argument("--verdict", type=int, default=0, help="spy screen (--spies verdict): press the verdict buttons N times, fuzzed")
ap.add_argument("--verdict-seed", type=int, default=1)
ap.add_argument("--particles", help="starmap: write the exe's background particle sets (`FStars` 0xF16F38) of the grabbed moment to this JSON file")
ap.add_argument("--hide", choices=["clouds", "planet"], help="starmap long info: hide the planet's clouds, or the planet under them")
ap.add_argument("--jsonl", help="write the 'verdict' records here")
ap.add_argument("--info-starlog", type=int, default=0, help="info screen: stage N lines in the (unsaved) star log first")
ap.add_argument("--info-select", help="info screen: hold the planet-settings menu K (26 tax, 27 auto-build, 28 priority) open at X,Y on item I: K,X,Y,I")
ap.add_argument("--size", help="window size WxH (ig2.settings width/height for this run; re/screens/widescreen.md)")
ap.add_argument("--via-starmap", type=int, default=0, metavar="F",
                help="open the starmap first and --kep F frames later (the overlays' SaveScreen pictures, re/screens/overlay-background.md)")
ap.add_argument("--script", help="an input script on the screen --kep (JSON: events, grabs; re/screens/sound.md §3)")
ap.add_argument("--exe-args", nargs="*", default=[], help="command-line arguments for ig2.exe (e.g. demomode: re/screens/mainscr.md §9.5)")
ap.add_argument("--boot", action="store_true", help="the main menu as the exe starts: no scenario, no save (use with --kep 15 --run)")
ap.add_argument("--probe", help="append this Frida script to the agent (a screen's own probe, e.g. probes/mainscr.js; its records go to stdout)")
ap.add_argument("--tutorial", type=int, default=1, help="with --probe probes/tutorial.js: the chapter its TutorialStart runs; its 'tutwin' records go to <out>-tutwin.jsonl")
ap.add_argument("--sounds", help="write the sounds the exe starts and the input events to this JSONL (sound.js)")
ap.add_argument("--grwar", action="store_true", help="colony screen: the human lands on --planet in real time, every unit placed and selected (colony.md §11)")
ap.add_argument("--grwar-arm", type=int, default=0, help="with --grwar: the air-strike timers full and SpecWeaponMode set to N (1 fighters, 2 big ships)")
ap.add_argument("--colony-info", nargs="?", const="on", help="colony screen: the planet info mode, with KIND (tax | autobuild | priority) its menu held open (colony.md §12)")
ap.add_argument("--self-destruct", type=int, default=0, help="colony screen: the planet's self-destruct countdown (WF+0x334) set to N (its window, colony.md §11.8)")
ap.add_argument("--grwar-land", action="store_true", help="with --grwar: no unit placed (still on board: the landing zone and texts, colony.md §11.3)")
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
sounds = open(a.sounds, "w", encoding="utf-8") if a.sounds else None
script = None
if a.script:
    with open(a.script, encoding="utf-8") as f:
        script = json.load(f)
frames_meta = {"start": None, "frames": [], "done": None}


def png_rgba_bottom_up(path, w, h, rgba):
    # glReadPixels' rows, bottom first, RGBA.
    rows = b"".join(b"\0" + b"".join(rgba[((h - 1 - y) * w + px) * 4:((h - 1 - y) * w + px) * 4 + 3] for px in range(w)) for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b""))


def on(msg, data):
    if msg["type"] == "send":
        p = msg["payload"]
        if p.get("k") in ("sound", "input", "music", "track", "fade", "msg"):
            if sounds:
                sounds.write(json.dumps(p) + "\n")
                sounds.flush()
            if p.get("k") == "sound":
                print(f"sound {p.get('fn')} {p.get('table')}[{p.get('idx')}] {p.get('name')} t={p.get('t')} ui={p.get('ui')}", flush=True)
            if p.get("k") == "msg":
                print(f"msg 0x{p['type']:x} from {p['from']} t={p['t']} {p['fields']}", flush=True)
            if p.get("k") in ("music", "track"):
                print(f"{p['k']} {p.get('id', p.get('name'))} ui={p.get('ui')} kep={p.get('kep')} eppen={p.get('eppen')} regi={p.get('regi')} hangero={p.get('hangero')}", flush=True)
            return
        if p.get("k") in ("tutwin", "tutstart"):
            with open(f"{a.out}-tutwin.jsonl", "a", encoding="utf-8") as f:
                f.write(json.dumps(p) + "\n")
            return
        if p.get("k") == "frame":
            path = f"{a.out}-t{p['t']}.png"
            png_rgba_bottom_up(path, p["w"], p["h"], data)
            frames_meta["frames"].append(dict(p, png=os.path.basename(path)))
            print(f"frame t={p['t']} ticks={p['ticks']} {p['w']}x{p['h']} res={p['res']} -> {path}", flush=True)
            return
        if p.get("k") == "scriptStart":
            frames_meta["start"] = p
            if a.state_out:
                with open(a.state_out + ".ui.json", "w", encoding="utf-8") as f:
                    json.dump({"clouds": p.get("clouds"), "futofeny": p.get("futofeny"), "acttime": p.get("acttime"), "info_turn": p.get("infoTurn"), "info_alap": p.get("infoAlap"),
                               "res": p.get("res"), "trade_eta": p.get("tradeEta"), "dkm": p.get("dkm"), "faders": p.get("faders")}, f)
        if p.get("k") == "scriptDone":
            frames_meta["done"] = p
        if p.get("k") == "particles" and a.particles:
            with open(a.particles, "w", encoding="utf-8") as f:
                json.dump(p, f)
        elif p.get("k") in ("verdict", "light", "resline") and records:
            records.write(json.dumps(p) + "\n")
            records.flush()
        else:
            print(p, flush=True)
        if p.get("k") == "grab":
            grabs.append(p)
            # The UI clocks a state save doesn't hold, beside it for the client (`--clouds`).
            if a.state_out:
                with open(a.state_out + ".ui.json", "w", encoding="utf-8") as f:
                    json.dump({"clouds": p.get("clouds"), "futofeny": p.get("futofeny"), "acttime": p.get("acttime"), "info_turn": p.get("infoTurn"), "info_alap": p.get("infoAlap"),
                               "info_list": p.get("infoList"), "faders": p.get("faders"), "rop_frames": p.get("ropFrames")}, f)
    else:
        print(msg, flush=True)
        done.append(1)


if a.size:
    # ig2_resize lays the screen out from the window ig2.settings asks for: set it for this run only.
    settings = os.path.join(a.install, "ig2.settings")
    orig = open(settings, "rb").read()
    w, h = a.size.lower().split("x")
    s = re.sub(rb"(?m)^width=\d+", b"width=" + w.encode(), orig)
    open(settings, "wb").write(re.sub(rb"(?m)^height=\d+", b"height=" + h.encode(), s))
    atexit.register(lambda: open(settings, "wb").write(orig))
exe = os.path.join(a.install, "ig2.exe")
device = frida.get_local_device()
pid = device.spawn([exe] + a.exe_args, cwd=a.install)
session = device.attach(pid)
here = os.path.dirname(os.path.abspath(__file__))
source = open(os.path.join(here, "screen.js"), encoding="utf-8").read() + "\n" + open(os.path.join(here, "sound.js"), encoding="utf-8").read()
if a.probe:
    source += "\n" + open(a.probe, encoding="utf-8").read().replace("TUT_CHAPTER", str(a.tutorial))
agent = session.create_script(source)
agent.on("message", on)
agent.load()
sound_tables = None
if a.sounds:
    sys.path.insert(0, here)
    import sound_tables
    sound_tables = sound_tables.find(a.install)
agent.post({"type": "config", "payload": {"slot": a.slot, "kep": a.kep, "planet": a.planet, "wait": a.wait, "viaStarmap": a.via_starmap, "shots": a.shots, "run": a.run, "boot": a.boot,
                                           "cam": [float(v) for v in a.cam.split(",")] if a.cam else None,
                                           "stateSlot": a.slot + 1 if a.state_out else 0, "debug": a.debug,
                                           "fleets": a.fleets, "planetTanks": a.planet_tanks, "rop": a.rop, "ropFleet": a.rop_fleet,
                                           "spies": a.spies, "aiEmpire": a.ai_empire,
                                           "ropMenu": point(a.rop_menu), "menu": menu(a.menu), "mouse": point(a.mouse),
                                           "spyFrame": a.spy_frame, "spyClosing": a.spy_closing,
                                           "resFrame": a.res_frame, "resClosing": a.res_closing, "dipFrame": a.dip_frame,
                                           "resKeringeto": a.res_keringeto, "lightProbe": a.light_probe, "tradeCheat": a.trade_cheat,
                                           "verdict": a.verdict, "verdictSeed": a.verdict_seed, "particles": bool(a.particles), "hide": a.hide,
                                           "infoStarlog": a.info_starlog, "infoPress": [int(x, 0) for x in a.info_press.split(",")] if a.info_press else [],
                                           "infoSelect": [int(x, 0) for x in a.info_select.split(",")] if a.info_select else None,
                                           "grwar": a.grwar, "grwarArm": a.grwar_arm, "grwarLand": a.grwar_land, "selfDestruct": a.self_destruct, "colonyInfo": a.colony_info,
                                           "script": script, "soundTables": sound_tables}})
device.resume(pid)
t0, n = time.time(), 0
WM_MOUSEMOVE = 0x0200
try:
    while time.time() - t0 < a.seconds and (frames_meta["done"] is None if script else n < a.shots) and not done:
        if script:
            # No pointer messages: the script owns the pointer.
            time.sleep(0.05)
            continue
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
    # A script that starts a game (the main menu's campaign) may have removed the slot already.
    if os.path.exists(slot):
        os.remove(slot)
    if os.path.exists(aside):
        shutil.move(aside, slot)
    if state_slot:
        if os.path.exists(state_slot):
            shutil.move(state_slot, a.state_out)
        if os.path.exists(state_slot + ".screen-aside"):
            shutil.move(state_slot + ".screen-aside", state_slot)
if script:
    with open(f"{a.out}-frames.json", "w", encoding="utf-8") as f:
        json.dump(frames_meta, f, indent=1)
    want = len(script.get("grabs", []))
    print(f"script: {len(frames_meta['frames'])}/{want} frames, done {frames_meta['done']}", flush=True)
    sys.exit(0 if frames_meta["done"] and len(frames_meta["frames"]) == want else 1)
sys.exit(0 if n >= a.shots else 1)
