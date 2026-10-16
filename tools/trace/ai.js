// Frida agent: the AI empire's daily pass on the GOG ig2.exe (spec re/rules/ai-empire.md).
// Every FRISSIT_MI (exe 0x50a6e0) call the game makes becomes one 'mi' record: 'pre' = byte-region changes since
// the previous record (other systems, the fuzzer), then checkpoints with the changes since the previous
// checkpoint: entering/leaving FRISSIT_MI_SZAMOLAS (0x50dd00), and per AI empire entering/leaving
// FRISSIT_MI_GYARTAS (0x50c6d0), FRISSIT_MI_KEMKEDES (0x50d0d0) and FRISSIT_MI_FEJLESZTES (0x50b0b0), then the
// end of FRISSIT_MI. The replay checks each ported segment between two checkpoints (KOLONIZALAS and FLOTTAK,
// not ported, fall between KEMKEDES/GYARTAS leave and FEJLESZTES enter).
// Part 2 adds checkpoints entering/leaving FRISSIT_MI_KOLONIZALAS (0x50d9a0), FRISSIT_MI_FLOTTAK (0x50b1c0),
// FRISSIT_MI_DESIGN (0x50abf0, nested in the other two) and ElkuldHaduzenet (0x4c09c0, the war declaration).
// --fuzz: set-up revives empires 1..8 as AIs (quick.scn has two living empires) with built-up planets; before each
// call it rewrites danger states, spies (states, skill points, experience, ranks), spy capacities and centres,
// pool 4, relation levels and contacts, the AI's design slots, the colony-ship flag, idle production lines,
// garrisons, ground strength, unrest, lab counts, main planet/fleet, sliders, and temporarily the day (Nap 20:
// scrapping), the difficulty, planet 1's owner and tech 1 (the AI's planet/tech parameter is always 1) (restored
// after the call). --ships adds warships (main fleets, scrapping, ship missions), slows one design now and then
// (scrapping) and hands the human's empire to the AI.
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  MI: va(0x50a6e0), Szamolas: va(0x50dd00), Gyartas: va(0x50c6d0), Kemkedes: va(0x50d0d0), Kolonizalas: va(0x50d9a0),
  Flottak_: va(0x50b1c0), Fejlesztes: va(0x50b0b0), Design: va(0x50abf0), War: va(0x4c09c0),
  // Part 2: the AI's day counters, scouting grid, diplomacy's power and planet counts, story flags.
  KolUtso: va(0xdb05e8), NoLoad: va(0xdb0638), Phase: va(0xdb0688), NoUpg: va(0xdb06d8), Racs: va(0xdb0738),
  Ero: va(0xbaea78), HanyB: va(0xbaeb18), Tamad: va(0x915f44), Erod: va(0xb99cb8), Radar2: va(0xbb61a8),
  RadarSz: va(0xbb6668), Sajat: va(0xbb40b0), StoryB0: va(0xf274a8), StoryB8: va(0xf274b0), StoryBC: va(0xf274b4),
  Spare: va(0xf274c8), DesignDebug: va(0xa3f0b0), AlapNev: va(0xbb6964), Waypointek: va(0xbb41a4),
  WaypointCount: va(0xbb60f0), TankNew: va(0x541750),
  // AI tables.
  FoBolygo: va(0xdb0548), FoFlotta: va(0xdb0598), KozX: va(0xdb2848), KozY: va(0xdb2898), Veszely: va(0xdb27f8),
  Mutato: va(0xdaf668), MutatoSpec: va(0xdafa20), Cuccok: va(0xdb28e8), UresB: va(0xdb0724), Csuszka: va(0xbad920),
  // Empires, money, techs, relations.
  El: va(0xbae6ac), MIzik: va(0x915f58), PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574), PP: va(0x912acc), PP2: va(0x912ad0),
  EN: va(0x912a3c), MP: va(0xbae2c0), Nehezseg: va(0x92827c), Nap: va(0xbae048), Honap: va(0xbae044), Ev: va(0xbae040),
  Ido: va(0xbae04c), SpyEn: va(0xbae3af), AiOff: va(0xb77244), StoryAc: va(0xf274a4), AllTechs: va(0xb77254),
  Money: va(0xbad168), Pools: va(0xbad200), Hatar: va(0xbad590), Szorzo: va(0xbadcb0), FullGDP: va(0xbae060),
  Techs: va(0xb9c0c8), Kutat: va(0xb9a9b8), Viszony: va(0xdabf38), Ismeri: va(0xbaebb8), Miota: va(0xbaed28),
  Labs: va(0x912728), LabsAll: va(0xba90e0), RandSeed: va(0xbaf2d0), RndState: va(0xbaeb64), RndOssz: va(0xbaebb4),
  // Spies.
  Kemek: va(0xbafcc8), Ossz: va(0xbb1f84), Kuld: va(0x912f48), Kidnap: va(0xbb405c), SpyCap: va(0xbb2388),
  SpyAll: va(0xbb23ac), SpyPlan: va(0xbb2418),
  // Planets, buildings, units.
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8), Lazad: va(0xb99668), Foldi: va(0xb99990),
  HanyEpulet: va(0xba9518), HanyEpuletLefoglalva: va(0xba9840), Epuletek: va(0xba9e90),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8), Hajok: va(0xbb4100), HajoCount: va(0xbb4104),
  Tankok: va(0xbb41a8), TankCount: va(0xbb4154), Tervek: va(0xb9aa18), TervCount: va(0xb9c028),
  CPool: va(0xbb4108), TPool: va(0xbb4158),
  AktKep: va(0xbae0fc), AktKepKell: va(0xbae250), Speed: va(0xbae25c),
  FlottaNew: va(0x53d400), HajoNew: va(0x53e800),
  // Callees (coverage).
  HajoDelete: va(0x53e5d0), MoneyLog: va(0x53fb10), Search2spy: va(0x6040a0), RndValasztas: va(0x540f10),
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, count: 400, fuzz: false, seed: 4242, debug: false,
  ships: false, aiHuman: false };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 4) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

