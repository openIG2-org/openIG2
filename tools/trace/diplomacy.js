// Frida agent: daily diplomacy on the GOG ig2.exe (spec re/rules/diplomacy.md).
// Records before/after every FRISSIT_Viszony (0x50df60, 'visz') and MI_DIPLOMACY (0x4c7060, 'mi') call
// as byte-region diffs ('init' = full dump, then per record 'pre' = changes since the previous record's
// "after" and 'post' = changes made by the call), with the libc rand() draws made inside the call.
// --fuzz perturbs relations, levels, contacts, contact ages, message timers, attack orders and the human's
// contact list before the "before" snapshot, and turns DEBUGX on for some MI_DIPLOMACY calls (restored after).
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330),
  Viszony_fn: va(0x50df60), MI_DIPLOMACY: va(0x4c7060), RandIat: va(0x8353ac),
  Viszony: va(0xdabf38), Ero: va(0xbaea78), UrEro: va(0xbaeac8), HanyBolygo_e: va(0xbaeb18),
  Vissza: va(0xbaeb68), Veszely: va(0xdb27f8), Mikor: va(0xdaf520), Ismeri: va(0xbaebb8),
  Miota: va(0xbaed28), El: va(0xbae6ac), MIzik: va(0x915f58), Tamadhat: va(0x915f44),
  PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574), MiFo: va(0xdb0598), Erod: va(0xb99cb8),
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8), Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8),
  CPool: va(0xbb4108), TPool: va(0xbb4158), Techs: va(0xb9c0c8), AktPlayerek: va(0xa41e98),
  MaxPlayerek: va(0xab64dc), RandSeed: va(0xbaf2d0), Waypointek: va(0xbb41a4), WaypointCount: va(0xbb60f0),
  PP: va(0x912acc), PP2: va(0x912ad0), EN: va(0x912a3c), MP: va(0xbae2c0), DEBUGX: va(0xbae587),
  Enabled: va(0xbae3ad), AiOff: va(0xb77244), StoryQuiet: va(0xf274a8), StorySpare: va(0xf274c8),
  Nehezseg: va(0x92827c), Tut1: va(0xf6915d), Tut2: va(0xf66f2a), Forced: va(0xb7724c),
  Ido: va(0xbae04c), Nap: va(0xbae048), Honap: va(0xbae044),
  // Human side (mode 'human'): FeldolgozAjanlat, FeldolgozValasz, FRISSIT_Diplomacy and their state.
  FeldolgozAjanlat: va(0x4c1240), FeldolgozValasz: va(0x4c1480), FRISSIT_Diplomacy: va(0x4c0f90),
  Money: va(0xbad168), Pools: va(0xbad200), Hatar: va(0xbad590), Szorzo: va(0xbadcb0),
  Kidnap: va(0xbb405c), Kutat: va(0xb9a9b8), NeededMoral: va(0xb9a630), Lazad: va(0xb99668),
  Utolso: va(0xb99340), HanyEpulet: va(0xba9518), HanyEpuletLefoglalva: va(0xba9840), Epuletek: va(0xba9e90),
  Kor: va(0xa41e48), Udv: va(0xa41cc4), DipOldNap: va(0xab6ffc), DipStarted: va(0xab69b5), AllTechs: va(0xb77254),
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, count: 2000, fuzz: false, seed: 4242,
  mode: 'daily', edges: false, every: 3 };
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

const nFleets = () => A.FlottaCount.readS32();
const nPlanets = () => A.HanyBolygo.readS32();
const rel = (a, b) => A.Viszony.add(a * 0x2d2 + b * 0x26);

