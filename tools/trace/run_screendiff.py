"""Capture comparison of one screen: ig2.exe vs ig2-client (ADR 0009 §4, re/screens/README.md §5).

    run_screendiff.py <install> <save.sav> <screen> [--out _local/screendiff] [--exe-png PNG] [--wait 240]
                      [--build] [--threshold T] [--shift N] [--mask X,Y,W,H]... [--no-default-masks]

<screen>: starmap | colony:<planet> | info[:IDS] | messages | research | production:<planet> | transfer:<planet> | diplomacy |
spy:<roster|report|verdict|applicant> (run_screen.py --spies stages the human's roster and one report) |
(spy and research: --anim-frame F [--anim-closing] holds both at frame F of the opening/closing, --anim-mouse X,Y
rests the pointer there: a tooltip; --spy-* are the same) |
fleetinfo | tanksinfo | rop | planetinfo | tax | autobuild | priority | planets (the starmap's menus on planet 71) |
spacewar:<grab.json> (a battle-screen grab of `run_spwc.py --grab-ticks`: the exe's PNG beside the JSON, <save.sav> its
--state-out save; re/screens/spacewar.md §9). The tool
1. captures ig2.exe on <save> at that screen (tools/trace/run_screen.py: Frida loads the save with the exe's
   `LoadSave`, asks for the screen through `AktKepKell`, pauses the clock, writes the exe's own save of the moment
   (`<name>-state.sav`) and grabs the 800 × 600 client area), unless --exe-png gives an earlier capture;
2. renders `ig2-client --snapshot` of that state save and screen at 800 × 600 (`--ticks 0 --speed 1 --paused`: the save's own moment, the clock as
   screen.js leaves the exe's);
3. compares them with `ig2-screendiff` for the whole frame and the screen's regions (640 × 480 virtual pixels),
   writes `<out>/<name>-{exe,client,heat}.png` and `<out>/<name>.json`, and prints the table.
The message line is masked by default (it scrolls on the UI clock in both). Exit code 0 = every region matches.
Images hold game imagery: keep --out under _local/.

--script S (screens with `script` below; re/screens/sound.md §3) compares a frame sequence instead of one still: the
exe runs the input script S on the screen (run_screen.py --script --sounds: one UI tick a drawn frame, each grab read
back at exactly t ticks, the exe's save of the script's first frame), the client runs the same script on that save
(`--snapshot --script`), and every grab t is diffed: `<name>-{exe,client,heat}-t<T>.png`, `<name>-seq.json` (per grab
the regions, the exe's screen state; the sounds the exe started, each with the input before it) and a table of
mean differences, grabs × regions. --exe-seq PREFIX reuses an earlier exe run (PREFIX-t<T>.png, PREFIX-frames.json,
the state save beside it as <name>-state.sav).
"""
import argparse
import json
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
ENGINE = os.path.join(ROOT, "engine")
EXE_SUFFIX = ".exe" if os.name == "nt" else ""
# CARGO_TARGET_DIR points at a shared build (a branch checkout building into the main checkout's target).
TARGET = os.environ.get("CARGO_TARGET_DIR") or os.path.join(ENGINE, "target")
CLIENT = os.path.join(TARGET, "release", "ig2-client" + EXE_SUFFIX)
DIFF = os.path.join(TARGET, "release", "ig2-screendiff" + EXE_SUFFIX)

