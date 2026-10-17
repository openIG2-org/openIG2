"""Spawn the user's ig2.exe under Frida, load an original save and record a save at every sim tick boundary
(tools/trace/replay.js), for the port's save replay (engine/crates/ig2-sim/tests/save_replay.rs).

Usage (project venv): _local/venv/Scripts/python.exe -u tools/trace/run_replay.py <install-dir> <save> <out-dir>
          [--ticks 40] [--speed 3] [--seed 4242] [--slot 99] [--seconds 400] [--stage] [--traders] [--spies] [--merges] [--wormholes] [--battles [--battles-mode space|ground|land]] [--deaths [--deaths-mode base|campaign]] [--slot-base 100] [--pre-slot 98]
The input save is copied to the game's data directory (%LOCALAPPDATA%/GyroscopeGames/ig2) as save<slot>.sav and
loaded with the exe's `LoadSave(slot, name, 1)`; the game then writes save<100+k>.sav per tick, which this script
moves to <out-dir>/<k>.sav, with <out-dir>/replay.jsonl (one line per save: tick, seed, non-saved globals). Files
the game had in those slots are moved aside and put back afterwards.
--stage exercises the trade market (re/rules/trade.md): replay.js edits the loaded state (a trade port, an early
story merchant, old offers, pending deliveries) and saves it as <out-dir>/pre.sav, then makes the human buy through
the exe's MULTIGET_Trade (the messages go to replay.jsonl as {"k": "buy"}) before save 0.
The output holds game state derived from the user's install: keep it under _local/, never commit it.
Run in the foreground: launched from a background shell the game stalled after loading.
"""
import argparse
import json
import os
import shutil
import sys
import time

import frida

ap = argparse.ArgumentParser()
ap.add_argument("install")
ap.add_argument("save")
ap.add_argument("out")
ap.add_argument("--ticks", type=int, default=40)
ap.add_argument("--speed", type=int, default=3, help="SetIdoSpeed level: 1/2/3 = x1/x4/x16 in single player")
ap.add_argument("--seed", type=int, default=4242, help="libc rand() is reseeded with seed + Ido2 every tick")
ap.add_argument("--slot", type=int, default=99)
ap.add_argument("--slot-base", type=int, default=100, help="per-tick saves go to slot base + k (pick another base to record beside another run)")
ap.add_argument("--pre-slot", type=int, default=98)
ap.add_argument("--seconds", type=float, default=400)
ap.add_argument("--scenario", default="scenario/quick.scn")
ap.add_argument("--stage", action="store_true", help="stage trade purchases and deliveries before save 0")
ap.add_argument("--randlog", action="store_true", help="report the callers of libc rand() per tick ('rands')")
ap.add_argument("--story-countdown", type=int, default=100, help="--stage: StoryAdat+0xc4 (story merchant) in Ido")
ap.add_argument("--traders", action="store_true",
                help="give empire planets trade centres/banks and move traders near their targets before save 0")
ap.add_argument("--spies", action="store_true",
                help="stage spy missions, catch checks and reports for the first daily spy pass (info-messages.md 3.3)")
ap.add_argument("--spies-mode", default="base", choices=["base", "force1", "force2", "force3", "lost"],
                help="--spies: the staged mix; force1/2/3 also set the SP debug switch 0xB77250 (success/caught/recruit)")
ap.add_argument("--battles", action="store_true",
                help="stage auto-resolved space battles / ground wars for the first ticks (info-messages.md 3.2)")
ap.add_argument("--battles-mode", default="space", choices=["space", "ground", "land"])
ap.add_argument("--lopsided-tanks", type=int, default=40, help="--battles-mode ground: vehicles of the lopsided attacks")
ap.add_argument("--even-tanks", type=int, default=16, help="--battles-mode ground: vehicles of the even attacks")
ap.add_argument("--deaths", action="store_true",
                help="revive three dead empires without planets whose countdowns run out in the first days (BirodalomMeghal)")
ap.add_argument("--deaths-mode", default="base", choices=["base", "campaign"])
ap.add_argument("--lost-tanks", type=int, default=3, help="--battles-mode land: vehicles of the lopsided attacks that lose")
ap.add_argument("--land-ships", type=int, default=30, help="--battles-mode land: warships of the planet battle's attacker")
ap.add_argument("--wormholes", action="store_true",
                help="open a wormhole with the exe's AddWormhole and stage trips through it (and to an unlisted wormhole fleet) before save 0")
ap.add_argument("--merges", action="store_true",
                help="stage fleet merges, colonise/terraform arrivals, a dead empire's fleet and a wormhole trip before save 0")
