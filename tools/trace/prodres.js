// Frida agent: production and research queues on the GOG ig2.exe (spec re/rules/production-research.md).
// Modes (config.mode):
//   research   - before/after every FRISSIT_Research (0x51ddd0) call.
//   production - before/after the production block at the end of FRISSIT_Starmap (0x597322 .. 0x597aac).
// State is recorded as byte regions: one full dump ('init'), then per record the changes since the
// previous record's "after" ('pre', made by other systems or the fuzzer) and the changes the hooked code
// made ('post'). --fuzz perturbs research targets, labs, urgency, progress, money, race, difficulty and
// speed (research), or fills planet production queues with random entries, capacities, rally planets and
// delivery fleets and spawns idle fleets next to planets (production).
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0),
  SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  FRISSIT_Research: va(0x51ddd0), ProdStart: va(0x597322), ProdEnd: va(0x597aac),
  FlottaNew: va(0x53d400), HajoNew: va(0x53e800), FlottaMozgat: va(0x53d270),
  Ido2: va(0xbae050), Nap: va(0xbae048), IdoSpeed: va(0xbae25c), Nehezseg: va(0x92827c),
  EN: va(0x912a3c), MP: va(0xbae2c0), PP2: va(0x912ad0), StoryBonus: va(0xf27464),
  Cheat: va(0xb7727c), AllTechs: va(0xb77254), SpeedLock: va(0xbad0d4), ProgSpeed: va(0xbae54c),
  Techs: va(0xb9c0c8), RAF: va(0xb9a9b8), Urgent: va(0xb9c014), Labs: va(0x912728),
  Money: va(0xbad168), Pools: va(0xbad200), Hatar: va(0xbad590), Szorzo: va(0xbadcb0),
  PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574),
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8),
  Hajok: va(0xbb4100), HajoCount: va(0xbb4104),
  Tankok: va(0xbb41a8), TankCount: va(0xbb4154),
  Tervek: va(0xb9aa18), TervCount: va(0xb9c028),
  LastFleet: va(0xb99340), FleetNames: va(0xbb40b0),
  ColonyPool: va(0xbb4108), TerraPool: va(0xbb4158), AlapFlottaNev: va(0xbb6964),
};

const cfg = { mode: 'research', scenario: 'scenario/quick.scn', startAfterFrames: 1200, count: 3000,
  fuzz: false, seed: 4242, every: 3 };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 20) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

function hexOf(b, from, to) {
  let s = '';
  for (let i = from; i < to; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}
let fz = 1;
const rnd = (n) => { fz ^= fz << 13; fz >>>= 0; fz ^= fz >>> 17; fz ^= fz << 5; fz >>>= 0; return fz % n; };

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl', traps: 'all' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const flottaNew = nf(A.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float']);
const hajoNew = nf(A.HajoNew, 'void', ['int', 'int']);
const mozgat = nf(A.FlottaMozgat, 'void', ['int', 'int', 'int', 'float', 'float']);

const nFleets = () => A.FlottaCount.readS32();
const fleetPtr = (i) => A.Flottak.readPointer().add(0x80 * i);
const planet = (p) => A.Bolygok.add(0x188 * p);
const designs = (e) => A.Tervek.add(4 * e).readPointer();
const designCount = (e) => A.TervCount.add(4 * e).readS32();

// ---------------------------------------------------------------- regions
function regions() {
  const r = [
    ['money', A.Money, 19 * 8], ['pools', A.Pools, 19 * 48], ['hatar', A.Hatar, 19 * 48],
    ['szorzo', A.Szorzo, 19 * 48], ['ki', A.PlayerKi, 19], ['faj', A.PlayerFaj, 19],
  ];
  if (cfg.mode === 'research') {
    r.push(['techs', A.Techs, 193 * 0x114], ['raf', A.RAF, 19 * 4], ['urgent', A.Urgent, 19], ['labs', A.Labs, 19 * 16]);
  } else {
    const np = A.HanyBolygo.readS32(), nf_ = nFleets(), ns = A.HajoCount.readS32(), nt = A.TankCount.readS32();
    r.push(['planets', A.Bolygok, 0x188 * (np + 1)], ['fleets', A.Flottak.readPointer(), 0x80 * (nf_ + 1)],
      ['ships', A.Hajok.readPointer(), 6 * (ns + 1)], ['tanks', A.Tankok, 8 * Math.min(nt + 1, 1001)],
      ['last', A.LastFleet, 201 * 4], ['names', A.FleetNames, 19 * 4],
      ['cpool', A.ColonyPool, 19 * 4], ['tpool', A.TerraPool, 19 * 4]);
    for (let e = 1; e <= 18; e++) {
      const c = designCount(e), p = designs(e);
      if (c > 0 && !p.isNull()) r.push(['d' + e, p, 0x84 * (c + 1)]);
    }
  }
  return r;
}
let known = null; // name -> Uint8Array (state as of the last record's "after")
function read() {
  const out = {};
  for (const [n, p, sz] of regions()) out[n] = new Uint8Array(p.readByteArray(sz));
  return out;
}
function diff(oldS, newS) {
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
  return d;
}
function globals() {
  return { ido2: A.Ido2.readS32(), nap: A.Nap.readS32(), speed: A.IdoSpeed.readS32(), diff: A.Nehezseg.readS32(),
    en: A.EN.readS32(), mp: A.MP.readU8(), pp2: A.PP2.readS32(), bonus: A.StoryBonus.readS32(),
    cheat: A.Cheat.readS32(), all: A.AllTechs.readS32(), nfleets: nFleets(), nships: A.HajoCount.readS32(),
    ntanks: A.TankCount.readS32(), nplanets: A.HanyBolygo.readS32(),
    fleetName: A.AlapFlottaNev.readPointer().readCString(), locked: A.SpeedLock.readS32(), progSpeed: A.ProgSpeed.readS32() };
}

