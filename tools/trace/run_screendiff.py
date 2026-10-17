"""Capture comparison of one screen: ig2.exe vs ig2-client (ADR 0009 §4, re/screens/README.md §5).

    run_screendiff.py <install> <save.sav> <screen> [--out _local/screendiff] [--exe-png PNG] [--wait 240]
                      [--build] [--threshold T] [--shift N] [--mask X,Y,W,H]... [--no-default-masks]

<screen>: starmap | colony:<planet> | info | messages | research | production:<planet> | transfer:<planet> | diplomacy |
spy:<roster|report|verdict|applicant> (run_screen.py --spies stages the human's roster and one report). The tool
1. captures ig2.exe on <save> at that screen (tools/trace/run_screen.py: Frida loads the save with the exe's
   `LoadSave`, asks for the screen through `AktKepKell`, pauses the clock, writes the exe's own save of the moment
   (`<name>-state.sav`) and grabs the 800 × 600 client area), unless --exe-png gives an earlier capture;
2. renders `ig2-client --snapshot` of that state save and screen at 800 × 600 (`--ticks 0 --speed 1 --paused`: the save's own moment, the clock as
   screen.js leaves the exe's);
3. compares them with `ig2-screendiff` for the whole frame and the screen's regions (640 × 480 virtual pixels),
   writes `<out>/<name>-{exe,client,heat}.png` and `<out>/<name>.json`, and prints the table.
The message line is masked by default (it scrolls on the UI clock in both). Exit code 0 = every region matches.
Images hold game imagery: keep --out under _local/.
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
    "starmap": dict(kep=2, client=[], regions=[TOPBAR, "view=0,20,640,460"]),
    "colony": dict(kep=3, client=["--colony", "{planet}", "--colony-live"], regions=[
        TOPBAR, "planet_panel=0,18,223,153", "radar=491,348,149,132", "bottom_info=5,403,162,72",
        "buttons=170,440,320,36", "view=223,20,268,320"]),
    # The info screen opens over 150 UI frames and fades its texts in over 100 more: the grab waits longer.
    "info": dict(kep=9, wait=1500, client=["--screen", "info"], regions=[
        TOPBAR, "heads=20,40,600,90", "race_names=30,110,580,20", "triangle=230,30,180,110", "empire_panel=110,140,420,90",
        "list=0,245,290,235", "chart=388,272,224,180", "buttons=345,280,55,170"]),
    "messages": dict(kep=10, client=["--screen", "messages"], regions=[TOPBAR, "panel=0,160,190,300", "inbox=0,190,250,210"]),
    # The research screen (research.md): the ball and its models turn with the frames drawn, so "ball" compares
    # their place and size more than their pixels.
    "research": dict(kep=4, lang="hu", client=["--screen", "research", "--ui-frames", "240"], regions=[
        TOPBAR, "info_box=175,25,300,55", "start_double=100,20,440,62", "left_list=3,81,135,106",
        "right_list=500,81,135,106", "labs=0,235,80,60", "labs_right=560,235,80,60", "wheels=60,330,520,150",
        "ball=150,70,340,300"]),
    # The starmap's panels (starmap.md §12–§13): the exe presses the icon itself (run_screen.py --rop); the transfer
    # first gives the human two fleets beside the planet (fleets 10 and 9 in quick-1.sav, the exe's FlottaNew slots).
    "production": dict(kep=2, exe=["--rop", "2"], client=["--panel", "production"], regions=[
        TOPBAR, "name_plate=250,30,140,60", "left_panel=5,80,190,292", "right_panel=445,80,191,292", "exit=445,380,190,30"]),
    "transfer": dict(kep=2, exe=["--fleets", "2", "--rop", "0xe", "--rop-fleet", "1"], client=["--panel", "transfer:9,10"], regions=[
        TOPBAR, "left_panel=28,27,273,334", "right_panel=338,27,273,334", "scroll_bar=306,29,28,333", "exit=445,380,190,30"]),
    # The diplomacy screen (diplomacy.md): the exe opens with a 2.8 s animation, so it waits 1000 frames; the round
    # frame shows the leader's video and the cages turn (animated, judged loosely).
    "diplomacy": dict(kep=6, wait=1000, client=["--screen", "diplomacy"], regions=[
        TOPBAR, "info=455,30,172,100", "menu=21,349,155,101", "name=60,215,155,20", "ring=180,95,280,290",
        "left_heads=15,140,170,200", "right_heads=455,140,180,200"]),
    # The spy screen (spy.md): the exe waits 1200 frames for the opening and the carousel to settle, the client runs as
    # many UI frames. The head video and the names' blink run on the clocks, so `ring` and `carousel` vary.
    "spy": dict(kep=7, wait=1200, exe=["--spies", "{mode}"], client=["--screen", "spy", "--ui-frames", "1200"], regions=[
        TOPBAR, "carousel=0,20,640,215", "left_panel=6,248,172,217", "right_panel=464,248,172,217", "ring=196,226,248,254"]),
}
# The message line's text (messages.md §2: clipped to x 270..593, y 5..16).
DEFAULT_MASKS = ["268,3,328,15"]


def run(cmd, **kw):
    print("$", " ".join(cmd), flush=True)
    return subprocess.run(cmd, **kw)


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
    ap.add_argument("--lang", help="the client's language (IG2_LANG: en, de, fr, es, it, hu), as the exe runs")
    a = ap.parse_args()

    kind, _, arg = a.screen.partition(":")
    if kind not in SCREENS:
        sys.exit(f"unknown screen {kind}: {', '.join(SCREENS)}")
    cfg = SCREENS[kind]
    mode = arg if kind == "spy" else ""
    planet = int(arg) if arg and not mode else (71 if kind in ("production", "transfer") else 42)
    name = f"{kind}{planet}" if kind == "colony" else f"spy-{mode or 'roster'}" if kind == "spy" else kind
    mode = mode or "roster"
    os.makedirs(a.out, exist_ok=True)
    env = dict(os.environ, IG2_INSTALL_DIR=os.path.abspath(a.install))
    if a.lang:
        env["IG2_LANG"] = a.lang

    if a.build or not (os.path.exists(CLIENT) and os.path.exists(DIFF)):
        cargo = shutil.which("cargo") or os.path.expanduser("~/.cargo/bin/cargo")
        if run([cargo, "build", "--release", "-p", "ig2-client", "-p", "ig2-screendiff"], cwd=ENGINE).returncode:
            sys.exit("build failed")

    exe_png = os.path.join(a.out, f"{name}-exe.png")
    save = a.exe_state or a.save
    if a.exe_png:
        shutil.copyfile(a.exe_png, exe_png)
    else:
        prefix = os.path.join(a.out, f"{name}-exe-cap")
        state = os.path.join(a.out, f"{name}-state.sav")
        r = run([sys.executable, os.path.join(HERE, "run_screen.py"), a.install, a.save, prefix, "--kep", str(cfg["kep"]),
                 "--planet", str(planet), "--wait", str(a.wait or cfg.get("wait", 240)), "--shots", "1", "--state-out", state,
                 "--seconds", "400"] + [x.format(mode=mode) for x in cfg.get("exe", [])])
        if r.returncode or not os.path.exists(prefix + "-0.png"):
            sys.exit("the exe capture failed")
        shutil.move(prefix + "-0.png", exe_png)
        if not os.path.exists(state):
            sys.exit("the exe wrote no save of the captured moment")
        save = state

    client_png = os.path.join(a.out, f"{name}-client.png")
    args = [x.format(planet=planet) for x in cfg["client"]]
    # The exe runs in the install's language (Hungarian here); a screen with `lang` renders the client in it unless --lang says.
    if "lang" in cfg and not a.lang:
        env = dict(env, IG2_LANG=cfg["lang"])
    r = run([CLIENT, "--snapshot", client_png, "--save", save, "--size", "800x600", "--ticks", "0", "--speed", "1", "--paused"] + args, env=env)
    if r.returncode:
        sys.exit("the client snapshot failed")

    masks = ([] if a.no_default_masks else DEFAULT_MASKS) + a.mask
    cmd = [DIFF, exe_png, client_png, "--json", "--heat", os.path.join(a.out, f"{name}-heat.png"),
           "--threshold", a.threshold, "--shift", a.shift]
    for reg in cfg["regions"]:
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
