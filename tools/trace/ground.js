// Frida agent: the auto-resolved ground war on the GOG ig2.exe (spec re/rules/ground-combat.md).
// Modes (config.mode):
//   war    - stage a landing: a fleet of an empire's ground vehicles (FlottaNew/HajoNew/TankNew) next to another
//            empire's planet (optionally with extra garrison vehicles TankNew(-p, ...)); then, from the frame hook,
//            srand(S1); AddTanksToPlanet(f, p, 1) (0x45e320); srand(S2); WF::Simulate (0x4a3730, thiscall on
//            the planet's battlefield). Snapshots A (before), B (after the landing), C (after the war): the
//            battlefield (every unit's 0x290-byte record with its hp/origin arrays), Tankok, fleets, the
//            planet and its buildings, RNG streams, and the libc rand() draws (two rand() after each call:
//            the draws are the MSVC LCG outputs from S up to that pair; matched in the Rust replay).
//            --fuzz: random ground pods on the attacker's designs, garrisons, building flags/damage,
//            human-controlled attacker (units keep flag 2), story damage bonus.
//   indit  - GroundWarIndit (0x59c2d0) on staged attacks (AI and human-controlled sides, lopsided or not), with
//            StartGrWar patched to `ret`: records the route and the outcome (planet owner, fleet vehicles).
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0),
  SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  GroundWarIndit: va(0x59c2d0), AddTanksToPlanet: va(0x45e320), Simulate: va(0x4a3730),
  StartGrWar: va(0x4a4a70), GroundWarFinished: va(0x47c5d0),
  FlottaNew: va(0x53d400), HajoNew: va(0x53e800), TankNew: va(0x541750), FlottakUjraSzamol: va(0x53dd90),
  RandSeed: va(0xbaf2d0),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8),
  Tankok: va(0xbb41a8), TankCount: va(0xbb4154),
  Tervek: va(0xb9aa18), TervCount: va(0xb9c028),
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8),
  SlotCount: va(0xba9840), Epuletek: va(0xba9e90), BuildingCount: va(0xba9518),
  PlayerKi: va(0xbae560), EN: va(0x912a3c), MP: va(0xbae2c0), MpBattleStatus: va(0xbae558),
  Players2: va(0x912ad0), StoryPlanet: va(0xf27438), Bonus1: va(0xf27468), Bonus2: va(0xf2746c),
  StoryNoDiplo: va(0xf27480), DbgNoLop: va(0xb7723c), DbgLopOff: va(0xb77230), DbgAlwaysSim: va(0xb77234),
  SimBan: va(0xf42670), ErodEro: va(0xb99cb8), BattlesWon: va(0xb79fb4), Timing: va(0x9b2eb8),
  GrWarBolygo: va(0xbad140), AktKep: va(0xbae0fc), AktPlanet: va(0xb9933c), AktKepKell: va(0xbae250),
  WF0: va(0x9c3244),                                   // battlefield of planet p: WF0 + p * 0x8ec
};
const WF_SIZE = 0x8ec, UNIT = 0x290;

const cfg = { mode: 'war', scenario: 'scenario/quick.scn', startAfterFrames: 1200, count: 200, fuzz: false,
  every: 4, seed: 4242 };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 3) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

