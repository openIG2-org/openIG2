// Frida agent for reference screenshots of the original's screens (re/screens/*.md): starts scenario/quick.scn,
// loads save<slot> with the exe's `LoadSave(slot, name, 1)` (exe 0x543c80), pauses the clock and asks for the
// screen the config names (`AktKepKell` 0xBAE250 / `AktPlanetKell` 0xBAE254: 3 = a planet's colony screen), then
// tells the driver when to grab the window. The driver (run_screen.py) captures it. Output: game imagery, keep it
// under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  LoadSave: va(0x543c80), AktKep: va(0xbae0fc), AktKepKell: va(0xbae250), AktPlanetKell: va(0xbae254), AktPlanet: va(0xb9933c),
  Ido2: va(0xbae050),
};

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl', traps: 'all' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const loadSave = nf(A.LoadSave, 'void', ['int', 'pointer', 'int']);

// `Ev`/`Honap`/`Nap` (0xBAE040/44/48).
const date = () => [0xbae040, 0xbae044, 0xbae048].map((x) => va(x).readS32());

// Skip the title video: `MainScr_Belep` (exe 0x4fe3c0) plays `title.bik` once (`VoltGTLogo` 0xBBB22F clear) and the
// player's click ends it; `PlayBink` runs the frames itself, so no screen request is taken until it ends.
va(0xbbb22f).writeU8(1);

let cfg = null;
recv('config', (m) => { cfg = m.payload; if (cfg.rop) hookRop(); });

let frames = 0, phase = 0, at = 0, shot = 0, ropDone = false;

// --spies MODE: a roster for the human's spy screen (`Kemek` 0xBAFCC8 [9][13] × 0x4c, `OsszHanyKem` 0xBB1F84;
// re/rules/espionage.md §1.1): five spies of different races and states (at home with skill points, on a mission
// against the AI, travelling home, captured, at home), the human knowing the AI (`BirodalomIsmeri` 0xBAEBB8). MODE
// also queues one report (`KemBeszolasok` 0xBAF338, count 0xBAF334; info-messages.md §3.3) the screen shows first:
// roster none, report a finished empire report (kind 1), verdict an AI spy caught by the human (kind 3), applicant
// a new recruit (kind 2).
function stageSpies(mode) {
  const en = va(0x912a3c).readS32(), ai = cfg.aiEmpire;
  const kemek = va(0xbafcc8), ossz = va(0xbb1f84);
  const rec = (e, k) => kemek.add((e * 13 + k) * 0x4c);
  // [name, state, race, mission, target, p1, left, skills, points, xp, rank, applicant]
  const plan = [
    ['Agent Alpha', 1, 1, 0, 0, 0, 0, [40, 30, 20, 50], 2, 4000, 3, 0],
    ['Agent Bravo', 3, 4, 2, ai, 0, 400, [60, 10, 30, 40], 0, 9000, 5, 0],
    ['Agent Charlie', 2, 7, 0, 0, 0, 100, [20, 20, 20, 20], 0, 500, 1, 0],
    ['Agent Delta', 4, 13, 0, 0, 0, 0, [10, 50, 70, 5], 0, 1200, 2, 0],
    ['Agent Echo', 1, 14, 0, 0, 0, 0, [90, 90, 90, 90], 0, 50000, 9, 0],
  ];
  if (mode === 'applicant') plan.push(['Agent Foxtrot', 1, 6, 0, 0, 0, 0, [30, 30, 30, 30], 0, 3000, 2, 1]);
  plan.forEach(([name, st, race, m, t, p1, left, sk, pts, xp, rank, appl], i) => {
    const r = rec(en, i + 1);
    r.writeByteArray(new Array(0x4c).fill(0));
    r.writeUtf8String(name);
    r.add(0x10).writeS16(st); r.add(0x12).writeU8(race); r.add(0x16).writeS16(appl);
    r.add(0x1a).writeU16(m); r.add(0x1c).writeU8(t); r.add(0x1e).writeS16(p1);
    r.add(0x24).writeS32(left); r.add(0x28).writeS32(left); r.add(0x2c).writeS32(-1000);
    r.add(0x31).writeByteArray(sk); r.add(0x38).writeS32(pts); r.add(0x3c).writeS32(xp); r.add(0x40).writeU8(rank);
  });
  ossz.add(4 * en).writeS32(plan.length);
  // One AI spy at home (the verdict's prisoner).
  const n = ossz.add(4 * ai).readS32() + 1;
  const r = rec(ai, n);
  r.writeByteArray(new Array(0x4c).fill(0));
  r.writeUtf8String('Agent Spectre');
  r.add(0x10).writeS16(4); r.add(0x12).writeU8(3); r.add(0x31).writeByteArray([50, 50, 50, 30]); r.add(0x40).writeU8(4);
  ossz.add(4 * ai).writeS32(n);
  va(0xbaebb8).add(19 * en + ai).writeU8(1);
  const reports = va(0xbaf338), count = va(0xbaf334);
  const put = (kind, a, b, c, x) => {
    const q = reports.add(0x30);
    q.writeByteArray(new Array(0x30).fill(0));
    q.writeU8(kind); q.add(4).writeS32(a); q.add(8).writeU8(b); q.add(9).writeU8(c);
    x.forEach((v, i) => q.add(0xc + 4 * i).writeS32(v));
    count.writeS32(1);
  };
  count.writeS32(0);
  if (mode === 'report') put(1, 2, 4, 1, [2, ai, 0, 0, 7, 1234567, 23, 61, 0]);
  if (mode === 'verdict') put(3, 1, 0, 0, [0, ai, n, 0, 0, 0, 0, 0, 0]);
  if (mode === 'applicant') put(2, plan.length, 0, 0, [0, en, 0, 0, 0, 0, 0, 0, 0]);
  return { en, ai, spies: plan.length, prisoner: n };
}