// ---------------------------------------------------------------- regions
function regions() {
  const np = nPlanets(), nfl = nFleets(), mp = A.MaxPlayerek.readS32(), wp = A.Waypointek.readPointer();
  return [
    ['visz', A.Viszony, 19 * 0x2d2], ['ero', A.Ero, 76], ['urero', A.UrEro, 76], ['hany', A.HanyBolygo_e, 76],
    ['vissza', A.Vissza, 76], ['veszely', A.Veszely, 76], ['mikor', A.Mikor, 181 * 4], ['ismeri', A.Ismeri, 361],
    ['miota', A.Miota, 361 * 4], ['el', A.El, 19], ['mizik', A.MIzik, 19], ['tamadhat', A.Tamadhat, 19],
    ['ki', A.PlayerKi, 19], ['faj', A.PlayerFaj, 19], ['mifo', A.MiFo, 76], ['erod', A.Erod, 4 * (np + 1)],
    ['planets', A.Bolygok, 0x188 * (np + 1)], ['fleets', A.Flottak.readPointer(), 0x80 * (nfl + 1)],
    ['cpool', A.CPool, 76], ['tpool', A.TPool, 76], ['techs', A.Techs, 193 * 0x114],
    // AktPlayerek[-1 .. MaxPlayerek): slot -1 is what the game reads for an empire without a slot.
    ['players', A.AktPlayerek.sub(0xc4), 0xc4 * (Math.max(mp, 0) + 1)], ['seeds', A.RandSeed, 21 * 4],
    ['spare', A.StorySpare, 76],
    wp.isNull() ? ['waypoints', A.Waypointek, 0] : ['waypoints', wp, 12 * (Math.max(A.WaypointCount.readS32(), 0) + 1)],
  ].concat(cfg.mode !== 'human' ? [] : [
    ['money', A.Money, 19 * 8], ['pools', A.Pools, 19 * 48], ['hatar', A.Hatar, 19 * 48], ['szorzo', A.Szorzo, 19 * 48],
    ['kidnap', A.Kidnap, 18 * 4], ['kutat', A.Kutat, 19 * 4], ['moral', A.NeededMoral, 4 * (np + 1)],
    ['lazad', A.Lazad, 4 * (np + 1)], ['utolso', A.Utolso, 4 * (np + 1)], ['hanyep', A.HanyEpulet, 4 * (np + 1)],
    ['nb', A.HanyEpuletLefoglalva, 4 * (np + 1)], ['kor', A.Kor, 19 * 4], ['udv', A.Udv, 19],
    ['dipnap', A.DipOldNap, 4], ['dipstart', A.DipStarted, 1],
  ]);
}
let known = null;
function read() {
  const out = {};
  for (const [n, p, sz] of regions()) out[n] = new Uint8Array(p.readByteArray(sz));
  if (cfg.mode === 'human') {
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
  }
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
  return { ido: A.Ido.readS32(), nap: A.Nap.readS32(), pp: A.PP.readS32(), pp2: A.PP2.readS32(), en: A.EN.readS32(),
    mp: A.MP.readU8(), debug: A.DEBUGX.readU8(), enabled: A.Enabled.readU8(), aioff: A.AiOff.readS32(),
    quiet: A.StoryQuiet.readS32(), diff: A.Nehezseg.readS32(), tut: A.Tut1.readU8() | A.Tut2.readU8(),
    forced: A.Forced.readS32(), nplanets: nPlanets(), nfleets: nFleets(), maxp: A.MaxPlayerek.readS32(),
    honap: A.Honap.readS32(), alltechs: A.AllTechs.readS32() };
}