let seq = 0, before = null, restore = null;
function begin() {
  if (seq >= cfg.count) return false;
  if (cfg.fuzz) restore = (cfg.mode === 'research' ? fuzzResearch : fuzzProduction)();
  const s = read();
  if (!known) { const init = {}; for (const n in s) init[n] = hexOf(s[n], 0, s[n].length); emit({ k: 'init', regions: init }); known = s; }
  before = { g: globals(), pre: diff(known, s) };
  known = s;
  return true;
}
function end() {
  const s = read();
  emit({ k: cfg.mode === 'research' ? 'res' : 'prod', seq: seq++, g: before.g, pre: before.pre, post: diff(known, s) });
  known = s;
  before = null;
  if (restore) { restore(); restore = null; }
  if (seq >= cfg.count) emit({ k: 'done' });
}

// ---------------------------------------------------------------- fuzz
function fuzzResearch() {
  const saved = [];
  const keep = (p, n) => saved.push([p, p.readByteArray(n)]);
  if (rnd(3) === 0) { keep(A.Nehezseg, 4); A.Nehezseg.writeS32(rnd(5)); }
  if (rnd(4) === 0) { keep(A.StoryBonus, 4); A.StoryBonus.writeS32(rnd(3) ? 0 : rnd(150)); }
  if (rnd(25) === 0) { keep(A.Cheat, 4); A.Cheat.writeS32(1); }
  if (rnd(40) === 0) setSpeed(1 + rnd(3), 1);
  for (let k = 0; k < 3; k++) {
    const e = 1 + rnd(18);
    switch (rnd(9)) {
      case 0: A.RAF.add(4 * e).writeS32(rnd(4) ? 1 + rnd(192) : 0); break;
      case 1: A.Urgent.add(e).writeU8(rnd(3) ? 0 : 1 + rnd(2)); break;
      case 2: for (let i = 1; i <= 3; i++) A.Labs.add(16 * e + 4 * i).writeS32(rnd(12)); break;
      case 3: { const t = A.RAF.add(4 * e).readS32(); if (t > 0) {
        const lim = A.Techs.add(0x114 * t + 0x38).readS32();
        A.Techs.add(0x114 * t + 0x58 + 4 * e).writeS32(rnd(2) ? lim - rnd(40) : rnd(Math.max(1, lim))); } break; }
      case 4: { const t = A.RAF.add(4 * e).readS32(); if (t > 0) A.Techs.add(0x114 * t + 0x45 + e).writeU8(1 + rnd(5)); break; }
      case 5: { const pl = A.Pools.add(48 * e + 24); pl.writeDouble(rnd(2) ? rnd(200) : rnd(200000));
        const pools = A.Pools.add(48 * e);
        A.Money.add(8 * e).writeDouble(pools.add(8).readDouble() + 0 + pools.add(16).readDouble() + pools.add(24).readDouble()); break; }
      case 6: keep(A.PlayerFaj.add(e), 1); A.PlayerFaj.add(e).writeU8(1 + rnd(14)); break;
      case 7: // more AI candidates: random techs researchable, labs up
        if (A.PlayerKi.add(e).readU8() !== 1) {
          for (let j = 0; j < 6; j++) { const t = 1 + rnd(192); const s = A.Techs.add(0x114 * t + 0x45 + e); if (s.readU8() < 4) s.writeU8(3); }
          A.RAF.add(4 * e).writeS32(rnd(2) ? 0 : A.RAF.add(4 * e).readS32());
        }
        break;
      default: break;
    }
  }
  return () => { for (const [p, b] of saved) p.writeByteArray(b); };
}