ap.add_argument("--orders", type=int, default=0, help="stage N random starmap orders (MULTIGET_Starmap) every --orders-every ticks")
ap.add_argument("--orders-every", type=int, default=3)
ap.add_argument("--colony", type=int, default=0, help="stage N random colony orders (MULTIGET_Colony) every --orders-every ticks")
ap.add_argument("--research", type=int, default=0, help="stage N random research orders (MULTIGET_Research) every --research-every ticks")
ap.add_argument("--research-every", type=int, default=2)
ap.add_argument("--info", type=int, default=0, help="stage N random info-screen messages (MULTIGET_Info: shares, limit) every --info-every ticks")
ap.add_argument("--info-every", type=int, default=2)
ap.add_argument("--trader-near", type=float, default=0.5, help="--traders: distance left to the trader's planet")
a = ap.parse_args()

data_dir = os.path.join(os.environ["LOCALAPPDATA"], "GyroscopeGames", "ig2")
os.makedirs(a.out, exist_ok=True)
aside = os.path.join(a.out, "aside")
os.makedirs(aside, exist_ok=True)


def slot_file(n):
    return os.path.join(data_dir, f"save{n}.sav")


moved = []
for n in [a.slot, a.pre_slot] + list(range(a.slot_base, a.slot_base + a.ticks + 2)):
    if os.path.exists(slot_file(n)):
        shutil.move(slot_file(n), os.path.join(aside, f"save{n}.sav"))
        moved.append(n)
shutil.copyfile(a.save, slot_file(a.slot))

exe = os.path.join(a.install, "ig2.exe")
agent = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "replay.js"), encoding="utf-8").read()
log = open(os.path.join(a.out, "replay.jsonl"), "w", encoding="utf-8")
finished = False
saved = 0


def on_message(msg, _data):
    global finished, saved
    if msg["type"] == "error":
        print("agent error:", msg.get("description"), msg.get("stack", "")[:400], file=sys.stderr)
        return
    p = msg["payload"]
    if p.get("type") != "batch":
        print("agent:", p)
        return
    for ev in p["events"]:
        if ev["k"] == "save":
            src = slot_file(ev["slot"])
            if not os.path.exists(src):
                print("no save file", src, file=sys.stderr)
                continue
            shutil.move(src, os.path.join(a.out, f"{ev['idx']}.sav"))
            log.write(json.dumps(ev) + "\n")
            log.flush()
            saved += 1
            if saved % 10 == 1:
                print(f"save {ev['idx']} ({ev['where']}): Ido {ev['ido']} Ido2 {ev['ido2']}")
        elif ev["k"] == "pre":
            shutil.move(slot_file(ev["slot"]), os.path.join(a.out, "pre.sav"))
            log.write(json.dumps(ev) + "\n")
            print("staged:", ev["stage"])
        elif ev["k"] == "buy":
            log.write(json.dumps(ev) + "\n")
            print("buy: accepted" if ev["accepted"] else "buy: rejected")
        elif ev["k"] in ("sorder", "corder", "rorder", "iorder"):
            log.write(json.dumps(ev) + "\n")
        elif ev["k"] in ("traders", "spies", "merges", "wormholes", "battles", "deaths", "orders", "colony"):
            log.write(json.dumps(ev) + "\n")
            print(ev["k"], "staged:", ev["stage"])
        elif ev["k"] == "done":
            finished = True
        else:
            print("agent:", ev)


device = frida.get_local_device()
pid = device.spawn([exe], cwd=a.install)
session = device.attach(pid)
script = session.create_script(agent)
script.on("message", on_message)
script.load()
script.post({"type": "config", "payload": {"scenario": a.scenario, "slot": a.slot, "speed": a.speed, "ticks": a.ticks,
                                           "seed": a.seed, "stage": a.stage,
                                           "traders": a.traders, "traderNear": a.trader_near, "spies": a.spies, "spiesMode": a.spies_mode, "merges": a.merges, "wormholes": a.wormholes, "battles": a.battles, "deaths": a.deaths, "deathsMode": a.deaths_mode, "slotBase": a.slot_base, "preSlot": a.pre_slot, "battlesMode": a.battles_mode, "lopsidedTanks": a.lopsided_tanks, "evenTanks": a.even_tanks, "lostTanks": a.lost_tanks, "landShips": a.land_ships,
                                           "storyCountdown": a.story_countdown,
                                           "randlog": a.randlog, "orders": a.orders, "ordersEvery": a.orders_every, "colony": a.colony,
                                           "research": a.research, "researchEvery": a.research_every,
                                           "info": a.info, "infoEvery": a.info_every}})
device.resume(pid)
print(f"spawned ig2.exe pid {pid}; up to {a.seconds}s")
t0 = time.time()
try:
    while time.time() - t0 < a.seconds and not finished:
        time.sleep(1)
    time.sleep(2)
finally:
    try:
        script.unload()
    except frida.InvalidOperationError:
        pass
    device.kill(pid)
    log.close()
    time.sleep(1)
    if os.path.exists(slot_file(a.slot)):
        os.remove(slot_file(a.slot))
    for n in moved:
        shutil.move(os.path.join(aside, f"save{n}.sav"), slot_file(n))
    os.rmdir(aside)
    print(f"{saved} saves -> {a.out}")