// --fleets N: N fleets of the human (`EN` 0x912A3C) beside the shown planet, built like production does (`FlottaNew`
// 0x53d400, `HajoNew` 0x53e800, `TankNew` 0x541750): ships of the human's first two ship designs and, when it has
// one, a vehicle entry; quick-1.sav's human has no fleet of its own.
const flottaNew = nf(va(0x53d400), 'int', ['pointer', 'int', 'int', 'float', 'float', 'float']);
const hajoNew = nf(va(0x53e800), 'void', ['int', 'int']);
const tankNew = nf(va(0x541750), 'void', ['int', 'int', 'int']);
const fleets = [];
function populate(n) {
  const e = va(0x912a3c).readS32(), cnt = va(0xb9c028).add(4 * e).readS32(), designs = va(0xb9aa18).add(4 * e).readPointer();
  const ships = [], vehicles = [];
  for (let d = 1; d <= cnt; d++) {
    const p = designs.add(0x84 * d), cls = p.add(0x16).readS16();
    if (cls === 1 && p.add(0x36).readU8() === 0) ships.push(d); else if (cls === 2) vehicles.push(d);
  }
  const q = va(0xb85ea8).add(0x188 * cfg.planet);
  for (let k = 0; k < n && ships.length; k++) {
    const f = flottaNew(Memory.allocAnsiString('Fleet ' + (k + 1)), e, 0, q.add(0x18).readFloat() + 2 * k, q.add(0x1c).readFloat() + 1, 0);
    if (f < 1) continue;
    for (let j = 0; j < 3 + k; j++) hajoNew(f, ships[j % Math.min(2, ships.length)]);
    if (vehicles.length) tankNew(f, vehicles[0], 2 + k);
    fleets.push(f);
  }
  send({ k: 'fleets', e, ships, vehicles, fleets });
}