function hexOf(b, from, to) {
  let s = '';
  for (let i = from; i < to; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}
let fz = 1;
const rnd = (n) => { fz ^= fz << 13; fz >>>= 0; fz ^= fz >>> 17; fz ^= fz << 5; fz >>>= 0; return fz % n; };

const nf = (p, r, a) => new NativeFunction(p, r, a, { abi: 'mscdecl' });
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const flottaNew = nf(A.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float']);
const hajoNew = nf(A.HajoNew, 'void', ['int', 'int']);
const tankNew = nf(A.TankNew, 'void', ['int', 'int', 'int']);

const nPlanets = () => A.HanyBolygo.readS32();
const nFleets = () => A.FlottaCount.readS32();
const nShips = () => A.HajoCount.readS32();
const spyAt = (e, k) => A.Kemek.add(e * 0x3dc + k * 0x4c);
const designs = (e) => A.Tervek.add(4 * e).readPointer();
const nDesigns = (e) => A.TervCount.add(4 * e).readS32();
const owner = (p) => A.Bolygok.add(0x188 * p + 0x20).readU8();
const planet = (p) => A.Bolygok.add(0x188 * p);

// ---------------------------------------------------------------- regions
function regions() {
  const np = nPlanets(), nfl = nFleets(), ns = nShips(), nt = A.TankCount.readS32();
  const out = [
    ['fobolygo', A.FoBolygo, 76], ['foflotta', A.FoFlotta, 76], ['kozx', A.KozX, 76], ['kozy', A.KozY, 76],
    ['veszely', A.Veszely, 76], ['mutato', A.Mutato, 19 * 0x32], ['mutspec', A.MutatoSpec, 19 * 0x32],
    ['cuccok', A.Cuccok, 19 * 0x18c], ['uresb', A.UresB, 19], ['csuszka', A.Csuszka, 19 * 48],
    ['el', A.El, 19], ['mizik', A.MIzik, 19], ['money', A.Money, 19 * 8], ['pools', A.Pools, 19 * 48],
    ['hatar', A.Hatar, 19 * 48], ['szorzo', A.Szorzo, 19 * 48], ['fullgdp', A.FullGDP, 76],
    ['techs', A.Techs, 193 * 0x114], ['kutat', A.Kutat, 76], ['visz', A.Viszony, 19 * 0x2d2], ['ismeri', A.Ismeri, 361],
    ['miota', A.Miota, 19 * 19 * 4], ['labs', A.Labs, 19 * 16], ['labsall', A.LabsAll, 19 * 16], ['seeds', A.RandSeed, 21 * 4],
    ['rnd', A.RndState, 4], ['rndossz', A.RndOssz, 4],
    ['kemek', A.Kemek, 9 * 0x3dc], ['ossz', A.Ossz, 36], ['kuld', A.Kuld, 27 * 0x20], ['kidnap', A.Kidnap, 72],
    ['spycap', A.SpyCap, 36], ['spyall', A.SpyAll, 36], ['spyplan', A.SpyPlan, 9 * 201 * 4],
    ['planets', A.Bolygok, 0x188 * (np + 1)], ['lazad', A.Lazad, 4 * (np + 1)], ['foldi', A.Foldi, 4 * (np + 1)],
    ['hanyep', A.HanyEpulet, 4 * (np + 1)], ['nb', A.HanyEpuletLefoglalva, 4 * (np + 1)],
    ['fleets', A.Flottak.readPointer(), 0x80 * (nfl + 1)], ['ships', A.Hajok.readPointer(), 6 * (ns + 1)],
    ['tanks', A.Tankok, 8 * (Math.max(nt, 0) + 1)], ['cpool', A.CPool, 76], ['tpool', A.TPool, 76],
    ['kolutso', A.KolUtso, 76], ['nopak', A.NoLoad, 76], ['pakfaz', A.Phase, 76], ['noupg', A.NoUpg, 76],
    ['racs', A.Racs, 19 * 441], ['ero', A.Ero, 76], ['hanyb', A.HanyB, 76], ['tamad', A.Tamad, 19],
    ['erod', A.Erod, 4 * (np + 1)], ['radar2', A.Radar2, 19 * 32], ['radarsz', A.RadarSz, 76], ['sajat', A.Sajat, 76],
    ['spare', A.Spare, 76],
  ];
  const nw = A.WaypointCount.readS32(), wp = A.Waypointek.readPointer();
  out.push(['wp', wp, nw > 0 && !wp.isNull() ? 12 * (nw + 1) : 0]);
  for (let e = 1; e <= 18; e++) {
    const n = nDesigns(e), p = designs(e);
    out.push(['des' + e, p, n > 0 && !p.isNull() ? 0x84 * (n + 1) : 0]);
  }
  return out;
}
let known = null;
function read() {
  const out = {};
  for (const [n, p, sz] of regions()) out[n] = sz > 0 ? new Uint8Array(p.readByteArray(sz)) : new Uint8Array(0);
  // The building slots of planets 1..n, concatenated (slot counts in 'nb'; arrays are realloc'ed).
  const np = nPlanets(), parts = [];
  let total = 0;
  for (let p = 1; p <= np; p++) {
    const nb = A.HanyEpuletLefoglalva.add(4 * p).readS32(), arr = A.Epuletek.add(4 * p).readPointer();
    const b = nb > 0 && !arr.isNull() ? new Uint8Array(arr.add(0x20).readByteArray(nb * 0x20)) : new Uint8Array(0);
    parts.push(b); total += b.length;
  }
  const bld = new Uint8Array(total);
  let o = 0;
  for (const b of parts) { bld.set(b, o); o += b.length; }
  out.bld = bld;
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
  return { ido: A.Ido.readS32(), nap: A.Nap.readS32(), honap: A.Honap.readS32(), ev: A.Ev.readS32(), pp: A.PP.readS32(),
    pp2: A.PP2.readS32(), en: A.EN.readS32(), mp: A.MP.readU8(), diff: A.Nehezseg.readS32(), spyen: A.SpyEn.readU8(),
    storyac: A.StoryAc.readS32(), aioff: A.AiOff.readU8(), alltechs: A.AllTechs.readS32(), nplanets: nPlanets(),
    storyb0: A.StoryB0.readS32(), storyb8: A.StoryB8.readS32(), storybc: A.StoryBC.readS32(),
    ddebug: A.DesignDebug.readU8(), alapnev: A.AlapNev.readPointer().readCString(),
    ki: hexOf(new Uint8Array(A.PlayerKi.readByteArray(19)), 0, 19), faj: hexOf(new Uint8Array(A.PlayerFaj.readByteArray(19)), 0, 19) };
}

// ---------------------------------------------------------------- fuzz
const aiEmpires = () => {
  const out = [], pp2 = A.PP2.readS32();
  for (let e = 1; e <= pp2; e++) if (A.El.add(e).readU8() && A.PlayerKi.add(e).readU8() !== 1 && A.MIzik.add(e).readU8()) out.push(e);
  return out;
};
const alive8 = () => { const out = []; for (let e = 1; e <= 8; e++) if (A.El.add(e).readU8()) out.push(e); return out; };
function planetsOf(t) { const out = []; for (let p = 1; p <= nPlanets(); p++) if (owner(p) === t) out.push(p); return out; }
function fleetsOf(e) {
  const out = [], f = A.Flottak.readPointer();
  for (let i = 1; i <= nFleets(); i++) if (f.add(0x80 * i + 0x15).readU8() === e) out.push(i);
  return out;
}
function fuzzSpies(e) {
  const n = Math.min(12, A.Ossz.add(4 * e).readS32() + rnd(3));
  A.Ossz.add(4 * e).writeS32(n);
  for (let k = 1; k <= n; k++) {
    const s = spyAt(e, k);
    if (rnd(3) === 0 && s.add(0x10).readS16() > 0) continue;
    const st = [1, 1, 1, 3, 3, 2, 0][rnd(rnd(8) === 0 ? 7 : 5)];
    s.add(0x10).writeS16(st); s.add(0x16).writeS16(0);
    const m = st === 3 ? (rnd(3) ? 13 : 1 + rnd(12)) : (rnd(4) === 0 ? 13 : 0);
    s.add(0x1a).writeU16(m);
    if (st === 3 && m !== 13) { s.add(0x1c).writeU8(1 + rnd(8)); s.add(0x24).writeS32(500); s.add(0x28).writeS32(100 + rnd(400)); s.add(0x2c).writeS32(-1000); }
    if (st === 3 && m === 13) { s.add(0x1c).writeU8(0); s.add(0x28).writeS32(500); s.add(0x2c).writeS32(-1000); }
    for (let i = 0x31; i <= 0x34; i++) s.add(i).writeU8(rnd(4) === 0 ? rnd(40) : 1 + rnd(12));
    s.add(0x38).writeS32(rnd(3) ? 0 : rnd(6));
    s.add(0x3c).writeS32(rnd(3) ? 1500 + rnd(1000) : rnd(30000));
    s.add(0x40).writeU8(1 + rnd(20));
    s.add(0x12).writeU8(1 + rnd(14));
  }
}
function fuzz(saved) {
  const ais = aiEmpires(), al = alive8(), np = nPlanets();
  const tmp = (p, size, v) => { saved.push([p, p.readByteArray(size)]); if (size === 4) p.writeS32(v); else p.writeU8(v); };
  for (const e of ais) {
    if (rnd(2)) A.Veszely.add(4 * e).writeS32(1 + rnd(5));
    if (e <= 8 && rnd(2)) fuzzSpies(e);
    if (e <= 8 && rnd(3) === 0) A.SpyCap.add(4 * e).writeS32(1 + rnd(12));
    if (e <= 8 && rnd(4) === 0) A.SpyAll.add(4 * e).writeS32(rnd(14));
    if (rnd(3) === 0) A.Pools.add(48 * e + 32).writeDouble(rnd(4) === 0 ? -rnd(3000) : rnd(20000));
    for (const t of al) {
      if (t === e || rnd(3)) continue;
      A.Ismeri.add(e * 19 + t).writeU8(rnd(5) ? 1 : 0);
      A.Viszony.add(e * 0x2d2 + t * 0x26 + 0x25).writeU8(1 + rnd(5));
    }
    // Design slots: live designs of e built on the slot's hull, as FRISSIT_MI_DESIGN sets them (arbitrary records
    // crash TervAutoDesign/TervUjraSzamol once the upgrade timer runs out).
    const nd = nDesigns(e);
    if (nd > 0 && rnd(2)) {
      for (let k = 0; k < 6; k++) {
        const s = 1 + rnd(24), tab = rnd(2) ? A.Mutato : A.MutatoSpec;
        const fit = designsWhere(e, (p) => p.add(0x44).readS32() === s);
        tab.add(e * 0x32 + 2 * s).writeS16(rnd(4) === 0 || !fit.length ? 0 : fit[rnd(fit.length)]);
      }
    }
    if (rnd(3) === 0) A.UresB.add(e).writeU8(rnd(2));
    if (rnd(4) === 0) for (let i = 1; i <= 3; i++) A.LabsAll.add(e * 16 + 4 * i).writeS32(rnd(4) === 0 ? 10 + rnd(15) : rnd(12));
    const mine = planetsOf(e);
    if (mine.length && rnd(4) === 0) A.FoBolygo.add(4 * e).writeS32(rnd(4) === 0 ? 0 : mine[rnd(mine.length)]);
    const fl = fleetsOf(e);
    if (rnd(5) === 0) A.FoFlotta.add(4 * e).writeS32(fl.length && rnd(2) ? fl[rnd(fl.length)] : 0);
    for (const p of mine) {
      const q = planet(p);
      if (rnd(3) === 0) { q.add(0x78).writeS16(0); }
      if (rnd(3) === 0) { q.add(0x80).writeS16(0); }
      if (rnd(5) === 0) q.add(0x70).writeS32(rnd(3) ? 0 : 1 + rnd(3));
      if (rnd(5) === 0) q.add(0x74).writeS32(rnd(3) ? 0 : 1 + rnd(3));
      if (rnd(5) === 0) q.add(0x58).writeS32(rnd(15));
      if (rnd(5) === 0) A.Foldi.add(4 * p).writeS32(rnd(30));
      if (rnd(6) === 0) A.Lazad.add(4 * p).writeS32(rnd(3) ? 0 : 1 + rnd(5));
      if (e <= 8 && rnd(6) === 0) A.SpyPlan.add(4 * (e * 201 + p)).writeS32(rnd(2));
    }
  }
  // Sliders of the other empires (the human's come from the money screen): never all zero (the exe would spin).
  for (let e = 1; e <= 18; e++) {
    if (rnd(6)) continue;
    for (let i = 0; i < 6; i++) A.Csuszka.add(48 * e + 8 * i).writeDouble(i >= 1 && i <= 3 ? 1 + rnd(60) : rnd(30));
  }
  if (rnd(3) === 0) tmp(A.Nap, 4, 20);
  if (rnd(2) === 0) tmp(A.Nehezseg, 4, 1 + rnd(4));
  // The AI passes p1 = 1 (a boolean) as the planet / tech parameter: planet 1 must be the target's (mission 9)
  // and tech 1 missing (mission 8). Make that true now and then, for this call only.
  if (rnd(4) === 0 && al.length) tmp(planet(1).add(0x20), 1, al[rnd(al.length)]);
  for (const e of ais) if (e <= 8 && rnd(4) === 0) tmp(A.Techs.add(0x114 + 0x45 + e), 1, 3);
  // Scrapping (day 20) needs a bucket with < 10 % of the strength: make one design of an AI's ships slow.
  if (cfg.ships && rnd(3) === 0 && ais.length) {
    const e = ais[rnd(ais.length)], sh = A.Hajok.readPointer(), f = A.Flottak.readPointer(), used = {};
    for (let i = 1; i <= nShips(); i++) {
      const d = sh.add(6 * i + 2).readS16(), fl = sh.add(6 * i).readS16();
      if (d !== 0x7fff && fl > 0 && f.add(0x80 * fl + 0x15).readU8() === e) used[d] = (used[d] || 0) + 1;
    }
    const ds = Object.keys(used).map(Number).sort((x, y) => used[x] - used[y]);
    if (ds.length > 1) designs(e).add(0x84 * ds[0] + 0x3c).writeFloat(0.005 * (1 + rnd(3)));
  }
  void np;
  fuzz2(tmp, saved);
}

// Part 2 (colonisation, fleets, design): day counters, colony ships and free planets in sight, ground vehicles
// on planets and in fleets, contact ages, foes near the AI's fleets, fleet speeds, the story's guards.
const designsWhere = (e, test) => {
  const out = [];
  for (let d = 1; d <= nDesigns(e); d++) { const p = designs(e).add(0x84 * d); if (p.add(0x16).readS16() >= 0 && test(p)) out.push(d); }
  return out;
};
function fuzz2(tmp, saved) {
  const ais = aiEmpires(), al = alive8(), np = nPlanets(), fp = () => A.Flottak.readPointer();
  for (const e of ais) {
    if (rnd(3) === 0) A.NoLoad.add(4 * e).writeS32(rnd(3) ? 0 : rnd(5));
    if (rnd(3) === 0) A.Phase.add(4 * e).writeS32(rnd(3) ? 0 : rnd(5));
    if (rnd(4) === 0) A.NoUpg.add(4 * e).writeS32(rnd(3) ? 1 : rnd(4));
    if (rnd(6) === 0) A.KolUtso.add(4 * e).writeS32(rnd(np + 1));
    if (rnd(6) === 0) for (let k = 0; k < 30; k++) A.Racs.add(e * 441 + rnd(441)).writeU8(rnd(3));
    if (rnd(8) === 0) A.Tamad.add(e).writeU8(rnd(4) ? 1 : 0);
    for (const t of al) if (t !== e && rnd(4) === 0) A.Miota.add(4 * (e * 19 + t)).writeS32(rnd(2) ? -rnd(200) : rnd(200));
    for (let k = 0; k < 3; k++) {
      const p = 1 + rnd(np);
      if (owner(p) === 0) planet(p).add(0xd8 + 2 * e).writeS16(rnd(4) ? 1 : rnd(2) - 1);
    }
    const mine = planetsOf(e);
    if (!mine.length || e > 8) continue;
    if (rnd(4) === 0) {
      const col = designsWhere(e, (p) => p.add(0x34).readU8() !== 0);
      if (col.length) {
        const q = planet(mine[rnd(mine.length)]);
        const f = flottaNew(Memory.allocAnsiString('Probe'), e, 0, q.add(0x18).readFloat() + rnd(5) - 2, q.add(0x1c).readFloat() + rnd(5) - 2, 0);
        if (f > 0) for (let j = 0, n = 1 + rnd(3); j < n; j++) hajoNew(f, col[rnd(col.length)]);
      }
    }
    // No wars now and then (a war declaration needs an empire at peace), and transports in the main fleet.
    if (rnd(4) === 0) for (const t of al) {
      if (t === e || A.Viszony.add(e * 0x2d2 + t * 0x26 + 0x25).readU8() !== 1) continue;
      A.Viszony.add(e * 0x2d2 + t * 0x26 + 0x25).writeU8(3); A.Viszony.add(t * 0x2d2 + e * 0x26 + 0x25).writeU8(3);
    }
    const mainF = A.FoFlotta.add(4 * e).readS32();
    if (mainF > 0 && rnd(4) === 0 && fp().add(0x80 * mainF + 0x15).readU8() === e && fp().add(0x80 * mainF + 0x78).readS32() < 1) {
      const carriers = designsWhere(e, (p) => p.add(0x16).readS16() === 1 && p.add(0x28).readU8() > 0);
      if (carriers.length) hajoNew(mainF, carriers[rnd(carriers.length)]);
    }
    const ground = designsWhere(e, (p) => p.add(0x16).readS16() === 2);
    if (ground.length && rnd(3) === 0) tankNew(-mine[rnd(mine.length)], ground[rnd(ground.length)], 1 + rnd(8));
    const main = A.FoFlotta.add(4 * e).readS32();
    if (ground.length && main > 0 && rnd(4) === 0 && fp().add(0x80 * main + 0x15).readU8() === e) tankNew(main, ground[rnd(ground.length)], 1 + rnd(4));
  }
  // Foes next to the AI's fleets (visible to all), and assorted speeds.
  const nfl = nFleets();
  for (let k = 0; k < 4 && ais.length; k++) {
    const e = ais[rnd(ais.length)], mineF = fleetsOf(e);
    const g = 1 + rnd(nfl), gr = fp().add(0x80 * g);
    if (!mineF.length || gr.add(0x15).readU8() === 0xff || gr.add(0x15).readU8() === e || gr.add(0x16).readS16() !== 0) continue;
    const a = fp().add(0x80 * mineF[rnd(mineF.length)]);
    gr.add(0x30).writeFloat(a.add(0x30).readFloat() + rnd(31) - 15);
    gr.add(0x34).writeFloat(a.add(0x34).readFloat() + rnd(31) - 15);
    for (let x = 1; x <= 18; x++) gr.add(0x19 + x).writeU8(1);
  }
  for (let k = 0; k < 3; k++) { const g = 1 + rnd(nfl), gr = fp().add(0x80 * g); if (gr.add(0x15).readU8() !== 0xff) gr.add(0x38).writeFloat(0.02 * (1 + rnd(6))); }
  // Foes next to free planets in an AI's sight (a dangerous colony route merges the colony fleet back).
  for (let k = 0; k < 6 && ais.length; k++) {
    const e = ais[rnd(ais.length)], p = 1 + rnd(np), q = planet(p);
    if (owner(p) !== 0 || q.add(0xd8 + 2 * e).readS16() <= 0) continue;
    const g = 1 + rnd(nfl), gr = fp().add(0x80 * g);
    if (gr.add(0x15).readU8() === 0xff || gr.add(0x15).readU8() === e || gr.add(0x16).readS16() !== 0) continue;
    gr.add(0x30).writeFloat(q.add(0x18).readFloat() + rnd(9) - 4);
    gr.add(0x34).writeFloat(q.add(0x1c).readFloat() + rnd(9) - 4);
    for (let x = 1; x <= 18; x++) gr.add(0x19 + x).writeU8(1);
  }
  // One AI finds hostile warships beside every free planet in its sight (merges colony fleets back).
  if (ais.length && rnd(2) === 0) {
    const e = ais[rnd(ais.length)];
    const foes = [];
    for (let g = 1; g <= nfl; g++) {
      const gr = fp().add(0x80 * g), o = gr.add(0x15).readU8();
      if (o !== 0xff && o !== e && o <= 8 && gr.add(0x16).readS16() === 0 && gr.add(0x60).readS32() > 0) foes.push(g);
    }
    for (let p = 1, k = 0; p <= np && k < foes.length; p++) {
      const q = planet(p);
      if (owner(p) !== 0 || q.add(0xd8 + 2 * e).readS16() <= 0) continue;
      const gr = fp().add(0x80 * foes[k++]), o = gr.add(0x15).readU8();
      gr.add(0x30).writeFloat(q.add(0x18).readFloat() + rnd(5) - 2);
      gr.add(0x34).writeFloat(q.add(0x1c).readFloat() + rnd(5) - 2);
      for (let x = 1; x <= 18; x++) gr.add(0x19 + x).writeU8(1);
      A.Viszony.add(e * 0x2d2 + o * 0x26 + 0x25).writeU8(1); A.Viszony.add(o * 0x2d2 + e * 0x26 + 0x25).writeU8(1);
    }
  }
  // A planetless AI declares war on any target in sight (BirodalomHanyBolygo, diplomacy's daily count): for this
  // call only.
  for (const e of ais) if (rnd(6) === 0) tmp(A.HanyB.add(4 * e), 4, 0);
  // An AI with nothing left to colonise (>= 6 planets, no free planet in sight): its colony-ship designs go
  // (TervDelete) and are redone (FRISSIT_MI_DESIGN(e, false)). For this call only.
  for (const e of ais) {
    if (rnd(5) !== 0) continue;
    const v = (p) => planet(p).add(0xd8 + 2 * e);
    saved.push([A.HanyB.add(4 * e), A.HanyB.add(4 * e).readByteArray(4)]);
    A.HanyB.add(4 * e).writeS32(6 + rnd(4));
    for (let p = 1; p <= np; p++) if (owner(p) === 0 && v(p).readS16() > 0) { saved.push([v(p), v(p).readByteArray(2)]); v(p).writeS16(0); }
  }
}

// Set-up: revive the other empires of 1..8 as AIs that never die, with some built-up planets of AI 3
// (BolygoElfoglal on building-less planets crashes the exe), and (--ships) give empires with designs a few warships.
function setup() {
  const en = A.EN.readS32(), np = nPlanets();
  const built = []; for (let p = 1; p <= np; p++) if (A.HanyEpuletLefoglalva.add(4 * p).readS32() > 0 && owner(p) !== en && owner(p) !== 0) built.push(p);
  for (let e = 1; e <= 8; e++) {
    if (e === en || A.El.add(e).readU8()) continue;
    A.El.add(e).writeU8(1); A.MIzik.add(e).writeU8(1);
    if (A.PlayerFaj.add(e).readU8() === 0) A.PlayerFaj.add(e).writeU8(1 + rnd(14));
    for (let k = 0; k < 2 && built.length > 2; k++) { const i = rnd(built.length); A.Bolygok.add(0x188 * built[i] + 0x20).writeU8(e); built.splice(i, 1); }
  }
  if (cfg.aiHuman) A.PlayerKi.add(en).writeU8(0);
  for (let e = 1; e <= 8 && cfg.ships; e++) {
    const cnt = nDesigns(e);
    if (cnt < 1) continue;
    const ships = [];
    for (let d = 1; d <= cnt; d++) { const p = designs(e).add(0x84 * d); if (p.add(0x16).readS16() === 1 && p.add(0x36).readU8() === 0 && (p.add(0x2c).readS32() > 0 || p.add(0x30).readS32() > 0)) ships.push(d); }
    const mine = planetsOf(e);
    if (!ships.length || !mine.length) continue;
    for (let k = 0; k < 3; k++) {
      const q = planet(mine[rnd(mine.length)]);
      const f = flottaNew(Memory.allocAnsiString('Probe'), e, 0, q.add(0x18).readFloat() + rnd(9) - 4, q.add(0x1c).readFloat() + rnd(9) - 4, 0);
      if (f < 1) continue;
      for (let j = 0; j < 1 + rnd(5); j++) hajoNew(f, ships[rnd(ships.length)]);
    }
  }
  emit({ k: 'setup', ais: aiEmpires(), ships: nShips(), fleets: nFleets() });
}

// ---------------------------------------------------------------- hooks
let seq = 0, phase = 0, rec = null, calls = [];
function checkpoint(at, e) {
  if (!rec) return;
  const s = read();
  rec.cps.push({ at, e, d: diff(known, s), calls });
  calls = [];
  known = s;
}
function track(addr, name, n) {
  Interceptor.attach(addr, {
    onEnter(args) { if (rec) { const a = [name]; for (let i = 0; i < n; i++) a.push(args[i].toInt32()); calls.push(a); } },
  });
}
track(A.HajoDelete, 'hajodel', 1); track(A.MoneyLog, 'money', 2); track(A.Search2spy, 'search2spy', 4);
track(A.RndValasztas, 'valasztas', 2);
// Part-2 coverage: colony sends, merges, design deletes and creation, vehicle moves, fleet creation/deletion,
// and the result of every MenekulesHaKell.
for (const [va_, name, n] of [[0x53c2e0, 'delkolon', 2], [0x53d860, 'osszevon', 2], [0x541d50, 'tervdel', 4],
  [0x5422f0, 'tervnew', 2], [0x541920, 'autodesign', 3], [0x541410, 'tankatpakol', 4], [0x53d400, 'flottanew', 0],
  [0x53d040, 'flottadel', 1]]) track(va(va_), name, n);
Interceptor.attach(va(0x50f310), { onLeave(ret) { if (rec) calls.push(['menekul', ret.toInt32()]); } });
// --debug: report a crash (address, registers) before the process dies.
Process.setExceptionHandler((d) => {
  if (cfg.debug) {
    const rel = (p) => { try { return '0x' + (p.sub(mod.base).toInt32() + IMAGE_BASE).toString(16); } catch (e) { return String(p); } };
    send({ type: 'batch', events: [{ k: 'crash', type: d.type, address: rel(d.address), mem: d.memory ? [d.memory.operation, String(d.memory.address)] : null,
      ctx: { eax: String(d.context.eax), ecx: String(d.context.ecx), edx: String(d.context.edx), esi: String(d.context.esi), edi: String(d.context.edi) },
      stack: Thread.backtrace(d.context, Backtracer.FUZZY).slice(0, 12).map(rel) }] });
  }
  return false;
});
// --debug: when the frame hook stops running, sample every thread's stack (exe VAs).
let wdFrames = -1, wdSame = 0;
setInterval(() => {
  if (!cfg.debug || phase !== 1) return;
  wdSame = frames === wdFrames ? wdSame + 1 : 0;
  wdFrames = frames;
  if (wdSame === 3 || wdSame === 6) {
    const rel = (p) => { const o = p.sub(mod.base).toInt32(); return o >= 0 && o < mod.size ? '0x' + (o + IMAGE_BASE).toString(16) : String(p); };
    const th = Process.enumerateThreads().map((t) => ({ id: t.id, pc: rel(t.context.pc), bt: Thread.backtrace(t.context, Backtracer.FUZZY).slice(0, 16).map(rel) }));
    send({ type: 'batch', events: [{ k: 'hang', frames, seq, threads: th }] });
  }
}, 2000);
// --debug: which design callee never returns.
for (const [va_, name] of [[0x541920, 'autodesign'], [0x5422f0, 'tervnew'], [0x4b19d0, 'naming'], [0x5423b0, 'ujraszamol'],
  [0x53dd90, 'recount'], [0x541be0, 'tervclear']]) {
  Interceptor.attach(va(va_), {
    onEnter(args) { if (cfg.debug && rec) send({ type: 'batch', events: [{ k: 'dbg', f: name, at: 'in', a: [args[0].toInt32(), args[1].toInt32(), args[2].toInt32()] }] }); },
    onLeave() { if (cfg.debug && rec) send({ type: 'batch', events: [{ k: 'dbg', f: name, at: 'out' }] }); },
  });
}
function checkpoint2(at, e, extra) {
  if (!rec) return;
  if (cfg.debug) { send({ type: 'batch', events: [{ k: 'cpdbg', seq, at, e, extra }] }); }
  const s = read();
  const cp = { at, e, d: diff(known, s), calls };
  Object.assign(cp, extra || {});
  rec.cps.push(cp);
  calls = [];
  known = s;
}
for (const [addr, name, n] of [[A.Kolonizalas, 'kol', 1], [A.Flottak_, 'flo', 1], [A.Design, 'des', 2], [A.War, 'war', 2]]) {
  Interceptor.attach(addr, {
    onEnter(args) {
      this.e = args[0].toInt32(); this.b = n > 1 ? args[1].toInt32() & (name === 'des' ? 0xff : 0xffffffff) : 0;
      if (rec) calls.push([name, this.e, this.b]);
      // The story's guards (StoryAdat+0xb0/+0xb8/+0xbc) are read only by FRISSIT_MI_FLOTTAK: set them for its
      // duration only (left set past it, the story check resets the game) and record the values it saw.
      this.story = null;
      if (name === 'flo' && rec && cfg.fuzz && rnd(5) === 0) {
        this.story = [A.StoryB0.readS32(), A.StoryB8.readS32(), A.StoryBC.readS32()];
        A.StoryB0.writeS32(rnd(4) === 0 ? 1 : 0); A.StoryB8.writeS32(rnd(2)); A.StoryBC.writeS32(rnd(2));
      }
      const extra = { b: this.b };
      if (name === 'flo') extra.story = [A.StoryB0.readS32(), A.StoryB8.readS32(), A.StoryBC.readS32()];
      checkpoint2(name + '0', this.e, extra);
    },
    onLeave() {
      if (this.story) { A.StoryB0.writeS32(this.story[0]); A.StoryB8.writeS32(this.story[1]); A.StoryBC.writeS32(this.story[2]); }
      checkpoint2(name + '1', this.e, { b: this.b });
    },
  });
}
for (const [addr, name] of [[A.Szamolas, 'szam'], [A.Gyartas, 'gy'], [A.Kemkedes, 'kem'], [A.Fejlesztes, 'fej']]) {
  Interceptor.attach(addr, {
    onEnter(args) { this.e = name === 'szam' ? 0 : args[0].toInt32(); checkpoint(name + '0', this.e); },
    onLeave() { checkpoint(name + '1', this.e); },
  });
}

Interceptor.attach(A.MI, {
  onEnter() {
    this.on = phase === 1 && seq < cfg.count;
    if (!this.on) return;
    this.saved = [];
    if (cfg.fuzz) fuzz(this.saved);
    const s = read();
    if (!known) { const init = {}; for (const n in s) init[n] = hexOf(s[n], 0, s[n].length); emit({ k: 'init', regions: init }); known = s; }
    rec = { k: 'mi', seq, g: globals(), pre: diff(known, s), cps: [] };
    known = s;
    calls = [];
  },
  onLeave() {
    if (!this.on) return;
    checkpoint('end', 0);
    rec.seq = seq++;
    emit(rec);
    rec = null;
    // Undo the temporary writes; 'known' stays the state at the end, so the next 'pre' carries the undo.
    for (const [p, b] of this.saved.reverse()) p.writeByteArray(b); // newest first: overlapping saves unwind
    if (seq >= cfg.count) { emit({ k: 'done' }); phase = 2; }
  },
});

let frames = 0, started = false, loadedAt = 0, lastIdo = -1, sameFor = 0, homeKep = -1, wantSpeed = 0;
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
    if (started && phase === 0 && frames >= loadedAt + 300) {
      if (cfg.fuzz) setup();
      homeKep = A.AktKep.readS32(); wantSpeed = A.Speed.readS32(); phase = 1;
    }
    // Slow-downs (an enemy approaching the human's planets, SetIdoLelassit) leave the clock at ×1: restore it.
    if (phase === 1 && frames % 300 === 0 && A.Speed.readS32() !== wantSpeed) {
      emit({ k: 'slow', seq, speed: A.Speed.readS32(), kep: A.AktKep.readS32() });
      setSpeed(3, 1);
    }
    // Fuzzed state can stop the clock (a popup or story screen waiting for the human): log it, go back to the
    // starmap screen and restart the clock.
    if (phase === 1) {
      const ido = A.Ido.readS32();
      sameFor = ido === lastIdo ? sameFor + 1 : 0;
      lastIdo = ido;
      if (sameFor === 200 || (sameFor > 200 && sameFor % 600 === 0)) {
        emit({ k: 'stall', seq, ido, kep: A.AktKep.readS32(), speed: A.Speed.readS32() });
        A.AktKepKell.writeS32(homeKep); setPause(0, 0); setSpeed(3, 1);
      }
    }
    if (cfg.debug && frames % 300 === 0) emit({ k: 'tick', frames, phase, seq, ido: A.Ido.readS32(), nap: A.Nap.readS32(), speed: A.Speed.readS32(), kep: A.AktKep.readS32() });
  },
});

send({ type: 'ready', base: mod.base.toString() });
