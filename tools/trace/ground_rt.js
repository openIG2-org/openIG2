// Frida agent: the real-time ground war on the GOG ig2.exe (spec re/rules/ground-combat.md §11).
// Modes (config.mode; `battle` is documented at its section below):
//   place  - a staged real-time landing: AddTanksToPlanet(f, p, 0) (0x45e320) places every vehicle with
//            LocateTankSuitablePosition (0x486b90, landing zone 0xA34F50 + p*0xc0) and splits groups
//            (HalveGroupC); optionally StartGrWar(p) (0x4a4a70). Snapshots before / after: the battlefield
//            (every unit's 0x290-byte record with its hp/origin arrays), the landing zone, the planet and its
//            buildings, Tankok, fleets, streams; libc pinned by srand. The war is then undone with
//            GroundWarFinished(p, owner, 1) and an idle field (the off-screen countdown hazard, §5.1).
//   turret - EpuletNewSearchLocation(p, kind 0x1c..0x23, done, 0, -1, -1) (0x53cca0): AddBuildingTurrets
//            (0x45af60), AddTurret (0x45e670) with GetHeight (0x47ad60), BuildingBuilt (0x45faf0); then the
//            building is removed again (EpuletDelete 0x53c730).
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0),
  SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  AddTanksToPlanet: va(0x45e320), StartGrWar: va(0x4a4a70), GroundWarFinished: va(0x47c5d0),
  EpuletNewSearch: va(0x53cca0), EpuletDelete: va(0x53c730),
  FlottaNew: va(0x53d400), HajoNew: va(0x53e800), TankNew: va(0x541750),
  RandSeed: va(0xbaf2d0),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8),
  Tankok: va(0xbb41a8), TankCount: va(0xbb4154),
  Tervek: va(0xb9aa18), TervCount: va(0xb9c028),
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8),
  SlotCount: va(0xba9840), Epuletek: va(0xba9e90), BuildingCount: va(0xba9518),
  PlayerKi: va(0xbae560), EN: va(0x912a3c), MP: va(0xbae2c0),
  Players2: va(0x912ad0), StoryPlanet: va(0xf27438), Bonus1: va(0xf27468), Bonus2: va(0xf2746c),
  AktKep: va(0xbae0fc), AktPlanet: va(0xb9933c), Belep: va(0xbae24c), Atlepes: va(0xbae24d), AktKepKell: va(0xbae250), AktPlanetKell: va(0xbae254),
  WF0: va(0x9c3244),                                   // battlefield of planet p: WF0 + p * 0x8ec
  Landing0: va(0xa34f50),
  Terepek: va(0x99825c),                               // pointer to the colony maps (0x2714 B each)                              // landing zone of planet p: + p * 0xc0 (+0xb4 grid, +0xb8 made)
};
const WF_SIZE = 0x8ec, UNIT = 0x290;