// ---------------------------------------------------------------- fuzz
function alive() { const out = []; for (let e = 1; e <= 18; e++) if (A.El.add(e).readU8()) out.push(e); return out; }
function fuzzRelations(saved) {
  const al = alive();
  if (al.length < 2) return;
  const pick = () => al[rnd(al.length)];
  for (let k = 0; k < 6; k++) {
    const a = pick(), b = pick();
    if (a === b) continue;
    switch (rnd(9)) {
      case 0: { const l = 1 + rnd(5); rel(a, b).add(0x25).writeU8(l); rel(b, a).add(0x25).writeU8(l); break; }
      case 1: case 2: { const f = rnd(18), v = rnd(5) === 0 ? rnd(30000) - 10000 : rnd(10001); rel(a, b).add(2 * f).writeS16(v); break; }
      case 3: { const t = rnd(2); rel(a, b).add(0x24).writeU8(t); rel(b, a).add(0x24).writeU8(t); break; }
      case 4: A.Ismeri.add(a * 19 + b).writeU8(1); A.Ismeri.add(b * 19 + a).writeU8(1); break;
      case 5: A.Mikor.add(4 * (a * 9 + b)).writeS32(rnd(2) ? 280 + rnd(60) : rnd(1000)); break;
      case 6: A.Miota.add(4 * (a * 19 + b)).writeS32(rnd(3) ? rnd(700) - 100 : rnd(5000)); break;
      case 7: if (rnd(2)) { // a friendly pair: offers that pass the AI's rules
        const r = rel(a, b), w = (f, lo, hi) => r.add(2 * f).writeS16(lo + rnd(hi - lo + 1));
        w(6, 4900, 9500); w(14, 4000, 9500); w(0, 0, 900); w(5, 0, 900); w(17, 4500, 9500); w(1, 4500, 8500); w(15, 3000, 9000); w(8, 4000, 7000);
        break;
      } // fall through: an attack order
      default: { // a fleet of a attacks a planet or fleet of b
        const nfl = nFleets(), np = nPlanets();
        const own = [], tgtP = [], tgtF = [];
        for (let f = 1; f <= nfl; f++) {
          const o = A.Flottak.readPointer().add(0x80 * f).add(0x15).readU8();
          if (o === a) own.push(f); else if (o === b) tgtF.push(f);
        }
        for (let p = 1; p <= np; p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === b) tgtP.push(p);
        if (!own.length) break;
        const fl = A.Flottak.readPointer().add(0x80 * own[rnd(own.length)]);
        if (tgtP.length && (rnd(2) || !tgtF.length)) { fl.add(0x40).writeS16(2); fl.add(0x42).writeS16(1); fl.add(0x44).writeFloat(tgtP[rnd(tgtP.length)]); }
        else if (tgtF.length) { fl.add(0x40).writeS16(2); fl.add(0x42).writeS16(2); fl.add(0x44).writeFloat(tgtF[rnd(tgtF.length)]); }
        break;
      }
    }
  }
  // Main fleets with some strength, so MI_KitAkarTamadni finds targets.
  if (rnd(2) === 0) {
    const nfl = nFleets(), e = pick();
    if (nfl > 0) {
      const f = 1 + rnd(nfl); A.MiFo.add(4 * e).writeS32(f);
      const fl = A.Flottak.readPointer().add(0x80 * f); fl.add(0x60).writeS32(rnd(3000)); fl.add(0x68).writeS32(rnd(3000));
    }
  }
  const mp = A.MaxPlayerek.readS32();
  if (mp > 0 && rnd(4) === 0) { const s = A.AktPlayerek.add(0xc4 * rnd(mp)); s.add(0x17).writeU8(rnd(2)); s.add(0x14).writeU8(rnd(3) ? 1 : 0); }
  if (rnd(10) === 0) { saved.push([A.Nehezseg, A.Nehezseg.readByteArray(4)]); A.Nehezseg.writeS32(rnd(5)); }
  if (cfg.edges) fuzzEdges(al);
}

// Boundary values for the rules the plain fuzz never decided (report 2026-10-15): the "too scared" 7000/6000
// bounds, the `/4` main-fleet test of MI_KitAkarTamadni (with negative strengths: truncation toward zero),
// friendly war pairs for cease-fires, and the talk reaction roll.
function fuzzEdges(al) {
  const pick = () => al[rnd(al.length)];
  for (let k = 0; k < 3; k++) {
    const a = pick(), b = pick();
    if (a === b) continue;
    switch (rnd(4)) {
      case 0: rel(a, b).add(0x25).writeU8(1); rel(b, a).add(0x25).writeU8(1); rel(a, b).add(28).writeS16([5999, 6000, 6999, 7000, 7001][rnd(5)]); break;
      case 1: { // a friendly war: cease-fire and peace offers pass
        rel(a, b).add(0x25).writeU8(1); rel(b, a).add(0x25).writeU8(1);
        for (const [x, y] of [[a, b], [b, a]]) {
          const r = rel(x, y); r.add(12).writeS16(5500 + rnd(3000)); r.add(28).writeS16(6000 + rnd(3500));
          r.add(0).writeS16(rnd(300)); r.add(10).writeS16(rnd(300)); r.add(30).writeS16(4000 + rnd(3000));
        }
        A.Mikor.add(4 * (a * 9 + b)).writeS32(301 + rnd(50));
        break;
      }
      case 2: { // main fleets around attack(c)/4 == attack(e)
        const nfl = nFleets();
        if (nfl < 2) break;
        const fa = 1 + rnd(nfl), fb = 1 + rnd(nfl);
        if (fa === fb) break;
        A.MiFo.add(4 * a).writeS32(fa); A.MiFo.add(4 * b).writeS32(fb);
        const base = rnd(3) === 0 ? -(1 + rnd(5)) : rnd(400);
        const fl = (f) => A.Flottak.readPointer().add(0x80 * f);
        fl(fa).add(0x60).writeS32(base); fl(fb).add(0x60).writeS32(4 * base + rnd(7) - 3);
        fl(fa).add(0x68).writeS32(100000);
        break;
      }
      default: { // empire power for the 0.7 / 1.4 talk ratios
        const pa = 50 + rnd(5000);
        A.Ero.add(4 * a).writeFloat(pa);
        A.Ero.add(4 * b).writeFloat(pa * [0.69, 0.7, 0.71, 1.39, 1.4, 1.41][rnd(6)]);
      }
    }
  }
}