// --rop N: press the starmap's icon N once (`AktRopIkon` 0xE83654: 1 tanks/transfer, 2 production, 0xd split, 0xe
// transfer between two fleets), as a click would. `Starmap_Main` (exe 0x5b2b70) clears it on entry, so the value is
// put in right after that store.
function hookRop() {
  const target = va(0xe83654), b = [0xc7, 0x05];
  for (let i = 0; i < 4; i++) b.push(target.shr(8 * i).and(0xff).toInt32());
  const pattern = b.concat([0, 0, 0, 0]).map((x) => ('0' + x.toString(16)).slice(-2)).join(' ');
  const hit = Memory.scanSync(va(0x5b2b70), 0x200, pattern)[0];
  if (!hit) { send({ k: 'error', what: 'AktRopIkon store not found' }); return; }
  Interceptor.attach(hit.address.add(10), function () {
    if (cfg.ropNow && !ropDone) { target.writeS32(cfg.rop); ropDone = true; send({ k: 'rop', rop: cfg.rop }); }
  });
}
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    if (!cfg) return;
    frames++;
    // `IsActive` (exe 0x912A38) is clear while the window has no focus; `IG2_gameUpdate` then skips `UpdateFrame`,
    // which also finishes the screen fades, so no screen change completes. The capture runs in the background.
    va(0x912a38).writeU8(1);
    // `MainScr_Kilep` (exe 0x4fecc0) leaves the main menu by looping `UpdateFrame` + `FRISSITES` until its fade
    // counter (f32 0xBBB360, from 100) runs out, flagged by 0xBBB22A; the counter doesn't run in a background window.
    if (va(0xbbb22a).readU8() !== 0) va(0xbbb360).writeFloat(0);
    // Keep the pointer in the middle of the 640 × 480 screen (`Mouse_Akt_x/y` 0x916008/0x91600C, `Mouse_x/y`
    // 0xDB4654/0xDB4658), so the colony screen's edge scrolling (`Colony_Autoscroll`) stays still.
    // --cam: pin `ColonyXStart/YStart/Zoom/Pitch/Rotate` (0xBA90DC, 0xBA9514, 0xBA9E8C, 0xBA9B64, 0xBA983C).
    if (phase >= 3 && cfg.cam && A.AktKep.readS32() === 3) {
      [0xba90dc, 0xba9514, 0xba9e8c, 0xba9b64, 0xba983c].forEach((x, i) => va(x).writeFloat(cfg.cam[i]));
    }
    // Keep the clock paused (`IdoPause` 0xBAE264) once the screen is asked for (the loading screen needs it running,
    // and the screens' `_Belep` can restart it), so the save written at the grab is the moment on screen.
    if (phase >= 3 && !cfg.run && va(0xbae264).readU8() === 0) va(0xbae264).writeU8(1);
    if (phase >= 2) {
      va(0x916008).writeS32(320); va(0x91600c).writeS32(240);
      va(0xdb4654).writeS32(320); va(0xdb4658).writeS32(240);
    }
    // --debug: the dispatcher's state (`AktKep`, `AktKepKell` 0xBAE250, `Atlepes` 0xBAE24D, `Belep` 0xBAE24C,
    // `OldKep` 0xBAE19C, `IdoPause`) every 120 frames.
    if (cfg.debug && frames % 120 === 0) {
      send({ k: 'state', frames, phase, kep: A.AktKep.readS32(), kell: A.AktKepKell.readS32(), atlepes: va(0xbae24d).readU8(),
        belep: va(0xbae24c).readU8(), old: va(0xbae19c).readS32(), pause: va(0xbae264).readU8(), date: date(),
        stack: Thread.backtrace(this.context, Backtracer.FUZZY).slice(0, 16).map((p) => '0x' + p.sub(mod.base).add(IMAGE_BASE).toString(16)) });
    }
    if (phase === 0 && frames === 1200) {  // the menu needs ~1200 frames (replay.js)
      betolt(Memory.allocAnsiString('scenario/quick.scn'));
      at = frames; phase = 1;
      send({ k: 'scenario' });
    } else if (phase === 1 && frames >= at + 300) {
      loadSave(cfg.slot, Memory.allocAnsiString('screen'), 1);
      setSpeed(1, 1);
      setPause(cfg.run ? 0 : 1, 0);
      send({ k: 'loaded', kep: A.AktKep.readS32(), ido2: A.Ido2.readS32(), date: date() });
      at = frames; phase = 2;
    } else if (phase === 2 && frames >= at + 60 && (A.AktKep.readS32() === 2 || frames >= at + 300)) {
      // `LoadSave` leaves the game on the main menu screen (15) while the clock is paused (with the clock running it
      // goes on to the starmap); ask for ours from there.
      if (cfg.spies) send({ k: 'spies', stage: stageSpies(cfg.spies) });
      if (cfg.kep >= 0) {
        A.AktKepKell.writeS32(cfg.kep);
        A.AktPlanetKell.writeS32(cfg.planet);
      }
      at = frames; phase = 3;
    } else if (phase === 3 && cfg.rop && !cfg.ropNow && A.AktKep.readS32() === 2 && frames >= at + 60) {
      // The starmap is up: make the fleets, select one (--rop-fleet K: `StarmapMode` 0xB9998C = 2, `AktFleet`
      // 0xB99664 = the K-th made), then press the icon and wait from there.
      if (cfg.fleets) populate(cfg.fleets);
      if (cfg.ropFleet && fleets[cfg.ropFleet - 1]) { va(0xb9998c).writeS32(2); va(0xb99664).writeS32(fleets[cfg.ropFleet - 1]); }
      cfg.ropNow = true;
      at = frames;
    } else if (phase === 3 && (!cfg.rop || ropDone) && frames >= at + cfg.wait) {
      const f = (x) => va(x).readFloat();
      const p = A.AktPlanet.readS32();
      const wf = 0x9c3244 + p * 0x8ec;
      // `ColonyXStart/YStart/Zoom/Pitch/Rotate`, the camera points (`forgaspont` 0xDE0130, `kamerapont` 0xDE0148,
      // (h, x, y)), the battlefield's saved camera (`WF+0x1a8..`) and its centring flag `WF+0x184`.
      const cam = { x: f(0xba90dc), y: f(0xba9514), zoom: f(0xba9e8c), pitch: f(0xba9b64), rotate: f(0xba983c),
        pivot: [f(0xde0130), f(0xde0134), f(0xde0138)], eye: [f(0xde0148), f(0xde014c), f(0xde0150)],
        saved: [0, 4, 8, 12, 16].map((o) => f(wf + 0x1a8 + o)), centre: va(wf + 0x184).readU8() };
      // --state-out: write the state on screen with the exe's own `LoadSave(slot, name, 2)`, so the client can render
      // the very same moment.
      if (cfg.stateSlot) loadSave(cfg.stateSlot, Memory.allocAnsiString('screendiff'), 2);
      send({ k: 'grab', n: shot, kep: A.AktKep.readS32(), planet: p, ido2: A.Ido2.readS32(), date: date(), paused: va(0xbae264).readU8(), cam });
      shot++;
      at = frames;
      if (shot >= cfg.shots) phase = 4;
    }
  },
});
