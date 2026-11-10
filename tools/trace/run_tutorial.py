"""Record the tutorial interpreter in the GOG ig2.exe (tutorial.js) as JSON lines.

usage: run_tutorial.py <install> <out.jsonl> [--chapters 1,2] [--seconds 560] [--start-after F]
                       [--force-after T] [--battle-after T] [--next-after F] [--debug]

Starts the chapters one after another the way the main menu's tutorial choice does (Mode_Tutorial_Start,
TutorialStartMelyik) and records every TutorialFrissit tick, command and event. An event the script waits for
longer than --force-after UI ticks is fired by the tracer (the player's action); WAITFORBATTLE is cleared
after --battle-after ticks. Run it in the foreground (a background shell stalls the game).
Spec: re/rules/tutorial.md.
"""
import argparse

from frida_run import Tracer, agent_source

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("out")
ap.add_argument("--chapters", default="1")
ap.add_argument("--seconds", type=int, default=560)
ap.add_argument("--start-after", type=int, default=1200)
ap.add_argument("--force-after", type=int, default=300)
ap.add_argument("--battle-after", type=int, default=600)
ap.add_argument("--next-after", type=int, default=400)
ap.add_argument("--debug", action="store_true")
a = ap.parse_args()


def on_event(ev):
    if ev["k"] in ("start", "battle", "done") or (ev["k"] == "ev" and ev.get("forced")):
        print("agent:", ev)


t = Tracer(a.install, agent_source("tutorial.js"), a.out, on_event=on_event, v8=True,
           banner=f"spawned ig2.exe pid {{pid}}; chapters {a.chapters}, up to {a.seconds}s",
           config={"startAfterFrames": a.start_after, "forceAfter": a.force_after,
                   "battleAfter": a.battle_after, "nextAfterFrames": a.next_after,
                   "chapters": [int(x) for x in a.chapters.split(",") if x],
                   "debug": a.debug})
try:
    t.start()
    t.wait(a.seconds)
finally:
    t.stop()
    print(f"events: {t.counts} -> {a.out}")