// Fuzz set-up: quick.scn has only two living empires (AI 3, the human 6). Revive the other empires of
// 1..ProgramPlayers (not 5, which diplomacy skips) as AIs that never die (BirodalomMIzik 0), hand each a few
// planets, and let everyone know everyone, so AI-to-AI diplomacy runs.
function setup() {
  const pp = A.PP.readS32(), en = A.EN.readS32(), np = nPlanets();
  for (let e = 1; e <= pp; e++) {
    if (e === 5 || e === en) continue;
    if (!A.El.add(e).readU8()) {
      A.El.add(e).writeU8(1); A.MIzik.add(e).writeU8(0);
      if (A.PlayerFaj.add(e).readU8() === 0 || A.PlayerFaj.add(e).readU8() === 5) A.PlayerFaj.add(e).writeU8(1 + rnd(4));
      for (let k = 0; k < 2 + rnd(3); k++) { const p = 1 + rnd(np); const o = A.Bolygok.add(0x188 * p + 0x20); if (o.readU8() !== en) o.writeU8(e); }
    }
    A.Tamadhat.add(e).writeU8(1);
  }
  // Every empire sees every planet (targets need a visible planet).
  for (let p = 1; p <= np; p++) for (let e = 1; e <= pp; e++) { const v = A.Bolygok.add(0x188 * p + 0xd8 + 2 * e); if (v.readS16() <= 0) v.writeS16(1); }
  // One planetless AI empire (if 8 is free) runs its death countdown.
  if (pp >= 8 && en !== 8) {
    A.MIzik.add(8).writeU8(1); A.Vissza.add(32).writeS32(20 + rnd(30));
    for (let p = 1; p <= np; p++) { const o = A.Bolygok.add(0x188 * p + 0x20); if (o.readU8() === 8) o.writeU8(0); }
  }
  // Edges: AI 7 loses its planets with countdown 97, so the `power == 1.0 && countdown < 96` rule decides
  // (96 survives a day, 95 dies).
  if (cfg.edges && pp >= 7 && en !== 7) {
    A.MIzik.add(7).writeU8(1); A.Vissza.add(28).writeS32(97);
    for (let p = 1; p <= np; p++) { const o = A.Bolygok.add(0x188 * p + 0x20); if (o.readU8() === 7) o.writeU8(0); }
  }
  for (let a = 1; a <= pp; a++) for (let b = 1; b <= pp; b++) {
    if (a === b) continue;
    A.Ismeri.add(a * 19 + b).writeU8(1);
    const l = 1 + rnd(5); rel(a, b).add(0x25).writeU8(rel(b, a).add(0x25).readU8() || l); if (!rel(b, a).add(0x25).readU8()) rel(b, a).add(0x25).writeU8(l);
  }
}

// ---------------------------------------------------------------- hooks
let seq = 0, phase = 0, inCall = null, draws = [];
const randFn = A.RandIat.readPointer();
Interceptor.attach(randFn, { onLeave(r) { if (inCall) draws.push(r.toInt32()); } });