function ownedPlanets() {
  const out = [], np = A.HanyBolygo.readS32();
  for (let p = 1; p <= np; p++) { const o = planet(p).add(0x20).readU8(); if (o >= 1 && o <= 18 && designCount(o) > 0 && !designs(o).isNull()) out.push(p); }
  return out;
}
function pickDesign(e, cls) {
  const c = designCount(e), list = [];
  for (let d = 1; d <= c; d++) {
    const p = designs(e).add(0x84 * d), k = p.add(0x16).readS16();
    if (p.add(0x20).readS32() <= 0) continue;
    if (cls < 0 || k === cls) list.push(d);
  }
  return list.length ? list[rnd(list.length)] : 0;
}
function fuzzProduction() {
  const saved = [];
  const keep = (p, n) => saved.push([p, p.readByteArray(n)]);
  if (rnd(3) === 0) { keep(A.Nehezseg, 4); A.Nehezseg.writeS32(rnd(5)); }
  if (rnd(60) === 0) setSpeed(1 + rnd(3), 1);
  const mine = ownedPlanets();
  if (!mine.length) return () => {};
  for (let k = 0; k < 3; k++) {
    const p = mine[rnd(mine.length)], b = planet(p), o = b.add(0x20).readU8();
    switch (rnd(10)) {
      case 0: case 1: case 2: { // a fresh queue line
        const L = rnd(2);
        for (let s = 0; s < 4; s++) {
          const d = rnd(5) === 0 ? 0 : pickDesign(o, rnd(4) === 0 ? -1 : (L === 0 ? 1 : 2));
          b.add(0x78 + 8 * L + 2 * s).writeS16(d);
          b.add(0x88 + 4 * L + s).writeU8(rnd(6) === 0 ? 0 : 1 + rnd(3));
          b.add(0x90 + 4 * L + s).writeU8(rnd(3) === 0 ? 1 + rnd(2) : 0);
          b.add(0x98 + 4 * L + s).writeU8(rnd(256));
          b.add(0xad + 4 * L + s).writeU8(rnd(2));
        }
        b.add(0xa0 + 4 * L).writeS32(rnd(3) ? 1 + rnd(60) : rnd(3000));
        b.add(0xa8 + L).writeU8(rnd(2));
        break;
      }
      case 3: b.add(0x70 + 4 * rnd(2)).writeS32(rnd(4) ? rnd(3000) : rnd(200000)); break;
      case 4: b.add(0xac).writeU8(rnd(2) ? 0 : 1 + rnd(A.HanyBolygo.readS32())); break;
      case 5: b.add(0x64).writeU8(rnd(4) ? 0 : 1 + rnd(3)); break;
      case 6: A.LastFleet.add(4 * p).writeS32(rnd(3) ? 0 : 1 + rnd(Math.max(1, nFleets()))); break;
      case 7: if (b.add(0x114).readS32() > 6 || rnd(3) === 0) b.add(0x114).writeS32(rnd(4)); break;
      case 8: { const pl = A.Pools.add(48 * o + 8); pl.writeDouble(rnd(2) ? rnd(300) : 1e6);
        const pools = A.Pools.add(48 * o);
        A.Money.add(8 * o).writeDouble(pools.add(8).readDouble() + 0 + pools.add(16).readDouble() + pools.add(24).readDouble()); break; }
      default: spawnNear(p, o); break;
    }
  }
  return () => { for (const [p, b] of saved) p.writeByteArray(b); };
}
const probeName = Memory.allocAnsiString('Probe');
function spawnNear(p, o) {
  if (nFleets() > 400) return;
  const b = planet(p), d = pickDesign(o, 1);
  if (!d) return;
  const f = flottaNew(rnd(2) ? probeName : A.AlapFlottaNev.readPointer(), o, 0,
    b.add(0x18).readFloat() + (rnd(13) - 6) * 0.7, b.add(0x1c).readFloat() + (rnd(13) - 6) * 0.7, 0);
  if (f < 1) return;
  for (let j = 0; j < 1 + rnd(3); j++) hajoNew(f, d);
  if (rnd(3) === 0) mozgat(f, 1, 1, p, 0);
  else if (rnd(5) === 0) mozgat(f, 1, 0, b.add(0x18).readFloat() + 50, b.add(0x1c).readFloat());
}

// ---------------------------------------------------------------- hooks
let phase = 0;
Interceptor.attach(A.FRISSIT_Research, {
  onEnter() { this.on = cfg.mode === 'research' && phase === 1 && begin(); },
  onLeave() { if (this.on) end(); },
});
let prodOn = false;
Interceptor.attach(A.ProdStart, function () { prodOn = cfg.mode === 'production' && phase === 1 && begin(); });
Interceptor.attach(A.ProdEnd, function () { if (prodOn) { prodOn = false; end(); } });

let frames = 0, started = false, loadedAt = 0;
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
    if (started && phase === 0 && frames >= loadedAt + 300) phase = 1;
    if (phase === 1 && seq >= cfg.count) phase = 2;
  },
});

send({ type: 'ready', base: mod.base.toString() });
