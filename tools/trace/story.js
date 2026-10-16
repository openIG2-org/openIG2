// Frida agent: the story engine on the GOG ig2.exe (spec re/rules/campaign.md).
// Records:
//  'tick'   one per FRISSIT_Story (exe 0x5c1ff0) the game runs: the story state and the world inputs the scenario
//           scripts read, before and after (the counters' Story_Check(1) and the daily one nest inside);
//  'check'  one per outermost Story_Check (0x5c5610) the game enters from anywhere else (a Story_* hook);
//  'staged' a call the tracer makes itself from the frame hook (--fuzz): Story_Check(1) after writing event
//           flags and payloads, Story_Special, Story_FleetArrivedSave + Story_FleetArrivedCheck,
//           Story_SpaceBattleFinished with a fake participant list, st_AddGameInfo/st_RemoveGameInfo, WIN.
// Between records the fuzzer (--fuzz) queues scenario stories 360..375 (StartStory), sets their variables
// (st_vSet), planet ids (st_bAddNewAzonosito), fleet ids (fleet +0x16), counters (st_szStart) and other
// story state. MEGNYERES (0x53f6b0: the game is won) and MEGHALAS (0x53b630: an empire dies) are patched to
// count their calls and return, so the game goes on.
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  Frissit: va(0x5c1ff0), Check: va(0x5c5610), Win: va(0x5c9770), Megnyeres: va(0x53f6b0), Meghalas: va(0x53b630),
  StartStory: va(0x5c4840), AddGameInfo: va(0x5c9d00), RemoveGameInfo: va(0x5f0d50), BAddAzon: va(0x6031f0),
  VSet: va(0x605780), SzStart: va(0x605570), Special: va(0x5c9550), ArrivedSave: va(0x5c8cb0),
  ArrivedCheck: va(0x5c8c90), SpwFinished: va(0x5c9180), DiplSpecial: va(0x5c8c60),
  // Story state.
  sBolygok: va(0xf23a20), sSzamlalok: va(0xf24388), sLogikaiak: va(0xf24cf0), sValtozok: va(0xf251a8),
  nB: va(0xf23a08), nSz: va(0xf23a0c), nL: va(0xf23a10), nV: va(0xf23a14), Err: va(0xf23a1c), SkipFirst: va(0xf23a04),
  Flags: va(0xf2f8ac), Last: va(0xf2273c), SzVegzett: va(0xf26560), OldIdo: va(0xf432b4), Delayed: va(0xf26530),
  Restart: va(0xf261fc), Locked: va(0xf265dc), Adat: va(0xf273f8), Vodor: va(0xf42678), History: va(0xf43278),
  StartQ: va(0xf43284), Vegzes: va(0xf43290), Talal: va(0xf4329c), Olvas: va(0xf432a8),
  OzonN: va(0xf265d8), Ozon: va(0xf265e0), Signal: va(0xf26564), LastAddship: va(0xf26200),
  Special0: va(0xf265a4), RadarB: va(0xf2657c), RadarF: va(0xf26580), Dipl: va(0xf2656c), DiplValasz: va(0xf42660),
  Arr: va(0xf2654c), Gr: va(0xf26538), GrWin: va(0xf26532), YouWas: va(0xf26531), Spw: va(0xf265c4), KemTipus: va(0xf26588),
  // World.
  Ido: va(0xbae04c), Speed: va(0xbae25c), EN: va(0x912a3c), Nehezseg: va(0x92827c), MP: va(0xbae2c0),
  Campaign: va(0x928278), El: va(0xbae6ac), HanyB: va(0xbaeb18), Money: va(0xbad168), Techs: va(0xb9c0c8),
  Viszony: va(0xdabf38), RandSeed: va(0xbaf2d0), Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8), AktKep: va(0xbae0fc), AktKepKell: va(0xbae250),
};

const cfg = { scenario: 'scenario/intruders.scn', startAfterFrames: 1200, count: 2000, fuzz: false, seed: 4242,
  every: 40, speed: 3 };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 8) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

