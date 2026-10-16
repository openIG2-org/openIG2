// Frida agent: the daily spy pass on the GOG ig2.exe (spec re/rules/espionage.md).
// Records before/after every FRISSIT_Spy (exe 0x584b30) call the game makes, as byte-region diffs ('init' =
// full dump, then per record 'pre' = changes since the previous record's "after", including the fuzzer's,
// and 'post' = changes made by the call), with the callees it reached (rebellions, building and ship deletions,
// techs, recruits, captures, money) for coverage.
// --fuzz: set-up revives empires 1..8 (quick.scn has two living ones) with built-up planets and gives every
// empire with designs a few warships; before each call it rewrites some spy records (missions 1..13 due today,
// catch windows, double agents, applicants, captured spies, counter-intelligence), and now and then the
// kidnap table, the SP debug override (0xB77250), DEBUGX, the day (Nap 1: monthly experience) and the
// difficulty (all restored after the call).
// --orders N: after each traced call, N staged spy-screen orders through the exe's own MULTIGET_Spy (0x5883c0, spec
// §8): its Multi_GetNextTipus/Multi_Get calls are pointed at stubs handing it one staged message, the target spy and
// money are prepared first (part of the record's 'pre'); records {k: 'order', order: [type, sender, ...]}.
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330),
  FRISSIT_Spy: va(0x584b30), SpyNewCode: va(0x58f830),
  Kemek: va(0xbafcc8), Ossz: va(0xbb1f84), Van: va(0xbb23d0), Kuld: va(0x912f48), Forced: va(0xb77250),
  NoRecruit: va(0xf66f2b), Tut1: va(0xf6915d), Tut2: va(0xf66f2a), DEBUGX: va(0xbae587), Nehezseg: va(0x92827c),
  MP: va(0xbae2c0), PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574), El: va(0xbae6ac), MIzik: va(0x915f58),
  PP: va(0x912acc), PP2: va(0x912ad0), EN: va(0x912a3c), Nap: va(0xbae048), Honap: va(0xbae044), Ido: va(0xbae04c),
  Kidnap: va(0xbb405c), Viszony: va(0xdabf38), Ismeri: va(0xbaebb8), Techs: va(0xb9c0c8), Bolygok: va(0xb85ea8),
  HanyBolygo: va(0x912ac8), RandSeed: va(0xbaf2d0), AllTechs: va(0xb77254),
  Money: va(0xbad168), Pools: va(0xbad200), Hatar: va(0xbad590), Szorzo: va(0xbadcb0), Kutat: va(0xb9a9b8),
  NeededMoral: va(0xb9a630), Lazad: va(0xb99668), Utolso: va(0xb99340), HanyEpulet: va(0xba9518),
  HanyEpuletLefoglalva: va(0xba9840), Epuletek: va(0xba9e90), EpuletekLeiro: va(0x8eba38),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8), Hajok: va(0xbb4100), HajoCount: va(0xbb4104),
  Tankok: va(0xbb41a8), TankCount: va(0xbb4154), Tervek: va(0xb9aa18), TervCount: va(0xb9c028),
  AktKep: va(0xbae0fc), AktKepKell: va(0xbae250), SetIdoPause: va(0x541300), Speed: va(0xbae25c), CPool: va(0xbb4108), TPool: va(0xbb4158),
  FlottaNew: va(0x53d400), HajoNew: va(0x53e800),
  // Callees (coverage).
  BolygoFellazad: va(0x53bc40), EpuletDelete: va(0x53c730), HajoDelete: va(0x53e5d0), Talalmany: va(0x524e50),
  SpyNew: va(0x58f830), SpyCopy: va(0x58d170), Sorsarol: va(0x584350), Felfedez: va(0x4c1d60), MoneyLog: va(0x53fb10),
  MultigetSpy: va(0x5883c0), NextCalls: [va(0x5883cd), va(0x5888f2)],
  GetCalls: [va(0x588400), va(0x5884c3), va(0x588546), va(0x5886b5), va(0x588701), va(0x5888b5)],
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, count: 600, fuzz: false, seed: 4242, debug: false,
  ships: false, aiHuman: false, orders: 0 };
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
const setPause = nf(A.SetIdoPause, 'void', ['int', 'int']);
const flottaNew = nf(A.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float']);
const hajoNew = nf(A.HajoNew, 'void', ['int', 'int']);

const nPlanets = () => A.HanyBolygo.readS32();
const nFleets = () => A.FlottaCount.readS32();
const nShips = () => A.HajoCount.readS32();
const spyAt = (e, k) => A.Kemek.add(e * 0x3dc + k * 0x4c);
const designs = (e) => A.Tervek.add(4 * e).readPointer();
const owner = (p) => A.Bolygok.add(0x188 * p + 0x20).readU8();
const techStatus = (t, e) => A.Techs.add(t * 0x114 + 0x45 + e).readU8();

// Spy_New's names: `mov dword [ebp + d], imm32` with imm32 a string (relocated in memory).
const names = (() => {
  const code = new Uint8Array(A.SpyNewCode.readByteArray(0x4c0)), found = [];
  for (let i = 0; i + 7 <= code.length; i++) {
    if (code[i] !== 0xc7 || (code[i + 1] !== 0x45 && code[i + 1] !== 0x85)) continue;
    const wide = code[i + 1] === 0x85, o = i + (wide ? 6 : 3);
    const imm = (code[o] | code[o + 1] << 8 | code[o + 2] << 16 | code[o + 3] << 24) >>> 0;
    const q = ptr(imm);
    if (q.compare(mod.base) < 0 || q.compare(mod.base.add(mod.size)) >= 0) continue;
    try { const s = q.readCString(); if (s && s.length < 16 && /^[ -~]+$/.test(s)) found.push(s); } catch (e) { /* not a string */ }
  }
  return found;
})();

// ---------------------------------------------------------------- regions
function regions() {
  const np = nPlanets(), nfl = nFleets(), ns = nShips(), nt = A.TankCount.readS32();
  const out = [
    ['kemek', A.Kemek, 9 * 0x3dc], ['ossz', A.Ossz, 36], ['van', A.Van, 36], ['kuld', A.Kuld, 27 * 0x20],
    ['visz', A.Viszony, 19 * 0x2d2], ['ismeri', A.Ismeri, 361], ['techs', A.Techs, 193 * 0x114],
    ['planets', A.Bolygok, 0x188 * (np + 1)], ['money', A.Money, 19 * 8], ['pools', A.Pools, 19 * 48],
    ['hatar', A.Hatar, 19 * 48], ['szorzo', A.Szorzo, 19 * 48], ['kidnap', A.Kidnap, 18 * 4], ['seeds', A.RandSeed, 21 * 4],
    ['kutat', A.Kutat, 19 * 4], ['moral', A.NeededMoral, 4 * (np + 1)], ['lazad', A.Lazad, 4 * (np + 1)],
    ['utolso', A.Utolso, 4 * (np + 1)], ['hanyep', A.HanyEpulet, 4 * (np + 1)], ['nb', A.HanyEpuletLefoglalva, 4 * (np + 1)],
    ['el', A.El, 19], ['fleets', A.Flottak.readPointer(), 0x80 * (nfl + 1)], ['ships', A.Hajok.readPointer(), 6 * (ns + 1)],
    ['tanks', A.Tankok, 8 * (Math.max(nt, 0) + 1)], ['cpool', A.CPool, 76], ['tpool', A.TPool, 76],
  ];
  for (let e = 1; e <= 8; e++) {
    const n = A.TervCount.add(4 * e).readS32(), p = designs(e);
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
  return { ido: A.Ido.readS32(), nap: A.Nap.readS32(), honap: A.Honap.readS32(), pp: A.PP.readS32(), pp2: A.PP2.readS32(),
    en: A.EN.readS32(), mp: A.MP.readU8(), debug: A.DEBUGX.readU8(), diff: A.Nehezseg.readS32(),
    tut: A.Tut1.readU8() | A.Tut2.readU8(), norecruit: A.NoRecruit.readU8(), forced: A.Forced.readS32(),
    nplanets: nPlanets(), alltechs: A.AllTechs.readS32(), ki: hexOf(new Uint8Array(A.PlayerKi.readByteArray(19)), 0, 19),
    faj: hexOf(new Uint8Array(A.PlayerFaj.readByteArray(19)), 0, 19) };
}

// ---------------------------------------------------------------- fuzz
const empires = () => { const out = []; for (let e = 1; e <= 8; e++) if (A.El.add(e).readU8()) out.push(e); return out; };
function planetsOf(t) { const out = []; for (let p = 1; p <= nPlanets(); p++) if (owner(p) === t) out.push(p); return out; }
function param(m, t) {
  const mine = planetsOf(t), pick = (xs) => xs.length ? xs[rnd(xs.length)] : 0;
  switch (m) {
    case 1: return [rnd(6), 0];
    case 3: case 9: return [rnd(3) ? 0 : (rnd(4) ? pick(mine) : 1 + rnd(nPlanets())), 0];
    case 6: {
      const p = rnd(3) ? 0 : pick(mine);
      let tech = 0;
      if (rnd(2) && mine.length) { // the tech of one of t's buildings
        const q = pick(mine), nb = A.HanyEpuletLefoglalva.add(4 * q).readS32(), arr = A.Epuletek.add(4 * q).readPointer();
        if (nb > 0 && !arr.isNull()) { const kind = arr.add(0x20 * (1 + rnd(nb))).readU8(); tech = A.EpuletekLeiro.add(0x24 * kind).readU8(); }
      }
      return [p, tech];
    }
    case 7: {
      if (rnd(2)) return [0, 0];
      const ns = nShips(), sh = A.Hajok.readPointer(), n = A.TervCount.add(4 * t).readS32();
      for (let k = 0; k < 20; k++) {
        const i = 1 + rnd(Math.max(ns, 1)), d = sh.add(6 * i + 2).readS16();
        if (d > 0 && d <= n && d !== 0x7fff) return [designs(t).add(0x84 * d + 0x48).readS32(), 0];
      }
      return [1 + rnd(150), 0];
    }
    case 8: {
      if (rnd(3)) return [0, 0];
      const own = []; for (let x = 1; x <= 192; x++) if (techStatus(x, t) === 5) own.push(x);
      return [own.length && rnd(4) ? pick(own) : 1 + rnd(192), 0];
    }
    default: return [rnd(4), 0];
  }
}
function writeSpy(e, k, human) {
  const s = spyAt(e, k), al = empires();
  const st = [0, 1, 1, 2, 3, 3, 3, 3, 3, 4][rnd(10)];
  const name = rnd(3) ? names[rnd(names.length)] : 'Fz' + rnd(1000);
  for (let i = 0; i < 16; i++) s.add(i).writeU8(i < name.length ? name.charCodeAt(i) : 0);
  s.add(0x10).writeS16(st); s.add(0x12).writeU8(1 + rnd(14));
  s.add(0x16).writeS16(human && rnd(4) === 0 ? 1 : 0);
  s.add(0x18).writeU8(rnd(6) === 0 ? 1 + rnd(8) : 0);
  const m = rnd(5) === 0 ? 13 : 1 + rnd(13);
  const t = rnd(8) === 0 ? e : (al.length ? al[rnd(al.length)] : 1 + rnd(8));
  const [p1, p2] = param(m, t);
  s.add(0x1a).writeU16(st === 3 || st === 2 ? m : rnd(3) ? 0 : m); s.add(0x1c).writeU8(t);
  s.add(0x1e).writeS16(p1); s.add(0x20).writeS16(p2);
  const left = rnd(3) === 0 ? 1 + rnd(2000) : 1 + rnd(100);
  s.add(0x24).writeS32(left + rnd(500)); s.add(0x28).writeS32(left);
  s.add(0x2c).writeS32(rnd(2) ? left - 1 - rnd(48) : (rnd(2) ? -1000 : rnd(3000)));
  for (let i = 0x31; i <= 0x34; i++) s.add(i).writeU8(rnd(4) === 0 ? rnd(60) : i === 0x31 && rnd(2) ? 4 + rnd(12) : rnd(16));
  s.add(0x38).writeS32(rnd(10));
  const rank = 1 + rnd(20);
  s.add(0x40).writeU8(rank);
  const xpEdge = [0, 500, 1000, 2000, 3500, 5000, 7000, 10000, 13500, 17000, 21000, 25500, 30500, 36000, 42000, 48500, 55500, 63000, 71000, 80000, 90000];
  s.add(0x3c).writeS32(rnd(3) ? rnd(100000) : xpEdge[Math.min(rank + rnd(3), 20)] + rnd(3) - 1);
  for (let x = 1; x <= 8; x++) s.add(0x41 + x).writeU8(rnd(5) === 0 ? 1 : 0);
}
function fuzz(saved) {
  const en = A.EN.readS32();
  const n = 3 + rnd(8);
  for (let i = 0; i < n; i++) {
    const e = 1 + rnd(8), k = 1 + rnd(12), cnt = A.Ossz.add(4 * e);
    if (cnt.readS32() < k) {
      for (let j = cnt.readS32() + 1; j < k; j++) spyAt(e, j).add(0x10).writeS16(0);
      cnt.writeS32(k);
    }
    writeSpy(e, k, A.PlayerKi.add(e).readU8() === 1);
  }
  if (rnd(6) === 0) { const e = 1 + rnd(8); writeSpy(e, 0, false); spyAt(e, 0).add(0x10).writeS16(rnd(2)); }
  if (rnd(10) === 0) { const e = 1 + rnd(8); A.Ossz.add(4 * e).writeS32(12); for (let k = 1; k <= 12; k++) if (spyAt(e, k).add(0x10).readS16() <= 0) writeSpy(e, k, false); }
  if (rnd(4) === 0) { const t = 1 + rnd(8); A.Kidnap.add(4 * t).writeS32(rnd(3) ? 0 : 1 + rnd(5)); }
  if (rnd(5) === 0) { const a = 1 + rnd(8), b = 1 + rnd(8); A.Ismeri.add(a * 19 + b).writeU8(0); }
  if (rnd(5) === 0) { // unseen planets for planet reports
    for (let k = 0; k < 6; k++) { const p = 1 + rnd(nPlanets()), e = 1 + rnd(8); A.Bolygok.add(0x188 * p + 0xd8 + 2 * e).writeS16(rnd(3) ? 0 : 1); }
  }
  if (rnd(5) === 0) { for (let k = 0; k < 20; k++) A.Techs.add((1 + rnd(192)) * 0x114 + 0xc0 + 9 * (1 + rnd(8)) + 1 + rnd(8)).writeU8(rnd(2)); }
  const tmp = (p, size, v) => { saved.push([p, p.readByteArray(size)]); if (size === 4) p.writeS32(v); else p.writeU8(v); };
  if (rnd(5) === 0) tmp(A.Forced, 4, [1, 1, 2, 3][rnd(4)]);
  if (rnd(10) === 0) tmp(A.DEBUGX, 1, 1);
  if (rnd(6) === 0) tmp(A.Nap, 4, 1);
  if (rnd(8) === 0) tmp(A.Nehezseg, 4, rnd(5));
  if (rnd(20) === 0) tmp(A.NoRecruit, 1, 1);
  void en;
}

// Set-up: revive the other empires of 1..8 as AIs that never die, with some built-up planets of AI 3
// (BolygoElfoglal on building-less planets crashes the exe), and (--ships) give empires with designs a few warships.
function setup() {
  const en = A.EN.readS32(), np = nPlanets();
  const built = []; for (let p = 1; p <= np; p++) if (A.HanyEpuletLefoglalva.add(4 * p).readS32() > 0 && owner(p) !== en && owner(p) !== 0) built.push(p);
  for (let e = 1; e <= 8; e++) {
    if (e === en || A.El.add(e).readU8()) continue;
    A.El.add(e).writeU8(1); A.MIzik.add(e).writeU8(0);
    if (A.PlayerFaj.add(e).readU8() === 0) A.PlayerFaj.add(e).writeU8(1 + rnd(14));
    for (let k = 0; k < 2 && built.length > 2; k++) { const i = rnd(built.length); A.Bolygok.add(0x188 * built[i] + 0x20).writeU8(e); built.splice(i, 1); }
  }
  // Warships make real-time battles with the human's side, which stop the clock on the battle screen: only
  // with --ships, which also hands the human's empire to the AI (battles then auto-resolve).
  if (cfg.aiHuman) A.PlayerKi.add(en).writeU8(0);
  for (let e = 1; e <= 8 && cfg.ships; e++) {
    const cnt = A.TervCount.add(4 * e).readS32();
    if (cnt < 1) continue;
    const ships = [];
    for (let d = 1; d <= cnt; d++) { const p = designs(e).add(0x84 * d); if (p.add(0x16).readS16() === 1 && p.add(0x36).readU8() === 0 && (p.add(0x2c).readS32() > 0 || p.add(0x30).readS32() > 0)) ships.push(d); }
    const mine = planetsOf(e);
    if (!ships.length || !mine.length) continue;
    for (let k = 0; k < 3; k++) {
      const q = A.Bolygok.add(0x188 * mine[rnd(mine.length)]);
      const f = flottaNew(Memory.allocAnsiString('Probe'), e, 0, q.add(0x18).readFloat() + rnd(9) - 4, q.add(0x1c).readFloat() + rnd(9) - 4, 0);
      if (f < 1) continue;
      for (let j = 0; j < 1 + rnd(4); j++) hajoNew(f, ships[rnd(ships.length)]);
    }
  }
  emit({ k: 'setup', names: names.length, ships: nShips(), fleets: nFleets() });
}

// ---------------------------------------------------------------- staged orders (MULTIGET_Spy)
const multigetSpy = nf(A.MultigetSpy, 'void', []);
let ostub = null;
// Stubs: +0 Multi_GetNextTipus (returns the staged type once, then 0), +0x40 Multi_Get (writes the staged ints to
// its out-parameters at [esp + 8 + 8i], returns 1); data at +0x200 (type) and +0x210 (values).
function stubs(n) {
  if (ostub === null) ostub = Memory.alloc(Process.pageSize);
  const flag = ostub.add(0x200), vals = ostub.add(0x210);
  Memory.patchCode(ostub, 0x100, (code) => {
    const w = new X86Writer(code, { pc: ostub });
    w.putMovRegAddress('ecx', flag); w.putMovRegRegPtr('eax', 'ecx'); w.putMovRegPtrU32('ecx', 0); w.putRet();
    w.flush();
    const g = new X86Writer(code.add(0x40), { pc: ostub.add(0x40) });
    for (let i = 0; i < n; i++) {
      g.putMovRegRegOffsetPtr('ecx', 'esp', 8 + 8 * i);
      g.putMovRegAddress('edx', vals.add(4 * i)); g.putMovRegRegPtr('eax', 'edx');
      g.putMovRegPtrReg('ecx', 'eax');
    }
    g.putMovRegU32('eax', 1); g.putRet();
    g.flush();
  });
}
function patchCalls(calls, target) {
  const saved = calls.map((c) => c.readByteArray(5));
  for (const c of calls) Memory.patchCode(c, 5, (code) => { const w = new X86Writer(code, { pc: c }); w.putCallAddress(target); w.flush(); });
  return () => calls.forEach((c, i) => Memory.patchCode(c, 5, (code) => code.writeByteArray(saved[i])));
}
function runOrder(type, vals) {
  stubs(vals.length);
  ostub.add(0x200).writeS32(type);
  vals.forEach((v, i) => ostub.add(0x210 + 4 * i).writeS32(v));
  const undoNext = patchCalls(A.NextCalls, ostub), undoGet = patchCalls(A.GetCalls, ostub.add(0x40));
  try { multigetSpy(); } finally { undoNext(); undoGet(); }
}
// A random order, its spy prepared so the interesting branches come up.
function stageOrder() {
  const en = A.EN.readS32(), e = rnd(3) ? en : 1 + rnd(8), k = rnd(10) ? 1 + rnd(12) : rnd(13);
  const cnt = A.Ossz.add(4 * e);
  if (cnt.readS32() < k) { for (let j = cnt.readS32() + 1; j < k; j++) spyAt(e, j).add(0x10).writeS16(0); cnt.writeS32(k); }
  const s = spyAt(e, k);
  if (rnd(4) === 0) writeSpy(e, k, true);
  if (rnd(5) === 0) A.Money.add(8 * e).writeDouble(rnd(3) ? rnd(30000) : 0);
  const type = 0x47 + rnd(6);
  switch (type) {
    case 0x47: return [type, [e, k]];
    case 0x48: s.add(0x10).writeS16([1, 3, 3, 2][rnd(4)]); return [type, [e, k]];
    case 0x49: {
      s.add(0x10).writeS16(rnd(5) ? 1 : [0, 2, 3][rnd(3)]);
      const m = rnd(4) === 0 ? 1 : rnd(14), t = 1 + rnd(8), [p1, p2] = m === 1 ? [rnd(6), 0] : param(m, t);
      return [type, [e, k, m, t, p1, p2, rnd(6) ? 1 + rnd(40) : rnd(3)]];
    }
    case 0x4a: {
      const o = 1 + rnd(8), j = 1 + rnd(12);
      if (A.Ossz.add(4 * o).readS32() < j) { for (let x = A.Ossz.add(4 * o).readS32() + 1; x < j; x++) spyAt(o, x).add(0x10).writeS16(0); A.Ossz.add(4 * o).writeS32(j); }
      spyAt(o, j).add(0x10).writeS16(4);
      return [type, [e, rnd(8), o, j]];
    }
    case 0x4b: s.add(0x10).writeS16(1); s.add(0x16).writeS16(1); if (rnd(4) === 0) s.add(0x12).writeU8(14); return [type, [e, rnd(5), k]];
    default: s.add(0x10).writeS16(rnd(4) ? 1 + rnd(3) : 0); s.add(0x38).writeS32(rnd(3)); return [type, [e, k, rnd(6)]];
  }
}
function orders() {
  for (let i = 0; i < cfg.orders; i++) {
    const [type, vals] = stageOrder();
    const s = read(), g = globals(), pre = diff(known, s);
    known = s;
    calls = []; inCall = true;
    runOrder(type, vals);
    inCall = false;
    const after = read();
    emit({ k: 'order', seq, g, order: [type].concat(vals), pre, post: diff(known, after), calls });
    known = after;
  }
}

// ---------------------------------------------------------------- hooks
let seq = 0, phase = 0, inCall = false, calls = [];
function track(addr, name, n) {
  Interceptor.attach(addr, {
    onEnter(args) { if (inCall) { const a = [name]; for (let i = 0; i < n; i++) a.push(args[i].toInt32()); calls.push(a); } },
  });
}
track(A.BolygoFellazad, 'fellazad', 3); track(A.EpuletDelete, 'epdel', 2); track(A.HajoDelete, 'hajodel', 1);
track(A.Talalmany, 'talalmany', 2); track(A.SpyNew, 'spynew', 1); track(A.SpyCopy, 'spycopy', 3);
track(A.Sorsarol, 'sors', 4); track(A.Felfedez, 'felfedez', 2); track(A.MoneyLog, 'money', 2);

Interceptor.attach(A.FRISSIT_Spy, {
  onEnter() {
    this.on = phase === 1 && seq < cfg.count;
    if (!this.on) return;
    this.saved = [];
    if (cfg.fuzz) fuzz(this.saved);
    const s = read();
    if (!known) { const init = {}; for (const n in s) init[n] = hexOf(s[n], 0, s[n].length); emit({ k: 'init', regions: init }); known = s; }
    this.g = globals();
    this.pre = diff(known, s);
    known = s;
    calls = [];
    inCall = true;
  },
  onLeave() {
    if (!this.on) return;
    inCall = false;
    const s = read();
    emit({ k: 'spy', seq: seq++, g: this.g, pre: this.pre, post: diff(known, s), calls });
    known = s;
    for (const [p, b] of this.saved) p.writeByteArray(b);
    if (cfg.orders) orders();
    if (seq >= cfg.count) { emit({ k: 'done' }); phase = 2; }
  },
});

let frames = 0, started = false, loadedAt = 0, lastIdo = -1, sameFor = 0, homeKep = -1;
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
    if (started && phase === 0 && frames >= loadedAt + 300) { if (cfg.fuzz) setup(); homeKep = A.AktKep.readS32(); phase = 1; }
    // Fuzzed state can stop the clock (a popup or story screen waiting for the human): log it, go back to the
    // starmap screen and restart the clock (as spacewar.js does after a tactical battle).
    if (phase === 1) {
      const ido = A.Ido.readS32();
      sameFor = ido === lastIdo ? sameFor + 1 : 0;
      lastIdo = ido;
      if (sameFor === 200 || (sameFor > 200 && sameFor % 600 === 0)) {
        emit({ k: 'stall', seq, ido, kep: A.AktKep.readS32(), speed: A.Speed.readS32() });
        A.AktKepKell.writeS32(homeKep); setPause(0, 0); setSpeed(3, 1);
      }
    }
    if (cfg.debug && frames % 300 === 0) emit({ k: 'tick', frames, phase, seq, ido: A.Ido.readS32(), nap: A.Nap.readS32() });
  },
});

send({ type: 'ready', base: mod.base.toString(), names: names.length });