const cfg = { human: false, destructAfter: 0, endAfter: 0, small: false, mode: 'place', scenario: 'scenario/quick.scn', startAfterFrames: 1200, count: 200, fuzz: false,
  every: 4, seed: 4242, ticks: 1500, steps: 100000, speed: 0 };
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
const startGrWar = nf(A.StartGrWar, 'void', ['int']);
const finished = nf(A.GroundWarFinished, 'void', ['int', 'int', 'int']);
const epuletSearch = nf(A.EpuletNewSearch, 'int', ['int', 'int', 'int', 'int', 'int', 'int']);
const epuletDelete = nf(A.EpuletDelete, 'void', ['int', 'int']);
const flottaNew = nf(A.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float']);
const hajoNew = nf(A.HajoNew, 'void', ['int', 'int']);
const tankNew = nf(A.TankNew, 'void', ['int', 'int', 'int']);
// WF::RemoveTankGroup / WF::RemoveTarget (thiscall on the battlefield), for --end-after.
const removeGroup = nf(va(0x497c90), 'void', ['pointer', 'int'], 'thiscall');
const removeTarget = nf(va(0x499070), 'void', ['pointer', 'int'], 'thiscall');
const crt = Process.getModuleByName('msvcr120.dll');
const srand = nf(crt.getExportByName('srand'), 'void', ['uint']);
const crand = nf(crt.getExportByName('rand'), 'int', []);
const fleetName = Memory.allocAnsiString('Lander');

const fleetPtr = (i) => A.Flottak.readPointer().add(0x80 * i);
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
function snapLanding(p) {
  const z = A.Landing0.add(0xc0 * p);
  const made = z.add(0xb8).readU8();
  return { made, grid: made ? hex(z.add(0xb4).readPointer(), 40000) : '' };
}
function snapPlanet(p) {
  const ns = A.SlotCount.add(4 * p).readS32();
  return { raw: hex(A.Bolygok.add(0x188 * p), 0x188), slots: ns, count: A.BuildingCount.add(4 * p).readS32(),
    buildings: ns > 0 ? hex(A.Epuletek.add(4 * p).readPointer(), 0x20 * (ns + 1)) : '' };
}
function snapWorld(p, fleets) {
  const nt = A.TankCount.readS32();
  const s = { seeds: seeds(), tanks: hex(A.Tankok, 8 * (nt + 1)), ntanks: nt, field: snapField(p), planet: snapPlanet(p),
    landing: snapLanding(p) };
  if (fleets) s.fleets = hex(fleetPtr(0), 0x80 * (nFleets() + 1));
  return s;
}
function globals(p, es) {
  const des = {};
  for (const e of es) {
    if (e < 1 || e > 18 || e in des) continue;
    des[e] = hex(designs(e), 0x84 * (nDesigns(e) + 1));
  }
  return { en: A.EN.readS32(), mp: A.MP.readU8(), playerKi: hex(A.PlayerKi, 19), players2: A.Players2.readS32(),
    storyPlanet: A.StoryPlanet.readS32(), bonus: [A.Bonus1.readS32(), A.Bonus2.readS32()],
    prevOwner: planetOwner(p - 1), designs: des, nfleets: nFleets(), aktKep: A.AktKep.readS32(),
    aktPlanet: A.AktPlanet.readS32(),
    aktMap: A.Bolygok.add(0x188 * A.AktPlanet.readS32() + 0x23).readU8(), terrain: terrains(p),
    belep: A.Belep.readU8(), atlepes: A.Atlepes.readU8() };
}
// The live colony maps of the battle planet and the viewed one (heights +0x138c, base +0x84): the game
// changes them at run time, so the file values are not enough.
const TEREP = 0x2714;
function terrains(p) {
  const out = {};
  for (const q of [p, A.AktPlanet.readS32()]) {
    const m = A.Bolygok.add(0x188 * q + 0x23).readU8();
    if (!(m in out)) out[m] = hex(A.Terepek.readPointer().add(TEREP * m), TEREP);
  }
  return out;
}
function pinnedCall(seed, fn) {
  srand(seed);
  fn();
  const r1 = crand(), r2 = crand();
  return { seed, r1, r2 };
}
const seed32 = () => (rnd(65536) << 16 | rnd(65536)) >>> 0;

// ---------------------------------------------------------------- staging (as ground.js; no transporter pods)
const GROUND_PODS = [0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0xa0, 0xa1, 0xa2, 0xa3, 0xa4,
  0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad];
function fuzzDesign(e, d, saved) {
  const p = designs(e).add(0x84 * d);
  saved.push([p, p.readByteArray(0x84)]);
  for (let k = 0; k < 12; k++) {
    if (rnd(3) !== 0) continue;
    p.add(0x51 + k).writeU8(rnd(4) ? GROUND_PODS[rnd(GROUND_PODS.length)] : 0);
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
  for (let k = 0; k < (cfg.small ? 1 : 1 + rnd(3)); k++) picks.push(g1[rnd(g1.length)]);
  if (cfg.fuzz) for (const d of picks) if (rnd(2)) fuzzDesign(o1, d, restore);
  for (const d of picks) tankNew(f, d, cfg.small ? 1 + rnd(3) : 1 + rnd(cfg.fuzz ? 25 : 8));
  const g2 = classDesigns(o2, 2);
  if (g2.length && (cfg.small ? rnd(3) === 0 : rnd(3))) {
    for (let k = 0; k < (cfg.small ? 1 : 1 + rnd(3)); k++) {
      const d = g2[rnd(g2.length)];
      if (cfg.fuzz && rnd(2)) fuzzDesign(o2, d, restore);
      tankNew(-p, d, cfg.small ? 1 : 1 + rnd(5));
    }
  }
  if (cfg.fuzz && rnd(4) === 0) {
    const ki = A.PlayerKi.add(o1);
    restore.push([ki, ki.readByteArray(1)]);
    ki.writeU8(1);
  }
  return f;
}
// The war is undone: the attackers go home (GroundWarFinished, simulated variant: no screen code), the
// field is idle and its last status is the owner (else a later status call starts the end countdown).
function undoWar(p) {
  finished(p, planetOwner(p), 1);
  const w = wf(p);
  w.add(0x148).writeS32(-1); w.add(0x8c8).writeS32(planetOwner(p)); w.add(0x8cc).writeS32(0);
}

function placeProbe() {
  const t = pickTarget();
  if (!t) return false;
  const [p, o2, o1] = t;
  const restore = [];
  const f = stage(p, o1, o2, restore);
  if (f < 1) { for (const [a, b] of restore) a.writeByteArray(b); return false; }
  const g = globals(p, [o1, o2, planetOwner(p - 1)]);
  const before = snapWorld(p, true);
  const r1 = pinnedCall(seed32(), () => addTanks(f, p, 0));
  const after = snapWorld(p, true);
  let start = null;
  if (rnd(2)) {
    const r2 = pinnedCall(seed32(), () => startGrWar(p));
    start = { rand: r2, after: snapWorld(p, true), kepKell: A.AktKepKell.readS32() };
  }
  emit({ k: 'place', p, f, o1, o2, g, before, after, rand: r1, start });
  undoWar(p);
  for (const [a, b] of restore.reverse()) a.writeByteArray(b);
  return true;
}

function turretProbe() {
  const np = A.HanyBolygo.readS32();
  for (let k = 0; k < 40; k++) {
    const p = 2 + rnd(np - 1), o = planetOwner(p);
    if (o < 1 || o > 8 || A.SlotCount.add(4 * p).readS32() < 1) continue;
    const kind = 0x1c + rnd(8), done = rnd(2);
    const g = globals(p, [o]);
    const before = snapWorld(p, false);
    let slot = 0;
    const r = pinnedCall(seed32(), () => { slot = epuletSearch(p, kind, done, 0, -1, -1); });
    const after = snapWorld(p, false);
    emit({ k: 'turret', p, kind, done, slot, g, before, after, rand: r });
    if (slot > 0) epuletDelete(p, slot);
    return true;
  }
  return false;
}

// ---------------------------------------------------------------- battle (the real-time tick)
// An AI-vs-AI war is landed in real time (AddTanksToPlanet(f, p, 0) + StartGrWar(p)), the colony screen of p
// is opened (AktKepKell 3 / AktPlanetKell p) and the clock runs: the game's own FRISSIT_Grwar calls WF::MI
// (0x486d40) and WF::Refresh (0x490890) every 20 Hz tick. Around each call: libc pinned with srand, and the
// state as region diffs against the last record (`init` once, then `pre` = what changed since the last call,
// `post` = what the call changed). Regions: the battlefield, every unit (record, hp, origins), the three
// projectile lists and their objects, the planet and its buildings, streams, Tankok, fleets, the landing
// zone, a few globals.
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const MI = va(0x486d40), REFRESH = va(0x490890);
const G = {
  SpeedBattle: va(0xbae260), KiMelyik: va(0x9abae8), Gvalue: va(0x9b2900), StoryHold: va(0xf261f8),
  Pause: va(0xbae264), Timing: va(0x9b2eb8), Atjaras: va(0xa33388),
};
const OBJ = 0x48;
function battleRegions(p) {
  const w = wf(p), n = w.add(0x180).readS32();
  const r = [['wf', w, WF_SIZE]];
  const alive = w.add(0x188).readPointer(), units = w.add(0x330).readPointer();
  if (n > 0) r.push(['alive', alive, n]);
  for (let i = 0; i < n; i++) {
    if (!alive.add(i).readU8()) continue;
    const u = units.add(4 * i).readPointer();
    r.push(['u' + i, u, UNIT]);
    const size = u.add(0x190).readS32();
    if (size > 0 && size < 10000) {
      r.push(['hp' + i, u.add(0x154).readPointer(), 4 * size], ['or' + i, u.add(0x19c).readPointer(), 4 * size]);
    }
    const nw = u.add(0x22c).readS32(), wp = u.add(0x234).readPointer();
    if (nw > 0 && nw < 1000 && !wp.isNull()) r.push(['wp' + i, wp, 8 * nw]);
  }
  // The planet's passability maps (PlanetAtjarasMap 0xA33388 + p*0x24: [1..5] 200x200 u16, made +0x1c).
  const atj = G.Atjaras.add(0x24 * p);
  r.push(['atjobj', atj, 0x24]);
  for (let k = 1; k <= 5; k++) {
    const m = atj.add(4 * k).readPointer();
    if (!m.isNull()) r.push(['atj' + k, m, 80000]);
  }
  for (const [name, po, co] of [['A', 0xd4, 0xe0], ['B', 0xd8, 0xe8], ['C', 0xdc, 0xe4]]) {
    const cnt = w.add(co).readS32(), arr = w.add(po).readPointer();
    if (cnt <= 0 || arr.isNull()) continue;
    r.push(['L' + name, arr, 4 * cnt]);
    for (let k = 0; k < cnt; k++) {
      const o = arr.add(4 * k).readPointer();
      if (!o.isNull()) r.push([name + k, o, OBJ]);
    }
  }
  const ns = A.SlotCount.add(4 * p).readS32();
  r.push(['planet', A.Bolygok.add(0x188 * p), 0x188], ['seeds', A.RandSeed, 84],
    ['tanks', A.Tankok, 8 * (A.TankCount.readS32() + 1)], ['fleets', fleetPtr(0), 0x80 * (nFleets() + 1)],
    ['kimelyik', G.KiMelyik, 72], ['bcount', A.BuildingCount.add(4 * p), 4]);
  if (ns > 0) r.push(['bld', A.Epuletek.add(4 * p).readPointer(), 0x20 * (ns + 1)]);
  const z = A.Landing0.add(0xc0 * p);
  if (z.add(0xb8).readU8()) r.push(['landing', z.add(0xb4).readPointer(), 40000]);
  return r;
}
// The colony screen's voice lines (ColonySpeaker, a SpeakingDirector at 0xA3EA3C; spec 11.9): per speaker
// (+0 first Speeches, +4 id, +8 next) and Speeches (+0 samples, +8 cooldown, +0xc kind, +0x10 next) the i32s
// (speaker, kind, samples, cooldown); then Lang (0x912724) and the rearrange mute byte 0x9B2EBC.
function voicesBytes() {
  const v = [va(0x912724).readS32(), va(0x9b2ebc).readU8()];
  let guard = 0;
  for (let sp = va(0xa3ea3c).readPointer(); !sp.isNull() && guard < 4096; sp = sp.add(8).readPointer()) {
    const id = sp.add(4).readS32();
    for (let ss = sp.readPointer(); !ss.isNull() && guard < 4096; ss = ss.add(0x10).readPointer(), guard++) {
      v.push(id, ss.add(0xc).readS32(), ss.readS32(), ss.add(8).readS32());
    }
  }
  return new Uint8Array(new Int32Array(v).buffer);
}
function readRegions(p) {
  const out = {};
  for (const [n, ptr, sz] of battleRegions(p)) out[n] = new Uint8Array(ptr.readByteArray(sz));
  out.voices = voicesBytes();
  return out;
}
function diffRegions(oldS, newS) {
  const d = {};
  for (const n in newS) {
    const a = oldS[n] || new Uint8Array(0), b = newS[n], runs = [];
    let i = 0;
    while (i < b.length) {
      if (i < a.length && a[i] === b[i]) { i++; continue; }
      let j = i;
      while (j < b.length && !(j < a.length && a[j] === b[j])) j++;
      runs.push([i, hexOf(b, i, j)]);
      i = j;
    }
    if (runs.length || a.length !== b.length) d[n] = { len: b.length, runs };
  }
  for (const n in oldS) if (!(n in newS)) d[n] = { len: -1, runs: [] };
  return d;
}
function hexOf(b, from, to) {
  let s = '';
  for (let i = from; i < to; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}
function tickGlobals() {
  return { speed: G.SpeedBattle.readS32(), aktKep: A.AktKep.readS32(), aktPlanet: A.AktPlanet.readS32(),
    belep: A.Belep.readU8(), atlepes: A.Atlepes.readU8(), gvalue: G.Gvalue.readS32(), hold: G.StoryHold.readS32(),
    nfleets: nFleets(), ntanks: A.TankCount.readS32() };
}

let battle = null, known = null, call = null, seq = 0, homeKep = 2, battles = 0, ticks = 0;
// --ai-fuzz (re/rules/ground-ai.md): before some WF::MI calls, perturb the AI's state so every plan, phase
// and the air strike get exercised: per-empire blocks (WF+0x33c+0x40e: strategy 0..6, 0 = choose again,
// mode, phase, timer, counter), hit markers (+0x25c/+0x260), air-strike charges (+0x7f4/+0x7f8 + 8e) and
// carriers (weapons 0x20 / 8, +0x288). The writes happen before the `pre` read, so the trace carries them.
function aiFuzz(p) {
  if (rnd(cfg.aiFuzzEvery || 25) !== 0) return;
  const w = wf(p), n = w.add(0x180).readS32();
  const alive = w.add(0x188).readPointer(), units = w.add(0x330).readPointer();
  const live = [];
  for (let i = 0; i < n; i++) if (alive.add(i).readU8()) live.push(i);
  if (!live.length) return;
  const owners = [...new Set(live.map((i) => units.add(4 * i).readPointer().add(0x194).readS32()))];
  for (const e of owners) {
    if (e < 1 || e > 18 || A.PlayerKi.add(e).readU8() === 1) continue;
    const o = w.add(0x33c + 0x40 * e);
    switch (rnd(4)) {
      case 0: o.writeS32(rnd(7)); break;                                     // strategy (0: choose again)
      case 1: o.writeS32(1 + rnd(6)); o.add(8).writeS32(1 + rnd(3)); break;  // strategy + phase
      case 2: o.add(4).writeS32(rnd(2) ? 1 : 3); o.add(0x24).writeS32(rnd(160) - 1); o.add(0x28).writeS32(rnd(13)); break;
      default: break;
    }
    if (rnd(3) === 0) {                                                       // an air strike
      const big = rnd(2), mine = live.filter((i) => units.add(4 * i).readPointer().add(0x194).readS32() === e);
      const u = units.add(4 * mine[rnd(mine.length)]).readPointer();
      if (!(u.add(0x188).readU32() & 0x4020002)) {
        u.add(0x1d0).writeU32(u.add(0x1d0).readU32() | (big ? 8 : 0x20));
        u.add(0x288).writeS32(1 + rnd(3));
        w.add(0x7f4 + 8 * e + 4 * big).writeS32(big ? 300 : 200);
      }
    }
  }
  for (let k = 0; k < 1 + rnd(4); k++) {                                    // fresh hits
    const u = units.add(4 * live[rnd(live.length)]).readPointer();
    u.add(0x25c).writeS32(rnd(3) ? 10000 : rnd(10000));
    u.add(0x260).writeS32(rnd(4) ? live[rnd(live.length)] : 10000);
  }
}
function hookCall(kind) {
  return {
    onEnter() {
      if (!battle || this.context.ecx.compare(wf(battle.p)) !== 0) { this.skip = true; return; }
      this.skip = false;
      if (kind === 'mi' && cfg.aiFuzz) aiFuzz(battle.p);
      const s = readRegions(battle.p);
      emitInit(s);
      // The libc pair the call would have seen (FRISSIT_Grwar's own draws end here), then the pin.
      const entry = { r1: crand(), r2: crand() };
      const seed = seed32();
      call = { kind, g: tickGlobals(), pre: diffRegions(known, s), seed, entry };
      known = s;
      srand(seed);
    },
    onLeave() {
      if (this.skip || !call) return;
      const r1 = crand(), r2 = crand();
      const s = readRegions(battle.p);
      emit({ k: kind, battle: battles, seq: seq++, tick: battle.ticks, g: call.g, pre: call.pre, post: diffRegions(known, s),
        rand: { seed: call.seed, r1, r2 }, entry: call.entry });
      known = s;
      call = null;
      if (kind === 'ref') { battle.ticks++; ticks++; }
    },
  };
}
Interceptor.attach(MI, hookCall('mi'));
Interceptor.attach(REFRESH, hookCall('ref'));
function emitInit(s) {
  if (known) return;
  const init = {};
  for (const n in s) init[n] = hexOf(s[n], 0, s[n].length);
  emit({ k: 'init', battle: battles, regions: init });
  known = s;
}
// FRISSIT_Grwar (0x4747c0) around MI and Refresh: its own part (self-destruct timers, WF::Collapse every
// 300th pass) is the diff from its entry to the first nested call (or to its exit).
Interceptor.attach(va(0x4747c0), {
  onEnter() {
    this.skip = !battle;
    if (this.skip) return;
    const s = readRegions(battle.p);
    emitInit(s);
    const seed = seed32();
    emit({ k: 'grwar', battle: battles, tick: battle.ticks, g: tickGlobals(), pre: diffRegions(known, s), seed });
    known = s;
    srand(seed);
  },
  onLeave() {
    if (this.skip || !battle) return;
    const s = readRegions(battle.p);
    emit({ k: 'grwar_end', battle: battles, post: diffRegions(known, s), entry: { r1: crand(), r2: crand() } });
    known = s;
  },
});

function pickBattle() {
  const np = A.HanyBolygo.readS32(), en = A.EN.readS32();
  const ai = (e) => e >= 1 && e <= 8 && e !== en && A.PlayerKi.add(e).readU8() !== 1;
  for (let k = 0; k < 80; k++) {
    const p = 2 + rnd(np - 1), o2 = planetOwner(p);
    if (!ai(o2) || A.SlotCount.add(4 * p).readS32() < 1) continue;
    const ens = [];
    for (let e = 1; e <= 8; e++) if (e !== o2 && ai(e) && classDesigns(e, 2).length) ens.push(e);
    if (cfg.human) ens.splice(0, ens.length, ...(classDesigns(en, 2).length ? [en] : []));
    if (!ens.length) continue;
    return [p, o2, ens[rnd(ens.length)]];
  }
  return null;
}
function startBattle() {
  const t = pickBattle();
  if (!t) return false;
  const [p, o2, o1] = t;
  const restore = [];
  const f = stage(p, o1, o2, restore);
  if (f < 1) { for (const [a, b] of restore) a.writeByteArray(b); return false; }
  if (cfg.speed) G.SpeedBattle.writeS32(cfg.speed);
  emit({ k: 'battle', battle: battles, p, f, o1, o2, g: globals(p, [o1, o2, planetOwner(p - 1)]) });
  addTanks(f, p, 0);
  startGrWar(p);
  A.AktKepKell.writeS32(3); A.AktPlanetKell.writeS32(p);
  battle = { p, f, o1, o2, ticks: 0, restore, frame: frames };
  known = null;
  return true;
}
function endBattle(why) {
  const p = battle.p, w = wf(p);
  emit({ k: 'end', battle: battles, why, ticks: battle.ticks });
  if (w.add(0x148).readS32() !== -1 || planetOwner(p) === 0 || why !== 'finished') undoWar(p);
  for (const [a, b] of battle.restore.reverse()) a.writeByteArray(b);
  battle = null; known = null; battles++;
  A.AktKepKell.writeS32(homeKep);
}
// Called from the frame hook while a battle runs: keep the clock going on its colony screen, end it when
// the game finished it (WF+0x190 set by Refresh's countdown) or after cfg.ticks ticks.
function watchBattle() {
  const w = wf(battle.p);
  if (A.AktKep.readS32() === 3 && A.AktPlanet.readS32() === battle.p && G.Pause.readU8()) setPause(0, 0);
  if (A.AktKep.readS32() === 3 && A.AktPlanet.readS32() !== battle.p) A.AktPlanetKell.writeS32(battle.p);
  if (w.add(0x190).readU8() && battle.ticks > 0) return endBattle('finished');
  // --end-after T: between two ticks the attacker's vehicles are removed the game's own way, so the
  // defender wins and Refresh runs the end countdown into GroundWarFinished.
  // --destruct-after T: the planet's self-destruct timer (WF+0x334) is set to 3 battle steps.
  if (cfg.destructAfter && battle.ticks >= cfg.destructAfter && !battle.destructed) {
    battle.destructed = true;
    w.add(0x334).writeS32(3);
    emit({ k: 'destruct', battle: battles, tick: battle.ticks });
  }
  if (cfg.endAfter && battle.ticks >= cfg.endAfter && !battle.ended) {
    battle.ended = true;
    const n = w.add(0x180).readS32(), alive = w.add(0x188).readPointer(), units = w.add(0x330).readPointer();
    const gone = [];
    for (let i = 0; i < n; i++) {
      if (!alive.add(i).readU8()) continue;
      const u = units.add(4 * i).readPointer();
      if (u.add(0x194).readS32() === battle.o1 && (u.add(0x188).readU32() & 0x4000000) === 0) gone.push(i);
    }
    for (const i of gone) { removeGroup(w, i); removeTarget(w, i); }
    emit({ k: 'removed', battle: battles, tick: battle.ticks, units: gone });
  }
  if (battle.ticks >= cfg.ticks) return endBattle('ticks');
  if (frames - battle.frame > 60 * 60 && battle.ticks === 0) return endBattle('stuck');
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
    if (cfg.mode === 'battle') {
      try {
        if (frames === loadedAt + 300) homeKep = A.AktKep.readS32();
        if (battle) { watchBattle(); return; }
        if (frames % cfg.every !== 0 || A.AktKep.readS32() !== homeKep) return;
        if (battles >= cfg.count || ticks >= cfg.steps) { phase = 1; emit({ k: 'done', battles, ticks }); return; }
        if (!startBattle()) misses++;
        if (misses > 200) { phase = 1; emit({ k: 'done', battles, ticks }); }
      } catch (e) { emit({ k: 'error', msg: e.toString(), stack: e.stack }); emit({ k: 'done' }); phase = 1; }
      return;
    }
    if (frames % cfg.every !== 0) return;
    try {
      const ok = cfg.mode === 'turret' ? turretProbe() : placeProbe();
      if (ok) { done++; misses = 0; } else misses++;
      if (done >= cfg.count || misses > 200) { phase = 1; emit({ k: 'done', done }); }
    } catch (e) { emit({ k: 'error', msg: e.toString(), stack: e.stack }); emit({ k: 'done' }); phase = 1; }
  },
});

send({ type: 'ready', base: mod.base.toString() });