let fz = 1;
const rnd = (n) => { fz ^= fz << 13; fz >>>= 0; fz ^= fz >>> 17; fz ^= fz << 5; fz >>>= 0; return n > 0 ? fz % n : 0; };
const pick = (arr) => arr[rnd(arr.length)];
const hex = (p, n) => { const b = new Uint8Array(p.readByteArray(n)); let s = ''; for (const x of b) s += (x < 16 ? '0' : '') + x.toString(16); return s; };

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const storyCheck = nf(A.Check, 'void', ['int']);
const startStory = nf(A.StartStory, 'int', ['int', 'int']);
const vSet = nf(A.VSet, 'void', ['int', 'int']);
const szStart = nf(A.SzStart, 'void', ['int', 'int']);
const bAddAzon = nf(A.BAddAzon, 'void', ['int', 'int']);
const storySpecial = nf(A.Special, 'void', ['int', 'int', 'int', 'int', 'int', 'int', 'int', 'int']);
const arrivedSave = nf(A.ArrivedSave, 'void', ['int']);
const arrivedCheck = nf(A.ArrivedCheck, 'void', []);
const spwFinished = nf(A.SpwFinished, 'void', ['pointer', 'int', 'int']);
const addGameInfo = nf(A.AddGameInfo, 'void', ['int', 'pointer', 'pointer', 'pointer', 'int', 'int']);
const removeGameInfo = nf(A.RemoveGameInfo, 'void', ['int']);
const win = nf(A.Win, 'int', ['int', 'int']);

// MEGNYERES / MEGHALAS: count the calls, remember the last argument, return.
const counters = Memory.alloc(16);
function patchCounter(addr, slot) {
  Memory.patchCode(addr, 16, (code) => {
    const w = new X86Writer(code, { pc: addr });
    w.putMovRegRegOffsetPtr('eax', 'esp', 4);
    w.putMovNearPtrReg(counters.add(slot * 8 + 4), 'eax');
    const c = counters.add(slot * 8).toUInt32();
    w.putBytes([0xff, 0x05, c & 0xff, (c >>> 8) & 0xff, (c >>> 16) & 0xff, c >>> 24]); // inc dword [count]
    w.putRet();
    w.flush();
  });
}
patchCounter(A.Megnyeres, 0);
patchCounter(A.Meghalas, 1);
const ends = () => [counters.readS32(), counters.add(4).readS32(), counters.add(8).readS32(), counters.add(12).readS32()];

const nPlanets = () => A.HanyBolygo.readS32();
const nFleets = () => A.FlottaCount.readS32();
const fleet = (f) => A.Flottak.readPointer().add(0x80 * f);