TOPBAR = "topbar=0,0,640,20"
# The regions each screen is judged by, in virtual pixels (re/screens/hud.md, info.md, messages.md).
SCREENS = {
    "starmap": dict(kep=2, script=True, client=[], regions=[TOPBAR, "view=0,20,640,460"]),
    "colony": dict(kep=3, script=True, client=["--colony", "{planet}", "--colony-live"], regions=[
        TOPBAR, "planet_panel=0,18,223,153", "radar=491,348,149,132", "bottom_info=5,403,162,72",
        "buttons=170,440,320,36", "view=223,20,268,320"]),
    # The colony screen in a real-time ground war (colony.md §11): the exe lands the human on the planet, places and
    # selects every unit (run_screen.py --grwar); the client seeds its selection from the saved flag 8.
    "grwar": dict(kep=3, exe=["--grwar"], client=["--colony", "{planet}", "--colony-live", "--colony-ui", "war"], regions=[
        TOPBAR, "texts=200,30,300,40", "radar=491,348,149,132", "bottom_info=5,403,162,72",
        "buttons=200,425,260,50", "view=223,20,268,320"]),
    # The info screen opens over 150 UI frames and fades its texts in over 100 more: the grab waits longer.
    "info": dict(kep=9, wait=1500, client=["--screen", "info"], regions=[
        TOPBAR, "heads=20,40,600,90", "race_names=30,110,580,20", "triangle=230,30,180,110", "empire_panel=110,140,420,90",
        "list=0,245,290,235", "chart=388,272,224,180", "buttons=345,280,55,170"]),
    "messages": dict(kep=10, client=["--screen", "messages"], regions=[TOPBAR, "panel=0,160,190,300", "inbox=0,190,250,210"]),
    # The research screen (research.md): the ball and its models turn with the frames drawn, so "ball" compares
    # their place and size more than their pixels. The exe opens it over ~250 frames after loading the save: the
    # grab waits 1000 (at 240 it is mid-opening; held frames, --anim-frame 60 included, don't wait for it).
    "research": dict(kep=4, wait=1000, script=True, client=["--screen", "research", "--ui-frames", "240"], regions=[
        TOPBAR, "info_box=175,25,300,55", "start_double=100,20,440,62", "left_list=3,81,135,106",
        "right_list=500,81,135,106", "labs=0,235,80,60", "labs_right=560,235,80,60", "wheels=60,330,520,150",
        "ball=150,70,340,300"]),
    # The main menu (mainscr.md): as the exe boots (run_screen.py --boot), no save; the galaxy clouds and the dust are
    # random particles (statistically alike), the rings and labels are exact. Script only.
    "main": dict(kep=15, script=True, exe=["--boot", "--run"], client=["--screen", "main"], regions=[
        "rings_top=150,40,340,140", "rings_side=60,180,520,130", "rings_bottom=150,300,340,140", "galaxy=110,150,300,250",
        "background=0,0,640,480"]),
    # The starmap's panels (starmap.md §12–§13): the exe presses the icon itself (run_screen.py --rop); the transfer
    # first gives the human two fleets beside the planet (fleets 10 and 9 in quick-1.sav, the exe's FlottaNew slots).
    "production": dict(kep=2, exe=["--rop", "2"], client=["--panel", "production"], regions=[
        TOPBAR, "name_plate=250,30,140,60", "left_panel=5,80,190,292", "right_panel=445,80,191,292", "exit=445,380,190,30"]),
    "transfer": dict(kep=2, exe=["--fleets", "2", "--rop", "0xe", "--rop-fleet", "1"], client=["--panel", "transfer:9,10"], regions=[
        TOPBAR, "left_panel=28,27,273,334", "right_panel=338,27,273,334", "scroll_bar=306,29,28,333", "exit=445,380,190,30"]),
    # The fleet's and the vehicle info panels (starmap.md §19–§20): the exe opens them through `AktRopIkon` 0xf (fleet mode,
    # the first of two new human fleets) and 9 (planet mode, three vehicle entries put on the planet first); the grab waits 420 frames.
    "fleetinfo": dict(kep=2, wait=420, exe=["--fleets", "2", "--rop", "0xf", "--rop-fleet", "1"], client=["--panel", "fleet"], regions=[
        TOPBAR, "list=28,27,273,334", "model=338,27,273,152", "data=338,186,273,175", "scroll_bar=306,29,28,333", "exit=445,380,190,30"]),
    "tanksinfo": dict(kep=2, wait=420, exe=["--planet-tanks", "3", "--rop", "9"], client=["--panel", "tanks"], regions=[
        TOPBAR, "list=28,27,273,334", "model=338,27,273,152", "data=338,186,273,175", "scroll_bar=306,29,28,333", "exit=445,380,190,30"]),
    # The diplomacy screen (diplomacy.md): the exe opens with a 2.8 s animation, so it waits 1000 frames; the round
    # frame shows the leader's video and the cages turn (animated, judged loosely).
    "diplomacy": dict(kep=6, wait=1000, client=["--screen", "diplomacy"], regions=[
        TOPBAR, "info=455,30,172,100", "menu=21,349,155,101", "name=60,215,155,20", "ring=180,95,280,290",
        "left_heads=15,140,170,200", "right_heads=455,140,180,200"]),
    # The spy screen (spy.md): the exe waits 1200 frames for the opening and the carousel to settle, the client runs as
    # many UI frames. The head video and the names' blink run on the clocks, so `ring` and `carousel` vary.
    "spy": dict(kep=7, wait=1200, exe=["--spies", "{mode}"], client=["--screen", "spy", "--ui-frames", "1200"], regions=[
        TOPBAR, "carousel=0,20,640,215", "left_panel=6,248,172,217", "right_panel=464,248,172,217", "ring=196,226,248,254"]),
    # The space battle (spacewar.md): the exe's grab comes from run_spwc.py --grab-ticks (a battle staged on quick.scn),
    # the client draws the dumped slot (--battle).
    "spacewar": dict(kep=11, client=["--screen", "spacewar", "--battle", "{grab}"], regions=[
        TOPBAR, "view=0,20,640,340", "console=210,360,220,120", "info_box=5,423,162,52", "radar=274,388,94,67",
        "weapons=595,20,45,440"]),
    # The radial menu, the planet's long info and its select menus, the production view's planet list (starmap.md
    # §14–§17): the exe stages them (run_screen.py --rop-menu/--menu/--mouse), the client draws the same state.
    "rop": dict(kep=2, exe=["--rop-menu", "320,240"], client=["--rop-menu", "320,240"], regions=[TOPBAR, "icons=240,175,135,135"]),
    "planetinfo": dict(kep=2, exe=["--rop", "0xf"], client=["--planet-info"], regions=[
        TOPBAR, "panel=319,33,302,326", "planet=0,40,300,330", "exit=445,380,190,30"]),
    "tax": dict(kep=2, exe=["--rop", "0xf", "--menu", "tax@422,277", "--mouse", "430,300"],
                client=["--menu", "tax@422,277", "--mouse", "430,300"], masks=["425,293,115,22"], regions=[TOPBAR, "panel=319,33,302,326", "menu=416,275,130,108"]),
    "autobuild": dict(kep=2, exe=["--rop", "0xf", "--menu", "autobuild@422,299", "--mouse", "430,335"],
                      client=["--menu", "autobuild@422,299", "--mouse", "430,335"], masks=["425,328,115,22"], regions=[TOPBAR, "panel=319,33,302,326", "menu=416,297,130,121"]),
    "priority": dict(kep=2, exe=["--rop", "0xf", "--menu", "priority@422,321", "--mouse", "430,330"],
                     client=["--menu", "priority@422,321", "--mouse", "430,330"], masks=["425,323,115,22"], regions=[TOPBAR, "panel=319,33,302,326", "menu=416,319,130,43"]),
    # The exe's staging leaves the list without a hovered planet (`smSelected` −1), so no --mouse here.
    "planets": dict(kep=2, exe=["--rop", "2", "--menu", "planets@300,90"],
                    client=["--panel", "production", "--menu", "planets@300,90"], regions=[TOPBAR, "menu=294,88,200,60"]),
}
# The message line's text (messages.md §2: clipped to x 270..593, y 5..16).
DEFAULT_MASKS = ["268,3,328,15"]