function hook(addr, kind) {
  Interceptor.attach(addr, {
    onEnter() {
      this.on = phase === 1 && seq < cfg.count && cfg.mode !== 'human';
      if (!this.on) return;
      this.saved = [];
      if (cfg.fuzz) {
        fuzzRelations(this.saved);
        if (kind === 'mi') { const pp = A.PP.readS32(); for (let k = 0; k < 8; k++) { const a = 1 + rnd(pp), b = 1 + rnd(pp); A.Mikor.add(4 * (a * 9 + b)).writeS32(250 + rnd(200)); } }
        if (kind === 'mi' && rnd(3) !== 0) { this.saved.push([A.DEBUGX, A.DEBUGX.readByteArray(1)]); A.DEBUGX.writeU8(1); }
      }
      const s = read();
      if (!known) { const init = {}; for (const n in s) init[n] = hexOf(s[n], 0, s[n].length); emit({ k: 'init', regions: init }); known = s; }
      this.g = globals();
      this.pre = diff(known, s);
      known = s;
      draws = [];
      inCall = kind;
    },
    onLeave() {
      if (!this.on) return;
      inCall = null;
      const s = read();
      emit({ k: kind, seq: seq++, g: this.g, pre: this.pre, post: diff(known, s), rand: draws });
      known = s;
      for (const [p, b] of this.saved) p.writeByteArray(b);
      if (seq >= cfg.count) { emit({ k: 'done' }); phase = 2; }
    },
  });
}
hook(A.Viszony_fn, 'visz');
hook(A.MI_DIPLOMACY, 'mi');

// ---------------------------------------------------------------- mode 'human'
// From the frame hook, stage one call of the exe's own message functions on fuzzed state: the human's
// messages and deals to an AI, AI messages to the human and to other AIs, the human's answers (with
// counter-proposals), and FRISSIT_Diplomacy over a fuzzed contact list. Our Interceptor hooks don't fire
// inside our own NativeFunction calls, so libc rand() is pinned instead: srand(S) on this (the game's) thread
// before the call, two rand() after it, and the draws are the MSVC LCG outputs from S up to that pair.
const feldAjanlat = nf(A.FeldolgozAjanlat, 'void', Array(14).fill('int'));
const feldValasz = nf(A.FeldolgozValasz, 'int', Array(13).fill('int'));
const frissitDip = nf(A.FRISSIT_Diplomacy, 'void', []);
const crt = Process.getModuleByName('msvcr120.dll');
const srand = new NativeFunction(crt.getExportByName('srand'), 'void', ['uint'], { abi: 'mscdecl', traps: 'all' });
const crand = new NativeFunction(crt.getExportByName('rand'), 'int', [], { abi: 'mscdecl', traps: 'all' });

function lcgDraws(seed, r1, r2) {
  let h = seed >>> 0;
  const out = [];
  for (let i = 0; i < 400; i++) { h = (Math.imul(h, 214013) + 2531011) >>> 0; out.push((h >>> 16) & 0x7fff); }
  for (let k = 0; k + 1 < out.length; k++) if (out[k] === r1 && out[k + 1] === r2) return out.slice(0, k);
  return null;
}

function aliveAis() {
  const en = A.EN.readS32(), out = [];
  for (const e of alive()) if (e !== en && e !== 5 && A.PlayerFaj.add(e).readU8() !== 5) out.push(e);
  return out;
}
function ownedPlanets(e) {
  const out = [];
  for (let p = 1; p <= nPlanets(); p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() === e) out.push(p);
  return out;
}
const techStatus = (t, e) => A.Techs.add(t * 0x114 + 0x45 + e).readU8();
// A deal item that `giver` could hand to `taker`: [kind, value].
function dealItem(giver, taker) {
  const any = () => 1 + rnd(A.PP.readS32());
  switch (rnd(7)) {
    case 0: return [0, rnd(3) ? rnd(4000) : rnd(200000)];
    case 1: return [1, rnd(5) ? any() : taker];
    case 2: { // owned planets with buildings only: BolygoElfoglal on a free, building-less planet crashes the exe
      const built = (p) => A.HanyEpuletLefoglalva.add(4 * p).readS32() > 0;
      const own = ownedPlanets(giver).filter(built);
      if (own.length && rnd(5)) return [2, own[rnd(own.length)]];
      const any = []; for (let p = 1; p <= nPlanets(); p++) if (A.Bolygok.add(0x188 * p + 0x20).readU8() !== 0 && built(p)) any.push(p);
      return [2, any.length ? any[rnd(any.length)] : 1];
    }
    case 3: {
      const own = []; for (let t = 1; t <= 192; t++) if (techStatus(t, giver) === 5) own.push(t);
      return [3, own.length && rnd(4) ? own[rnd(own.length)] : 1 + rnd(192)];
    }
    case 4: return [4, rnd(7)];
    case 5: return [5, any()];
    default: return [6, any()];
  }
}
// b's view of a turns friendly enough for deals to pass (some of the time).
function befriend(b, a) {
  const r = rel(b, a), w = (f, lo, hi) => r.add(2 * f).writeS16(lo + rnd(hi - lo + 1));
  w(6, 4500, 9000); w(8, 4400, 9500); w(5, 0, 1500); w(7, 3000, 8000); w(14, 3000, 9900); w(0, 0, 1200); w(17, 3000, 9000);
  if (rnd(2)) { const l = 1 + rnd(5); r.add(0x25).writeU8(l); rel(a, b).add(0x25).writeU8(l); }
}