function pairs(base, n, flagByte) {
  const out = [];
  for (let i = 1; i <= n; i++) {
    const p = base.add(8 * i);
    out.push([p.readS32(), flagByte ? p.add(4).readU8() : p.add(4).readS32()]);
  }
  return out;
}
function vodor(p) {
  const ids = p.readPointer(), dual = p.add(4).readPointer(), n = p.add(8).readS32(), out = [];
  for (let i = 0; i < n; i++) out.push([ids.add(4 * i).readS32(), dual.add(4 * i).readS32()]);
  return out;
}
function story() {
  const buckets = {};
  for (let k = 0; k < 256; k++) {
    const q = vodor(A.Vodor.add(12 * k));
    if (q.length) buckets[k] = q;
  }
  const on = A.OzonN.readS32(), ozon = [];
  for (let i = 0; i < on; i++) { const p = A.Ozon.add(12 * i); ozon.push([p.readS32(), p.add(4).readS32(), p.add(8).readS32()]); }
  const ints = (p, n) => { const o = []; for (let i = 0; i < n; i++) o.push(p.add(4 * i).readS32()); return o; };
  return {
    b: pairs(A.sBolygok, A.nB.readS32(), true), sz: pairs(A.sSzamlalok, A.nSz.readS32(), false),
    l: pairs(A.sLogikaiak, A.nL.readS32(), true), v: pairs(A.sValtozok, A.nV.readS32(), false),
    nb: A.nB.readS32(), nsz: A.nSz.readS32(), nl: A.nL.readS32(), nv: A.nV.readS32(),
    err: A.Err.readS32(), flags: hex(A.Flags, 17), last: A.Last.readS32(), szv: A.SzVegzett.readS32(),
    oldido: A.OldIdo.readS32(), delayed: A.Delayed.readU8(), skip: A.SkipFirst.readS32(), restart: A.Restart.readS32(),
    adat: hex(A.Adat, 0x128), buckets, hist: vodor(A.History), start: vodor(A.StartQ), vegzes: vodor(A.Vegzes),
    talal: vodor(A.Talal), olvas: vodor(A.Olvas), ozon, signal: ints(A.Signal, 2), addship: A.LastAddship.readS32(),
    special: ints(A.Special0, 8), radar: [A.RadarB.readS32(), A.RadarF.readS32()],
    dipl: [A.Dipl.readS32(), A.Dipl.add(4).readS32(), A.DiplValasz.readS32()], arr: ints(A.Arr, 5),
    gr: ints(A.Gr, 5).concat([A.GrWin.readU8(), A.GrWin.add(1).readU8()]), youwas: A.YouWas.readU8(),
    spw: [A.Spw.readS32(), A.Spw.add(4).readS32()], kem: A.KemTipus.readS32(),
  };
}
function varOf(id) {
  const n = A.nV.readS32();
  for (let i = 1; i <= n; i++) if (A.sValtozok.add(8 * i).readS32() === id) return A.sValtozok.add(8 * i + 4).readS32();
  return 0;
}
function world() {
  const en = A.EN.readS32(), np = nPlanets(), nfl = nFleets(), planets = [], fleets = [];
  for (let p = 0; p <= np; p++) { // slot 0 too: a failed search returns 0 and the scripts read it
    const b = A.Bolygok.add(0x188 * p);
    planets.push([b.add(0x20).readU8(), b.add(0x28).readS32(), b.add(0xd8 + 2 * en).readS16()]);
  }
  for (let f = 0; f <= nfl; f++) { const q = fleet(f); fleets.push([q.add(0x15).readU8(), q.add(0x16).readS16()]); }
  const hanyb = [], visz = [];
  for (let e = 0; e < 19; e++) { hanyb.push(A.HanyB.add(4 * e).readS32()); visz.push(A.Viszony.add((en * 19 + e) * 0x26 + 0x25).readU8()); }
  const tech = varOf(0xe94);
  return {
    ido: A.Ido.readS32(), speed: A.Speed.readS32(), en, neh: A.Nehezseg.readS32(), mp: A.MP.readU8(),
    camp: A.Campaign.readS32(), el: hex(A.El, 19), hanyb, money: A.Money.add(8 * en).readDouble(), visz,
    tech: tech >= 0 && tech < 200 ? [tech, hex(A.Techs.add(0x114 * tech + 0x45), 19)] : [tech, ''],
    seed5: A.RandSeed.add(20).readU32(), planets, fleets,
  };
}
const snap = () => ({ s: story(), w: world(), ends: ends() });

let seq = 0, phase = 0, inTick = false, depth = 0, cur = null;
Interceptor.attach(A.Frissit, {
  onEnter() { if (phase === 1) { inTick = true; cur = { k: 'tick', seq: seq++, pre: snap() }; } },
  onLeave() {
    if (cur && cur.k === 'tick') { cur.post = snap(); emit(cur); cur = null; }
    inTick = false;
    if (seq >= cfg.count && phase === 1) { emit({ k: 'done' }); phase = 2; }
  },
});
Interceptor.attach(A.Check, {
  onEnter(args) {
    depth++;
    if (phase === 1 && !inTick && depth === 1) {
      cur = { k: 'check', seq: seq++, arg: args[0].toInt32() & 0xff, from: '0x' + (this.returnAddress.sub(mod.base).toInt32() + IMAGE_BASE).toString(16), pre: snap() };
    }
  },
  onLeave() {
    depth--;
    if (depth === 0 && cur && cur.k === 'check') { cur.post = snap(); emit(cur); cur = null; }
  },
});