def run(cmd, **kw):
    print("$", " ".join(cmd), flush=True)
    return subprocess.run(cmd, **kw)


def diff(exe_png, client_png, heat, regions, masks, a):
    cmd = [DIFF, exe_png, client_png, "--json", "--heat", heat, "--threshold", a.threshold, "--shift", a.shift]
    for reg in regions:
        cmd += ["--region", reg]
    for m in masks:
        cmd += ["--mask", m]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode == 2:
        sys.exit(r.stderr)
    return json.loads(r.stdout)


def clock_args(save):
    """The exe's UI clocks a state save doesn't hold (run_screen.py writes them beside it)."""
    ui = save + ".ui.json"
    if not os.path.exists(ui):
        return []
    with open(ui, encoding="utf-8") as f:
        clocks = json.load(f)
    args = []
    if clocks.get("clouds") is not None:
        args += ["--clouds", str(clocks["clouds"])]
    if clocks.get("futofeny") is not None:
        args += ["--futofeny", repr(clocks["futofeny"])]
    if clocks.get("acttime") is not None:
        args += ["--acttime", repr(clocks["acttime"] / 1000)]
    if clocks.get("info_alap") is not None:
        args += ["--info-alap", repr(clocks["info_alap"])]
    return args


def sound_table(path):
    """(input → sound) pairs of a --sounds log: each sound the exe started on the script, with the input before it."""
    rows = []
    if not os.path.exists(path):
        return rows
    with open(path, encoding="utf-8") as f:
        for line in f:
            p = json.loads(line)
            if p.get("k") != "sound":
                continue
            i = p.get("input") or {}
            rows.append(dict(t=p.get("t"), ui=p.get("ui"), fn=p.get("fn"), table=p.get("table"), idx=p.get("idx"), name=p.get("name"),
                             loop=p.get("loop"), vol=p.get("vol"), on=p.get("on"),
                             input=None if i.get("src") != "script" else dict(t=i.get("t"), ev=i.get("ev"), at=i.get("at"), key=i.get("key")),
                             after=p.get("dui") if i.get("src") == "script" else None))
    return rows