function hex(p, n) {
  const b = new Uint8Array(p.readByteArray(n));
  let s = '';
  for (let i = 0; i < b.length; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}
let fz = 1;
const rnd = (n) => { fz ^= fz << 13; fz >>>= 0; fz ^= fz >>> 17; fz ^= fz << 5; fz >>>= 0; return fz % n; };

const nf = (a, r, args, abi) => new NativeFunction(a, r, args, { abi: abi || 'mscdecl', traps: 'all' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const addTanks = nf(A.AddTanksToPlanet, 'void', ['int', 'int', 'int']);
const simulate = nf(A.Simulate, 'void', ['pointer'], 'thiscall');
const indit = nf(A.GroundWarIndit, 'void', ['int', 'int']);
const finished = nf(A.GroundWarFinished, 'void', ['int', 'int', 'int']);
const flottaNew = nf(A.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float']);
const hajoNew = nf(A.HajoNew, 'void', ['int', 'int']);
const tankNew = nf(A.TankNew, 'void', ['int', 'int', 'int']);
const crt = Process.getModuleByName('msvcr120.dll');
const srand = nf(crt.getExportByName('srand'), 'void', ['uint']);
const crand = nf(crt.getExportByName('rand'), 'int', []);
const fleetName = Memory.allocAnsiString('Lander');

const fleetPtr = (i) => A.Flottak.readPointer().add(0x80 * i);
const owner = (i) => fleetPtr(i).add(0x15).readU8();
const nFleets = () => A.FlottaCount.readS32();
const designs = (e) => A.Tervek.add(4 * e).readPointer();
const nDesigns = (e) => A.TervCount.add(4 * e).readS32();
const seeds = () => Array.from({ length: 21 }, (_, i) => A.RandSeed.add(4 * i).readU32());
const planetOwner = (p) => A.Bolygok.add(0x188 * p + 0x20).readU8();
const wf = (p) => A.WF0.add(WF_SIZE * p);

function classDesigns(e, cls) {
  const out = [];
  for (let d = 1; d <= nDesigns(e); d++) if (designs(e).add(0x84 * d + 0x16).readS16() === cls) out.push(d);
  return out;
}

// ---------------------------------------------------------------- snapshots
function snapField(p) {
  const w = wf(p), n = w.add(0x180).readS32();
  const alive = w.add(0x188).readPointer(), units = w.add(0x330).readPointer();
  const out = [];
  for (let i = 0; i < n; i++) {
    const a = alive.add(i).readU8();
    if (!a) { out.push({ i, alive: 0 }); continue; }
    const u = units.add(4 * i).readPointer();
    const size = u.add(0x190).readS32();
    const hp = [], origin = [];
    const hpp = u.add(0x154).readPointer(), orp = u.add(0x19c).readPointer();
    for (let k = 0; k < size && k < 10000; k++) { hp.push(hpp.add(4 * k).readS32()); origin.push(orp.add(4 * k).readS32()); }
    out.push({ i, alive: a, raw: hex(u, UNIT), hp, origin });
  }
  return { raw: hex(w, WF_SIZE), units: out };
}
function snapPlanet(p) {
  const ns = A.SlotCount.add(4 * p).readS32();
  return { raw: hex(A.Bolygok.add(0x188 * p), 0x188), slots: ns, count: A.BuildingCount.add(4 * p).readS32(),
    buildings: ns > 0 ? hex(A.Epuletek.add(4 * p).readPointer(), 0x20 * (ns + 1)) : '' };
}
function snapWorld(p, fleets) {
  const nt = A.TankCount.readS32();
  const s = { seeds: seeds(), tanks: hex(A.Tankok, 8 * (nt + 1)), ntanks: nt, field: snapField(p), planet: snapPlanet(p),
    timing: A.Timing.readS32(), battlesWon: hex(A.BattlesWon, 4 * 19) };
  if (fleets) s.fleets = hex(fleetPtr(0), 0x80 * (nFleets() + 1));
  return s;
}
function globals(p, o1, o2) {
  const des = {};
  for (const e of [o1, o2, planetOwner(p - 1)]) {
    if (e < 1 || e > 18 || e in des) continue;
    des[e] = hex(designs(e), 0x84 * (nDesigns(e) + 1));
  }
  return { en: A.EN.readS32(), mp: A.MP.readU8(), playerKi: hex(A.PlayerKi, 19), players2: A.Players2.readS32(),
    storyPlanet: A.StoryPlanet.readS32(), bonus: [A.Bonus1.readS32(), A.Bonus2.readS32()],
    prevOwner: planetOwner(p - 1), designs: des, nfleets: nFleets() };
}

// LCG draws are matched in the replay from (seed, r1, r2).
function pinnedCall(seed, fn) {
  srand(seed);
  fn();
  const r1 = crand(), r2 = crand();
  return { seed, r1, r2 };
}

// ---------------------------------------------------------------- staging
// The transporter pod 0x9f is rare: its partners make the exe fault (spec §8, the split partner's carrier).
const GROUND_PODS = [0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0xa0, 0xa1, 0xa2, 0xa3, 0xa4,
  0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad];
function fuzzDesign(e, d, saved) {
  const p = designs(e).add(0x84 * d);
  saved.push([p, p.readByteArray(0x84)]);
  for (let k = 0; k < 12; k++) {
    if (rnd(3) !== 0) continue;
    p.add(0x51 + k).writeU8(rnd(4) ? (rnd(40) === 0 ? 0x9f : GROUND_PODS[rnd(GROUND_PODS.length)]) : 0);
  }
}

function pickTarget() {
  const np = A.HanyBolygo.readS32();
  for (let k = 0; k < 60; k++) {
    const p = 2 + rnd(np - 1), o2 = planetOwner(p);
    if (o2 < 1 || o2 > 8) continue;
    const ens = [];
    for (let e = 1; e <= 8; e++) if (e !== o2 && classDesigns(e, 2).length) ens.push(e);
    if (!ens.length) continue;
    return [p, o2, ens[rnd(ens.length)]];
  }
  return null;
}

function stage(p, o1, o2, restore) {
  const q = A.Bolygok.add(0x188 * p);
  const f = flottaNew(fleetName, o1, 0, q.add(0x18).readFloat() + rnd(5) - 2, q.add(0x1c).readFloat() + rnd(5) - 2, 0);
  if (f < 1) return 0;
  const carriers = [];
  for (let d = 1; d <= nDesigns(o1); d++) {
    const r = designs(o1).add(0x84 * d);
    if (r.add(0x16).readS16() === 1 && r.add(0x28).readU8() > 0) carriers.push(d);
  }
  if (carriers.length && rnd(4)) for (let k = 0; k < 1 + rnd(2); k++) hajoNew(f, carriers[rnd(carriers.length)]);
  const g1 = classDesigns(o1, 2);
  const picks = [];
  for (let k = 0; k < 1 + rnd(3); k++) picks.push(g1[rnd(g1.length)]);
  if (cfg.fuzz) for (const d of picks) if (rnd(2)) fuzzDesign(o1, d, restore);
  for (const d of picks) tankNew(f, d, 1 + rnd(cfg.fuzz ? 12 : 6));
  const g2 = classDesigns(o2, 2);
  if (g2.length && rnd(3)) {
    for (let k = 0; k < 1 + rnd(3); k++) {
      const d = g2[rnd(g2.length)];
      if (cfg.fuzz && rnd(2)) fuzzDesign(o2, d, restore);
      tankNew(-p, d, 1 + rnd(5));
    }
  }
  if (cfg.fuzz) {
    const ns = A.SlotCount.add(4 * p).readS32();
    for (let k = 0; k < 3 && ns > 0; k++) {
      const b = A.Epuletek.add(4 * p).readPointer().add(0x20 * (1 + rnd(ns)));
      if (b.readU8() === 0) continue;
      if (rnd(3) === 0) b.add(0x19).writeU8(b.add(0x19).readU8() ^ (rnd(2) ? 1 : 0x20));
      if (rnd(3) === 0) b.add(4).writeS32(rnd(200));
    }
    if (rnd(4) === 0) {
      const ki = A.PlayerKi.add(o1);
      restore.push([ki, ki.readByteArray(1)]);
      ki.writeU8(1);
    }
    if (rnd(4) === 0) {
      for (const b of [A.Bonus1, A.Bonus2]) { restore.push([b, b.readByteArray(4)]); b.writeS32(rnd(60)); }
    }
  }
  return f;
}

function warProbe() {
  const t = pickTarget();
  if (!t) return false;
  const [p, o2, o1] = t;
  const restore = [];
  const f = stage(p, o1, o2, restore);
  if (f < 1) { for (const [a, b] of restore) a.writeByteArray(b); return false; }
  const g = globals(p, o1, o2);
  const before = snapWorld(p, true);
  const s1 = (rnd(65536) << 16 | rnd(65536)) >>> 0, s2 = (rnd(65536) << 16 | rnd(65536)) >>> 0;
  const r1 = pinnedCall(s1, () => addTanks(f, p, 1));
  const landed = snapWorld(p, false);
  landed.fleet = hex(fleetPtr(f), 0x80);
  // Fuzzed states can make the exe fault inside the war (original undefined behaviour): keep the input.
  if (cfg.fuzz) { emit({ k: 'pre', p, f, o1, o2, g, landed, s2 }); flush(); }
  const r2 = pinnedCall(s2, () => simulate(wf(p)));
  const after = snapWorld(p, true);
  emit({ k: 'war', p, f, o1, o2, g, before, landed, after, rand1: r1, rand2: r2 });
  for (const [a, b] of restore.reverse()) a.writeByteArray(b);
  return true;
}

// ---------------------------------------------------------------- indit
function inditProbe() {
  const t = pickTarget();
  if (!t) return false;
  const [p, o2, o1] = t;
  const restore = [];
  const f = stage(p, o1, o2, restore);
  if (f < 1) { for (const [a, b] of restore) a.writeByteArray(b); return false; }
  const ki = A.PlayerKi;
  for (const e of [o1, o2]) restore.push([ki.add(e), ki.add(e).readByteArray(1)]);
  if (rnd(3) === 0) ki.add(o1).writeU8(1);
  if (rnd(3) === 0) ki.add(o2).writeU8(1);
  if (cfg.fuzz) {
    for (const x of [A.DbgAlwaysSim, A.SimBan]) { restore.push([x, x.readByteArray(4)]); x.writeS32(rnd(6) === 0 ? 1 : 0); }
    const e = A.ErodEro.add(4 * p);
    restore.push([e, e.readByteArray(4)]);
    e.writeS32(rnd(3) ? 0 : rnd(400));
  }
  const g = globals(p, o1, o2);
  g.dbg = [A.DbgNoLop.readS32(), A.DbgLopOff.readS32(), A.DbgAlwaysSim.readS32(), A.SimBan.readS32()];
  g.erod = A.ErodEro.add(4 * p).readS32();
  g.mpStatus = A.MpBattleStatus.readS32();
  const before = snapWorld(p, true);
  A.GrWarBolygo.writeS32(-1);
  A.Timing.writeS32(-7);
  const patch = A.StartGrWar.readByteArray(1);
  Memory.patchCode(A.StartGrWar, 1, (c) => c.writeU8(0xc3));
  const kep = A.AktKepKell.readS32();
  emit({ k: 'start', p, f, o1, o2 }); flush();
  indit(f, p);
  Memory.patchCode(A.StartGrWar, 1, (c) => c.writeByteArray(patch));
  const realtime = A.GrWarBolygo.readS32() === p;
  const sim = A.Timing.readS32() !== -7;
  const after = snapWorld(p, true);
  A.AktKepKell.writeS32(kep);
  // Send the landed attackers home again (the simulated variant: no story or screen code), idle field.
  // `last status := owner`, else the next GroundWarSetBattleStatus starts the end-of-battle countdown, which
  // only runs down on the colony screen: a later WF::Simulate on this planet would never end (spec §5.1).
  if (realtime) {
    finished(p, planetOwner(p), 1);
    const w = wf(p);
    w.add(0x148).writeS32(-1); w.add(0x8c8).writeS32(planetOwner(p)); w.add(0x8cc).writeS32(0);
  }
  emit({ k: 'indit', p, f, o1, o2, g, before, after, realtime, sim });
  for (const [a, b] of restore.reverse()) a.writeByteArray(b);
  return true;
}

// ---------------------------------------------------------------- driver
let frames = 0, started = false, phase = 0, done = 0, loadedAt = 0, misses = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true; fz = cfg.seed;
      betolt(Memory.allocAnsiString(cfg.scenario));
      setSpeed(3, 1);
      loadedAt = frames;
      emit({ k: 'loaded' });
      return;
    }
    if (!started || frames < loadedAt + 300 || phase > 0) return;
    if (frames % cfg.every !== 0) return;
    try {
      const ok = cfg.mode === 'indit' ? inditProbe() : warProbe();
      if (ok) { done++; misses = 0; } else misses++;
      if (done >= cfg.count || misses > 200) { phase = 1; emit({ k: 'done', done }); }
    } catch (e) { emit({ k: 'error', msg: e.toString(), stack: e.stack }); emit({ k: 'done' }); phase = 1; }
  },
});

send({ type: 'ready', base: mod.base.toString() });