// ---------------------------------------------------------------- staged calls (--fuzz)
// Variables, planet and fleet ids the scenario stories read (st_scen.cpp, stories 0x168..0x177).
const STORIES = [0x168, 0x169, 0x16a, 0x16b, 0x16c, 0x16d, 0x16e, 0x16f, 0x170, 0x171, 0x172, 0x173, 0x174, 0x175, 0x176, 0x177];
const fake = Memory.alloc(0x30 * 4);
function liveFleets() { const o = []; for (let f = 1; f <= nFleets(); f++) if (fleet(f).add(0x15).readU8() !== 0xff) o.push(f); return o; }
function empireOrZero() { return rnd(4) === 0 ? 0 : rnd(19); }
function prepare(id) {
  const np = nPlanets(), en = A.EN.readS32();
  const randPlanet = () => 1 + rnd(np);
  switch (id) {
    case 0x168: case 0x16b: case 0x174: {
      const fid = id === 0x168 ? 0xe11 : id === 0x16b ? 0xe2f : 0xe89, lf = liveFleets(), dead = [];
      for (let f = 1; f <= nFleets(); f++) if (fleet(f).add(0x15).readU8() === 0xff) dead.push(f);
      // A dead slot with the id too: st_fSearch skips free slots.
      if (dead.length && rnd(2)) fleet(pick(dead)).add(0x16).writeS16(fid);
      if (lf.length && rnd(3)) fleet(pick(lf)).add(0x16).writeS16(fid);
      break;
    }
    case 0x169: case 0x176: { const n = 1 + rnd(7); for (let k = 1; k <= n; k++) vSet((id === 0x169 ? 0xe1a : 0xe9c) + k, rnd(3) ? empireOrZero() : 0); break; }
    case 0x16a: case 0x16c: case 0x16d: bAddAzon(randPlanet(), { 0x16a: 0xe25, 0x16c: 0xe39, 0x16d: 0xe43 }[id]); break;
    case 0x16e: vSet(0xe4d, 1 + rnd(4)); break;
    case 0x16f: { const m = A.Money.add(8 * en).readDouble(); vSet(0xe57, rnd(2) ? Math.floor(m) - 1 + rnd(3) : rnd(Math.max(1, Math.floor(m * 2) + 2))); break; }
    case 0x170: vSet(0xe61, A.HanyB.add(4 * en).readS32() - 1 + rnd(3)); break;
    case 0x171: {
      const own = []; for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === en) own.push(p);
      const p = own.length && rnd(3) ? pick(own) : randPlanet(), val = A.Bolygok.add(0x188 * p + 0x28).readS32();
      vSet(0xe6b, rnd(2) ? val - 1 + rnd(3) : rnd(30000)); bAddAzon(p, 0xe6b); break;
    }
    case 0x172: vSet(0xe75, rnd(19)); break;
    case 0x173: vSet(0xe7f, pick([1, 2, 3, 29, 31, 40, 359, 361, 400])); break;
    case 0x175: vSet(0xe93, rnd(19)); vSet(0xe94, 1 + rnd(192)); break;
    case 0x177: vSet(0xea7, rnd(2) ? A.HanyB.add(4 * en).readS32() - 1 + rnd(3) : pick([0, 2, 3, 4, 5, 6, rnd(19)])); break;
  }
}
function staged(name, f) {
  const r = { k: 'staged', seq: seq++, call: name, pre: snap() };
  f(r);
  r.post = snap();
  emit(r);
}
function fuzzStep() {
  const c = rnd(14), en = A.EN.readS32();
  if (c < 5) {
    const id = pick(STORIES);
    prepare(id);
    if (rnd(4) === 0) {
      staged('start', (r) => { r.args = [id]; startStory(id, 0); storyCheck(1); });
    } else {
      startStory(id, 0); // runs at the next Story_Check(1)
    }
  } else if (c === 5) {
    const id = rnd(2) ? pick([0xe4d, 0xe7f]) : 9000 + rnd(12), d = rnd(4);
    szStart(id, d);
  } else if (c === 6) {
    const args = [pick([0x11, 0x12, 0xc, 0xf, 3, 7]), rnd(30), rnd(5), rnd(3), rnd(9), rnd(9), rnd(9), rnd(9)];
    staged('special', (r) => { r.args = args; storySpecial(...args); });
  } else if (c === 7) {
    const lf = liveFleets();
    if (!lf.length) return;
    const f = pick(lf), q = fleet(f), kind = rnd(3), tgt = kind === 2 ? pick(lf) : 1 + rnd(nPlanets());
    const save = [q.add(0x42).readS16(), q.add(0x44).readFloat()];
    staged('arrive', (r) => {
      r.args = [f, kind, tgt];
      q.add(0x42).writeS16(kind); q.add(0x44).writeFloat(tgt);
      arrivedSave(f); arrivedCheck();
      q.add(0x42).writeS16(save[0]); q.add(0x44).writeFloat(save[1]);
    });
  } else if (c === 8) {
    const n = 2 + rnd(2), list = [];
    for (let i = 0; i < n; i++) {
      const owner = rnd(3) ? (i === 0 ? en : rnd(19)) : rnd(19), id = pick([0, 0xe89, 0xe11, rnd(5000)]);
      const p = fake.add(0x30 * i); p.add(0x18).writeS32(owner); p.add(0x20).writeS32(id); p.add(0x24).writeS32(id); list.push([owner, id]);
    }
    staged('spwfinished', (r) => { r.args = list; spwFinished(fake, n, 1); });
  } else if (c === 9) {
    // Raw trigger flags + Story_Check(1): space battle start (1), counter (5), daily (0x10), signal (0xc), ...
    const flag = pick([1, 5, 0x10, 0xc, 0, 3, 0xe]);
    staged('flag', (r) => {
      const szv = flag === 5 ? pick([0xe4d, 0xe7f, 9000 + rnd(12)]) : 0;
      r.args = [flag, szv];
      A.Flags.add(flag).writeU8(1);
      if (flag === 5) A.SzVegzett.writeS32(szv);
      storyCheck(1);
    });
  } else if (c === 10) {
    const id = pick([0xe7f, 0xe80, 0xe81, 0xe4d, 3600 + rnd(200)]), owner = pick([-1, -1, 0, rnd(400)]);
    staged('gameinfo', (r) => { r.args = [id, owner]; addGameInfo(id, Memory.allocAnsiString('x'), ptr(0), ptr(0), 1, owner); });
  } else if (c === 11) {
    const on = A.OzonN.readS32(), id = on && rnd(2) ? A.Ozon.add(12 * rnd(on) + 4).readS32() : 3600 + rnd(200);
    staged('rmgameinfo', (r) => { r.args = [id]; removeGameInfo(id); });
  } else if (c === 12) {
    const ids = [];
    for (const k in story().buckets) for (const [id] of story().buckets[k]) ids.push(id);
    const id = ids.length && rnd(3) ? pick(ids) : pick(STORIES), hist = rnd(2);
    staged('win', (r) => { r.args = [id, hist]; win(id, hist); });
  } else {
    // Unrelated variables, logic flags and planet ids (the engine's tables).
    if (rnd(2)) vSet(8000 + rnd(40), rnd(100) - 20);
    else bAddAzon(rnd(4) ? 1 + rnd(nPlanets()) : 0, 8000 + rnd(40));
  }
}