function stageHuman() {
  if (seq >= cfg.count) return;
  const saved = [];
  fuzzRelations(saved);
  const en = A.EN.readS32(), ais = aliveAis();
  if (!ais.length) return;
  const pickAi = () => ais[rnd(ais.length)];
  // Kidnapped family members (spy state) that hostage deals release.
  if (rnd(3) === 0) { const e = 1 + rnd(8); A.Kidnap.add(4 * e).writeS32(rnd(4)); A.Kidnap.add(36 + 4 * e).writeS32(rnd(4) ? 1 + rnd(8) : 0); }
  // A Viszony v17 of exactly 1 lets "a declares war on value" deals through.
  if (rnd(6) === 0) rel(pickAi(), 1 + rnd(A.PP.readS32())).add(34).writeS16(1);
  if (rnd(4) === 0) { saved.push([A.Forced, A.Forced.readByteArray(4)]); A.Forced.writeS32(rnd(3)); }
  const ido = A.Ido.readS32();
  let kind, args, call;
  const msg = (from, to, decl, c, sub, value, sub2, value2) => [from, decl, ido, from, to, c, sub, value, sub2, value2, -1, 0, rnd(20), 0];
  const r = rnd(100);
  if (r < 50) { // a message to an AI: from the human (deals, negotiate, talk) or from another AI
    const b = pickAi();
    const a = r < 38 ? en : pickAi();
    if (a === b) return;
    let c, sub, value, sub2 = -1, value2 = -1;
    if (a === en && r < 30) {
      c = [1, 1, 2, 2, 3, 3, 4][rnd(7)];
      [sub, value] = dealItem(c === 2 ? b : a, c === 2 ? a : b);
      if (c === 1) [sub2, value2] = dealItem(b, a);
      if (c === 4) { sub = rnd(6); value = 0; }
      if (rnd(2)) befriend(b, a);
    } else if (rnd(2)) {
      c = 0; sub = [0, 0, 1, 1, 3, 5, 5, 8, 2, 4, 6, 7][rnd(12)]; value = 1 + rnd(A.PP.readS32());
      if (rnd(2)) befriend(b, a);
      if (sub <= 1 && rnd(2)) { // a friendly war: cease-fire or peace can be accepted
        for (const [x, y] of [[a, b], [b, a]]) rel(x, y).add(0x25).writeU8(1 + rnd(2));
        const q = rel(b, a); q.add(12).writeS16(5600 + rnd(3000)); q.add(28).writeS16(6200 + rnd(3500)); q.add(0).writeS16(0); q.add(10).writeS16(rnd(200));
      }
    } else {
      c = 6; sub = [0, 1, 2, 3, 4, 6, 8, 9, 5][rnd(9)]; value = 1 + rnd(A.PP.readS32());
      const pa = 50 + rnd(5000); A.Ero.add(4 * a).writeFloat(pa); A.Ero.add(4 * b).writeFloat(pa * [0.5, 0.69, 0.7, 0.71, 1, 1.39, 1.4, 1.41, 2][rnd(9)]);
    }
    kind = 'ajanlat'; args = msg(a, b, rnd(8) === 0 ? 1 : 0, c, sub, value, sub2, value2); call = () => feldAjanlat(...args);
  } else if (r < 62) { // an AI message to the human: it waits in the contact list
    const a = pickAi(), c = rnd(4);
    let sub, value, sub2 = -1, value2 = -1;
    if (c === 0) { sub = rnd(9); value = 1 + rnd(A.PP.readS32()); }
    else { [sub, value] = dealItem(c === 2 ? en : a, c === 2 ? a : en); if (c === 1) [sub2, value2] = dealItem(en, a); }
    kind = 'ajanlat'; args = msg(a, en, rnd(6) === 0 ? 1 : 0, c, sub, value, sub2, value2); call = () => feldAjanlat(...args);
  } else if (r < 85) { // the human answers an AI message (sometimes with a counter-proposal)
    const a = pickAi(), c = [0, 1, 1, 1, 2, 3][rnd(6)];
    let sub, value, sub2 = -1, value2 = -1;
    if (c === 0) { sub = [0, 1, 3, 5, 8, 7][rnd(6)]; value = 1 + rnd(A.PP.readS32()); }
    else { [sub, value] = dealItem(c === 2 ? en : a, c === 2 ? a : en); if (c === 1) [sub2, value2] = dealItem(en, a); }
    const counter = rnd(3) === 0 ? 1 : 0, yes = rnd(3) ? 1 : 0;
    // The counter-proposal's own items: the human gives sub/value, the AI sub2/value2.
    if (counter && c === 1) { [sub, value] = dealItem(en, a); [sub2, value2] = dealItem(a, en); }
    kind = 'valasz'; args = [en, en, a, c, ido, counter | (yes << 16), sub, value, sub2, value2, -1, 0, rnd(20)];
    call = () => feldValasz(...args);
  } else { // FRISSIT_Diplomacy over pending messages, some stale
    const mp = A.MaxPlayerek.readS32();
    for (let i = 0; i < mp; i++) {
      if (rnd(2)) continue;
      const s = A.AktPlayerek.add(0xc4 * i), e = s.readS32();
      if (e <= 0 || e === en) continue;
      const c = rnd(4);
      let sub, value, sub2 = -1, value2 = -1;
      if (c === 0) { sub = rnd(2) ? 5 : rnd(9); value = 1 + rnd(A.PP.readS32()); }
      else { sub = [2, 4, 5, 2, 0][rnd(5)]; value = sub === 2 ? 1 + rnd(nPlanets()) : sub === 5 ? 1 + rnd(A.PP.readS32()) : rnd(6); if (c === 1) { sub2 = [2, 4, 5, 3][rnd(4)]; value2 = sub2 === 2 ? 1 + rnd(nPlanets()) : 1 + rnd(8); } }
      const m = [rnd(2), ido, e, en, c, sub, value, sub2, value2];
      for (let k = 0; k < m.length; k++) s.add(0x30 + 4 * k).writeS32(m[k]);
      s.add(0x58).writeU8(rnd(5) === 0 ? 1 : 0); s.add(0x17).writeU8(rnd(5) ? 1 : 0);
    }
    if (rnd(3) === 0) A.DipOldNap.writeS32(A.Nap.readS32() + 1);
    if (rnd(4) === 0) { // 1 January
      saved.push([A.Honap, A.Honap.readByteArray(4)], [A.Nap, A.Nap.readByteArray(4)]);
      A.Honap.writeS32(1); A.Nap.writeS32(1); A.DipOldNap.writeS32(2);
    }
    kind = 'frissit'; args = []; call = () => frissitDip();
  }
  const s = read();
  if (!known) { const init = {}; for (const n in s) init[n] = hexOf(s[n], 0, s[n].length); emit({ k: 'init', regions: init }); known = s; }
  const g = globals(), pre = diff(known, s);
  known = s;
  const S = (rnd(0x7fffffff) + 1) >>> 0;
  srand(S);
  call();
  const r1 = crand(), r2 = crand();
  const rand = lcgDraws(S, r1, r2);
  const s2 = read();
  emit({ k: kind, seq: seq++, g, args, pre, post: diff(known, s2), rand: rand === null ? 'lost' : rand });
  known = s2;
  for (const [p, b] of saved) p.writeByteArray(b);
  if (seq >= cfg.count) { emit({ k: 'done' }); phase = 2; }
}

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
    if (started && phase === 0 && frames >= loadedAt + 300) { if (cfg.fuzz) setup(); phase = 1; }
    if (cfg.mode === 'human' && phase === 1 && frames % cfg.every === 0) stageHuman();
    if (cfg.debug && frames % 300 === 0) emit({ k: 'tick', frames, phase, ido: A.Ido.readS32(), nap: A.Nap.readS32() });
  },
});

send({ type: 'ready', base: mod.base.toString() });