def sequence(a, kind, cfg, env, masks, planet):
    stem = os.path.splitext(os.path.basename(a.script))[0]
    name = f"{kind}-seq-{stem}" if stem != kind else f"{kind}-seq"
    with open(a.script, encoding="utf-8") as f:
        grabs = sorted(json.load(f).get("grabs", []))
    state = os.path.join(a.out, f"{name}-state.sav")
    sounds = os.path.join(a.out, f"{name}-sounds.jsonl")
    prefix = a.exe_seq or os.path.join(a.out, f"{name}-exe")
    if not a.exe_seq:
        r = run([sys.executable, os.path.join(HERE, "run_screen.py"), a.install, a.save, prefix, "--kep", str(cfg["kep"]),
                 "--planet", str(planet), "--script", a.script, "--sounds", sounds, "--state-out", state, "--seconds", "560"] + cfg.get("exe", []))
        if r.returncode:
            sys.exit("the exe's script run failed")
    with open(prefix + "-frames.json", encoding="utf-8") as f:
        meta = json.load(f)
    by_t = {x["t"]: x for x in meta["frames"]}
    client = os.path.join(a.out, f"{name}-client.png")
    # The colony's `--colony-live` camera comes from the state save (the script's first frame).
    args = [x.format(planet=planet) for x in cfg["client"] if x not in ("--ui-frames", "240")] + ["--script", a.script]
    r = run([CLIENT, "--snapshot", client, "--save", state, "--size", "800x600", "--ticks", "0", "--speed", "1", "--paused"] + args + clock_args(state), env=env)
    if r.returncode:
        sys.exit("the client's script run failed")
    out = []
    for t in grabs:
        exe_png = f"{prefix}-t{t}.png"
        client_png = os.path.join(a.out, f"{name}-client-t{t}.png")
        if not (os.path.exists(exe_png) and os.path.exists(client_png)):
            out.append(dict(t=t, missing=True))
            continue
        rows = diff(exe_png, client_png, os.path.join(a.out, f"{name}-heat-t{t}.png"), cfg["regions"], masks, a)
        out.append(dict(t=t, exe=by_t.get(t, {}).get("res"), ticks=by_t.get(t, {}).get("ticks"), regions=rows))
    table = sound_table(sounds if not a.exe_seq else os.path.join(os.path.dirname(prefix), f"{name}-sounds.jsonl"))
    result = dict(screen=kind, script=os.path.basename(a.script), save=os.path.basename(a.save), masks=masks, threshold=float(a.threshold),
                  start=meta.get("start"), grabs=out, sounds=table)
    with open(os.path.join(a.out, f"{name}-seq.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, indent=1)
    regions = [r.split("=")[0] for r in cfg["regions"]]
    print(f"\n{kind} sequence {os.path.basename(a.script)} ({os.path.basename(a.save)}): best-offset mean per region, * = differs")
    print(f"{'t':>5} " + " ".join(f"{r[:11]:>11}" for r in ["frame"] + regions))
    for g in out:
        if g.get("missing"):
            print(f"{g['t']:>5} missing")
            continue
        cells = {x["region"]: x for x in g["regions"]}
        print(f"{g['t']:>5} " + " ".join(f"{cells[r]['mean']:10.2f}{'*' if cells[r]['verdict'] != 'match' else ' '}" for r in ["frame"] + regions if r in cells))
    print("\nsounds (exe): t, input before it → table[id] name")
    for s in table:
        i = s["input"]
        before = f"{i['ev']} {i.get('key') or i.get('at')} at {i['t']} (+{s['after']})" if i else "-"
        print(f"{s['t'] if s['t'] is not None else '-':>5}  {before:<34} {s['fn']} {s['table']}[{s['idx']}] {s['name']}")
    ok = all(not g.get("missing") and all(x["verdict"] == "match" for x in g["regions"]) for g in out)
    sys.exit(0 if ok else 1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("install")
    ap.add_argument("save")
    ap.add_argument("screen")
    ap.add_argument("--out", default=os.path.join(ROOT, "_local", "screendiff"))
    ap.add_argument("--exe-png", help="use this capture of ig2.exe instead of making one")
    ap.add_argument("--exe-state", help="with --exe-png: the exe's save of that moment (default: <save>)")
    ap.add_argument("--wait", type=int, help="frames the exe shows the screen before the grab (default: the screen's, else 240)")
    ap.add_argument("--build", action="store_true", help="cargo build --release the client and the comparator first")
    ap.add_argument("--threshold", default="6")
    ap.add_argument("--shift", default="2")
    ap.add_argument("--mask", action="append", default=[])
    ap.add_argument("--no-default-masks", action="store_true")
    ap.add_argument("--fleets", type=int, default=0, help="info: give the human N fleets first (run_screen.py --fleets)")
    ap.add_argument("--select", help="info: hold a planet-settings menu open in the exe (run_screen.py --info-select K,X,Y,I)")
    ap.add_argument("--starlog", type=int, default=0, help="info: stage N star log lines first (run_screen.py --info-starlog)")
    ap.add_argument("--lang", help="the client's language (IG2_LANG: en, de, fr, es, it, hu); default: the one ig2.exe picks (re/formats/locale.md)")
    ap.add_argument("--anim-frame", "--spy-frame", dest="anim_frame", type=float,
                    help="spy, research: hold the opening's frame (spyframe, resframe) at F in both (--anim-closing: the closing's)")
    ap.add_argument("--anim-closing", "--spy-closing", dest="anim_closing", action="store_true")
    ap.add_argument("--anim-mouse", "--spy-mouse", dest="anim_mouse",
                    help="spy, research: the pointer rests at the virtual point X,Y in both (a hot-spot's tooltip)")
    ap.add_argument("--arm", type=int, default=0, help="grwar: both call-in timers full and the air strike N armed (run_screen.py --grwar-arm)")
    ap.add_argument("--script", help="compare the frame sequence of this input script (re/screens/sound.md §3)")
    ap.add_argument("--exe-seq", help="with --script: reuse this exe run (its run_screen.py out-prefix)")
    a = ap.parse_args()

    kind, _, arg = a.screen.partition(":")
    if kind not in SCREENS:
        sys.exit(f"unknown screen {kind}: {', '.join(SCREENS)}")
    cfg = SCREENS[kind]
    mode = arg if kind in ("spy", "info") else ""
    grab = arg if kind == "spacewar" else ""
    if kind == "spacewar":
        if not grab:
            sys.exit("spacewar:<grab.json>: the dump of run_spwc.py --grab-ticks")
        a.exe_png = a.exe_png or os.path.splitext(grab)[0] + ".png"
        arg = ""
    planet = int(arg) if arg and not mode else (71 if kind in ("production", "transfer", "fleetinfo", "tanksinfo", "rop", "planetinfo", "tax", "autobuild", "priority", "planets") else 42)
    name = f"{kind}{planet}" if kind in ("colony", "grwar") else f"spy-{mode or 'roster'}" if kind == "spy" else kind
    if grab:
        name = "spacewar-" + os.path.splitext(os.path.basename(grab))[0]
    held = []
    held_exe = []
    # info:IDS (e.g. info:9, info:31+10, info:15): the exe presses these hot-spots first (re/screens/info.md §6.5–§6.8).
    if kind == "info" and mode:
        held_exe = ["--info-press", mode.replace("+", ",")]
        name = f"info-{mode.replace('+', '_')}"
    if kind == "info" and a.fleets:
        held_exe += ["--fleets", str(a.fleets)]
        name += f"-f{a.fleets}"
    if kind == "info" and a.select:
        held_exe += ["--info-select", a.select]
        name += "-m" + a.select.split(",")[0]
    if kind == "info" and a.starlog:
        held_exe += ["--info-starlog", str(a.starlog)]
        name += f"-s{a.starlog}"
    mode = mode or "roster"
    if kind == "grwar" and a.arm:
        held_exe += ["--grwar-arm", str(a.arm)]
        name += f"-arm{a.arm}"
    if kind in ("spy", "research") and a.anim_frame is not None:
        flag = "--spy" if kind == "spy" else "--res"
        held = ["--anim-frame", str(a.anim_frame)] + (["--anim-closing"] if a.anim_closing else [])
        held_exe = [flag + "-frame", str(a.anim_frame)] + ([flag + "-closing"] if a.anim_closing else [])
        name += f"-{'close' if a.anim_closing else 'open'}{a.anim_frame:g}"
    if kind in ("spy", "research") and a.anim_mouse:
        held += ["--mouse", a.anim_mouse]
        held_exe += ["--mouse", a.anim_mouse]
        name += "-mouse" + a.anim_mouse.replace(",", "_")
    os.makedirs(a.out, exist_ok=True)
    env = dict(os.environ, IG2_INSTALL_DIR=os.path.abspath(a.install))
    if a.lang:
        env["IG2_LANG"] = a.lang

    if a.build or not (os.path.exists(CLIENT) and os.path.exists(DIFF)):
        cargo = shutil.which("cargo") or os.path.expanduser("~/.cargo/bin/cargo")
        if run([cargo, "build", "--release", "-p", "ig2-client", "-p", "ig2-screendiff"], cwd=ENGINE).returncode:
            sys.exit("build failed")

    if a.script:
        if not cfg.get("script"):
            sys.exit(f"--script: the client runs no input scripts on {kind} yet")
        sequence(a, kind, cfg, env, ([] if a.no_default_masks else DEFAULT_MASKS) + cfg.get("masks", []) + a.mask, planet)

    exe_png =os.path.join(a.out, f"{name}-exe.png")
    save = a.exe_state or a.save
    if a.exe_png:
        if os.path.abspath(a.exe_png) != os.path.abspath(exe_png):
            shutil.copyfile(a.exe_png, exe_png)
    else:
        prefix = os.path.join(a.out, f"{name}-exe-cap")
        state = os.path.join(a.out, f"{name}-state.sav")
        r = run([sys.executable, os.path.join(HERE, "run_screen.py"), a.install, a.save, prefix, "--kep", str(cfg["kep"]),
                 "--planet", str(planet), "--wait", str(a.wait or cfg.get("wait", 240)), "--shots", "1", "--state-out", state,
                 "--seconds", "400"] + [x.format(mode=mode) for x in cfg.get("exe", [])] + held_exe)
        if r.returncode or not os.path.exists(prefix + "-0.png"):
            sys.exit("the exe capture failed")
        shutil.move(prefix + "-0.png", exe_png)
        if not os.path.exists(state):
            sys.exit("the exe wrote no save of the captured moment")
        save = state

    client_png = os.path.join(a.out, f"{name}-client.png")
    args = [x.format(planet=planet, grab=os.path.abspath(grab) if grab else "") for x in cfg["client"]] + held
    if kind == "grwar" and a.arm:
        args[args.index("war")] = f"war:{a.arm}"
    # The exe's starmap clocks at the grab (run_screen.py writes them beside the state save).
    ui = save + ".ui.json"
    if os.path.exists(ui):
        with open(ui, encoding="utf-8") as f:
            clocks = json.load(f)
        if clocks.get("clouds") is not None:
            args += ["--clouds", str(clocks["clouds"])]
        if clocks.get("futofeny") is not None:
            args += ["--futofeny", repr(clocks["futofeny"])]
        if clocks.get("acttime") is not None:
            args += ["--acttime", repr(clocks["acttime"] / 1000)]
        if clocks.get("info_alap") is not None:
            args += ["--info-alap", repr(clocks["info_alap"])]
        if clocks.get("info_turn") is not None and kind == "info":
            args += ["--info-turn", repr(clocks["info_turn"])]
        il = clocks.get("info_list")
        if il and kind == "info":
            args += ["--info-list", f"{il['pointer']},{il['step']},{il['counter']}",
                     "--info-globes", ";".join(",".join(repr(x) for x in g) for g in il["globes"])]
            # The detail mode and the star log (re/screens/info.md §6.6, §6.8); the log isn't saved: hand it over.
            dt = il.get("detail")
            if dt and dt["mode"]:
                if il.get("select") and il["select"][0]:
                    args += ["--info-select", ",".join(str(x) for x in il["select"])]
                args += ["--info-detail", f"{dt['entry']},{dt['slot']},{dt['leaving']},{dt['ptr']}",
                         "--info-texts", ",".join(str(x) for x in dt["texts"])]
            st = il.get("star")
            if st:
                if st["mode"] or st["frame"] > 45:
                    args += ["--info-star", repr(st["frame"])]
                log = save + ".starlog.txt"
                with open(log, "w", encoding="utf-8") as f:
                    f.write(f"{st['pointer']} {st['filled']}\n")
                    for c, t in st["log"]:
                        f.write(f"{c} {t}\n")
                args += ["--info-log", log]
    r = run([CLIENT, "--snapshot", client_png, "--save", save, "--size", "800x600", "--ticks", "0", "--speed", "1", "--paused"] + args, env=env)
    if r.returncode:
        sys.exit("the client snapshot failed")

    # The pointer and a hot-spot's tooltip the exe draws beside it (screens held under --mouse).
    masks = ([] if a.no_default_masks else DEFAULT_MASKS) + cfg.get("masks", []) + a.mask
    cmd = [DIFF, exe_png, client_png, "--json", "--heat", os.path.join(a.out, f"{name}-heat.png"),
           "--threshold", a.threshold, "--shift", a.shift]
    regions = list(cfg["regions"])
    if kind == "info" and a.select:
        # The open menu's box (re/screens/info.md §7.1): 108 / 13 per item + 4 / 43 high.
        k, x, y = (int(v, 0) for v in a.select.split(",")[:3])
        regions.append(f"select={x - 6},{y - 2},130,{ {26: 108, 27: 121, 28: 43}.get(k, 108) }")
    for reg in regions:
        cmd += ["--region", reg]
    for m in masks:
        cmd += ["--mask", m]
    r = run(cmd, capture_output=True, text=True)
    if r.returncode == 2:
        sys.exit(r.stderr)
    rows = json.loads(r.stdout)
    result = dict(screen=a.screen, save=os.path.basename(a.save), state=os.path.basename(save), masks=masks, threshold=float(a.threshold), regions=rows)
    with open(os.path.join(a.out, f"{name}.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, indent=1)
    print(f"\n{a.screen} ({os.path.basename(a.save)}), masks {masks}")
    print(f"{'region':<14} {'mean@0':>7} {'best':>8} {'mean':>7} {'>32':>7} {'luma exe/client':>16}  verdict")
    for x in rows:
        print(f"{x['region']:<14} {x['mean0']:7.2f} {'(%+d,%+d)' % (x['dx'], x['dy']):>8} {x['mean']:7.2f} {x['over32'] * 100:6.1f}% "
              f"{x['luma_exe']:7.1f}/{x['luma_client']:<7.1f}  {x['verdict']}")
    sys.exit(0 if all(x["verdict"] == "match" for x in rows) else 1)


if __name__ == "__main__":
    main()