let frames = 0, started = false, loadedAt = 0, lastIdo = -1, sameFor = 0, homeKep = -1, wantSpeed = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true; fz = cfg.seed;
      betolt(Memory.allocAnsiString(cfg.scenario));
      setSpeed(cfg.speed, 1);
      loadedAt = frames;
      emit({ k: 'loaded' });
      return;
    }
    if (started && phase === 0 && frames >= loadedAt + 300) {
      homeKep = A.AktKep.readS32(); wantSpeed = A.Speed.readS32(); phase = 1;
      emit({ k: 'setup', init: snap() });
    }
    if (phase === 1 && cfg.fuzz && frames % cfg.every === 0) {
      try { fuzzStep(); } catch (e) { emit({ k: 'error', where: 'fuzz', e: String(e), stack: e.stack }); }
    }
    if (phase === 1 && frames % 300 === 0 && A.Speed.readS32() !== wantSpeed) setSpeed(cfg.speed, 1);
    if (phase === 1) {
      const ido = A.Ido.readS32();
      sameFor = ido === lastIdo ? sameFor + 1 : 0;
      lastIdo = ido;
      if (sameFor === 200 || (sameFor > 200 && sameFor % 600 === 0)) {
        emit({ k: 'stall', seq, ido, kep: A.AktKep.readS32(), speed: A.Speed.readS32() });
        A.AktKepKell.writeS32(homeKep); setPause(0, 0); setSpeed(cfg.speed, 1);
      }
    }
  },
});

send({ type: 'ready', base: mod.base.toString() });
