// Frida agent: colony side systems of the GOG ig2.exe (re/rules/colony-side.md).
// Modes (cfg.mode):
//   economy  - before/after snapshots of every FRISSIT_Planet(true, 0) with all planets and the side
//              globals (labs, spy capacity, strengths, yearly income, money, tech table, contacts);
//              cfg.fuzz pushes planets into rebellion/autobuild states first.
//   elfoglal - from the frame hook, call BolygoElfoglal(p, owner) on fuzzed state.
//   fellazad - call BolygoFellazad(p, to, force).
//   colonise - BolygoElfoglal + Generate_Planet0 + rndlog(1,0,0) + EpuletNewSearchLocation(p, 1, ..).
//   money    - MoneyMennyi(e, k) and MoneyHozzaadOld(e, k, x) on random pools.
//   arrive   - real fleet arrivals: on FRISSIT_Starmap ticks that aren't radar refreshes, 1-2 fleets get an
//              AI owner, the colonising (bit3) or terraforming (bit4) flag and a move to an unowned planet,
//              placed on its approach point (x, y + 2) so they arrive in this tick's movement loop
//              (fleet-movement.md 4.3); before/after snapshots of that call, with the rndlog calls.
// Frida 17 does not fire our own hooks inside our NativeFunction calls: direct-call modes compare state.
// Addresses are image VAs, rebased at runtime.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0), SetIdoSpeed: va(0x541330),
  FRISSIT_Planet: va(0x474930), rndlog: va(0x542da0),
  BolygoFellazad: va(0x53bc40), BolygoElfoglal: va(0x53b720), Generate_Planet0: va(0x59adf0),
  EpuletNewSearch: va(0x53cca0), MoneyMennyi: va(0x540170), MoneyHozzaad: va(0x53fbd0),
  RandSeed: va(0xbaf2d0),
  Ido: va(0xbae04c), Ev: va(0xbae040), Honap: va(0xbae044), Nap: va(0xbae048),
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8),
  HanyEpuletLefoglalva: va(0xba9840), HanyEpulet: va(0xba9518), Epuletek: va(0xba9e90),
  EN: va(0x912a3c), MultiPlayerMode: va(0xbae2c0), PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574),
  ProgramPlayers2: va(0x912ad0),
  GlobalisMoral: va(0xbaeac4), GlobalisMoralSebesseg: va(0xbaeb14),
  BolygoNeededMoral: va(0xb9a630), Lazad: va(0xb99668), UtolsoKozeli: va(0xb99340),
  FullGDP: va(0xbae060), FullTax: va(0xbae0b0),
  TradeLast: va(0xbae100), TradeThis: va(0xbae150), TaxLast: va(0xbae1a0), TaxThis: va(0xbae1f0),
  StoryAdat: va(0xf273f8), Kidnap: va(0xbb405c), ProblemakLeiro: va(0x912858), M: va(0xa3eb74),
  Labor: va(0x912728), LaborOssz: va(0xba90e0),
  KemLehet: va(0xbb2388), KemOssz: va(0xbb23ac), KemPer: va(0xbb2418), TradePorts: va(0xbb23f4),
  Foldi: va(0xb99990), Erod: va(0xb99cb8), IonGun: va(0xb99fe0), Tuzolto: va(0xb9a308),
  Money: va(0xbad168), Pools: va(0xbad200), Hatar: va(0xbad590), Szorzo: va(0xbadcb0),
  Nehezseg: va(0x92827c), PenzSzorzo: va(0x8f8848),
  El: va(0xbae6ac), Ismeri: va(0xbaebb8), RndN: va(0xbaeb64), RndTotal: va(0xbaebb4),
  Tech: va(0xb9c0c8), AllTechs: va(0xb77254),
  AbProblem: va(0x8f1d50), AbMin: va(0x8f1dd0), AbMax: va(0x8f1e50), AbMitEpit: va(0x996530),
  AbEgyeb: va(0x9965d0), AbAlso: va(0x996504), AbEgyebSz: va(0x9965a4),
  TalalmanyokrEro: va(0x8eb730), TalalmanyEroHP: va(0xba9210), pGyartas0: va(0x90b8a8),
  Flottak: va(0xbb40a4), FlottaN: va(0xbb40a8), FlottaPerOwner: va(0xbb40b0),
  Tankok: va(0xbb41a8), TankN: va(0xbb4154),
  FRISSIT_Starmap: va(0x594ae0), Ido2: va(0xbae050), Mult: va(0xbae25c),
  MapCursor: va(0x996420), Terepek: va(0x99825c), HanyTerep: va(0x9b2ec8), // NextColonyMap (0x486920)
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, speed: 3, mode: 'economy', fuzz: false,
  count: 100, every: 5, seed: 12345 };
recv('config', (m) => { Object.assign(cfg, m.payload); fz = cfg.seed >>> 0; });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 4) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 500);

const HEX = Array.from({ length: 256 }, (_, i) => (i < 16 ? '0' : '') + i.toString(16));
function hex(p, n) {
  const b = new Uint8Array(p.readByteArray(n));
  const out = new Array(b.length);
  for (let i = 0; i < b.length; i++) out[i] = HEX[b[i]];
  return out.join('');
}
const i32s = (p, n) => Array.from({ length: n }, (_, i) => p.add(4 * i).readS32());

function snapshot() {
  const n = A.HanyBolygo.readS32();
  const planets = [];
  for (let i = 1; i <= n; i++) {
    const p = A.Bolygok.add(i * 0x188);
    const nb = A.HanyEpuletLefoglalva.add(4 * i).readS32();
    const arr = A.Epuletek.add(4 * i).readPointer();
    planets.push({ i, raw: hex(p, 0x188), nb, bld: nb > 0 && !arr.isNull() ? hex(arr.add(0x20), nb * 0x20) : '' });
  }
  const nf = A.FlottaN.readS32();
  const fp = A.Flottak.readPointer();
  const nt = A.TankN.readS32();
  return {
    t: Date.now(), ido: A.Ido.readS32(), ev: A.Ev.readS32(), honap: A.Honap.readS32(), nap: A.Nap.readS32(),
    seeds: Array.from({ length: 21 }, (_, i) => A.RandSeed.add(4 * i).readU32()),
    en: A.EN.readS32(), mp: A.MultiPlayerMode.readU8(), players2: A.ProgramPlayers2.readS32(),
    playerKi: hex(A.PlayerKi, 19), playerFaj: hex(A.PlayerFaj, 19),
    gmoral: A.GlobalisMoral.readS32(), gmoralSpeed: A.GlobalisMoralSebesseg.readS32(),
    neededMoral: i32s(A.BolygoNeededMoral, n + 1), lazad: i32s(A.Lazad, n + 1),
    utolso: i32s(A.UtolsoKozeli, n + 1), hanyEpulet: i32s(A.HanyEpulet, n + 1),
    fullGdp: i32s(A.FullGDP, 19), fullTax: i32s(A.FullTax, 19),
    tradeLast: i32s(A.TradeLast, 19), tradeThis: i32s(A.TradeThis, 19),
    taxLast: i32s(A.TaxLast, 19), taxThis: i32s(A.TaxThis, 19),
    story: hex(A.StoryAdat, 0x128), kidnap: i32s(A.Kidnap, 9),
    problems: hex(A.ProblemakLeiro, 480), m: A.M.readS32(),
    labor: i32s(A.Labor, 19 * 4), laborOssz: i32s(A.LaborOssz, 19 * 4),
    kemLehet: i32s(A.KemLehet, 9), kemOssz: i32s(A.KemOssz, 9), kemPer: hex(A.KemPer, 9 * 201 * 4),
    tradePorts: i32s(A.TradePorts, 9),
    foldi: i32s(A.Foldi, 201), erod: i32s(A.Erod, 201), iongun: i32s(A.IonGun, 201), tuzolto: i32s(A.Tuzolto, 201),
    money: hex(A.Money, 19 * 8), pools: hex(A.Pools, 19 * 48), hatar: hex(A.Hatar, 19 * 48), szorzo: hex(A.Szorzo, 19 * 48),
    nehezseg: A.Nehezseg.readS32(),
    el: hex(A.El, 19), ismeri: hex(A.Ismeri, 19 * 19), rndN: A.RndN.readS32(), rndTotal: A.RndTotal.readS32(),
    tech: hex(A.Tech, 193 * 0x114), allTechs: A.AllTechs.readS32(),
    abMitEpit: i32s(A.AbMitEpit, 32),
    fleets: nf > 0 ? hex(fp.add(0x80), nf * 0x80) : '', fleetPerOwner: i32s(A.FlottaPerOwner, 19),
    tanks: nt > 0 ? hex(A.Tankok.add(8), nt * 8) : '',
    ido2: A.Ido2.readS32(), mult: A.Mult.readS32(), mapCursor: hex(A.MapCursor, 8),
    planets,
  };
}

function statics() {
  return {
    k: 'static',
    abProblem: i32s(A.AbProblem, 32), abMin: i32s(A.AbMin, 32), abMax: i32s(A.AbMax, 32),
    abEgyeb: i32s(A.AbEgyeb, 11 * 51), abAlso: i32s(A.AbAlso, 11), abEgyebSz: i32s(A.AbEgyebSz, 11),
    tro: i32s(A.TalalmanyokrEro, 193), hp: i32s(A.TalalmanyEroHP, 193),
    penzSzorzo: Array.from({ length: 5 }, (_, i) => A.PenzSzorzo.add(4 * i).readFloat()),
    pGyartas0: hex(A.pGyartas0, 64),
    // Terepek[i]+0: the surface type of terrain i (exe stride 0x2714).
    terrainKinds: Array.from({ length: A.HanyTerep.readS32() }, (_, i) => A.Terepek.readPointer().add(i * 0x2714).readU8()),
  };
}

const nf = (a, r, args) => new NativeFunction(a, r, args, 'mscdecl');
const betolt = nf(A.Betolt_Scenario, 'void', ['pointer']);
const setSpeed = nf(A.SetIdoSpeed, 'void', ['int', 'int']);
const fellazad = nf(A.BolygoFellazad, 'int', ['int', 'int', 'int']);
const elfoglal = nf(A.BolygoElfoglal, 'void', ['int', 'int']);
const genPlanet0 = nf(A.Generate_Planet0, 'void', ['int', 'int', 'int', 'int']);
const epuletSearch = nf(A.EpuletNewSearch, 'int', ['int', 'int', 'int', 'int', 'int', 'int']);
const rndlog = nf(A.rndlog, 'int', ['int', 'int', 'int', 'pointer', 'int']);
// MoneyMennyi returns its double in x87 st0, which Frida's NativeFunction doesn't read on this ABI:
// call it through a thunk that stores st0 (`thunk(e, k, double *out)`).
const mennyiThunk = Memory.alloc(Process.pageSize);
Memory.patchCode(mennyiThunk, 64, (code) => {
  const w = new X86Writer(code, { pc: mennyiThunk });
  w.putPushReg('ebp');
  w.putMovRegReg('ebp', 'esp');
  w.putPushU32(0); w.putMovRegRegOffsetPtr('eax', 'ebp', 12); w.putMovRegOffsetPtrReg('esp', 0, 'eax');
  w.putPushU32(0); w.putMovRegRegOffsetPtr('eax', 'ebp', 8); w.putMovRegOffsetPtrReg('esp', 0, 'eax');
  w.putCallAddress(A.MoneyMennyi);
  w.putAddRegImm('esp', 8);
  w.putMovRegRegOffsetPtr('ecx', 'ebp', 16);
  w.putBytes([0xdd, 0x19]);                    // fstp qword ptr [ecx]
  w.putPopReg('ebp');
  w.putRet();
  w.flush();
});
const mennyiCall = nf(mennyiThunk, 'void', ['int', 'int', 'pointer']);
const mennyiOut = Memory.alloc(8);
const moneyMennyi = (e, k) => { mennyiCall(e, k, mennyiOut); return mennyiOut.readDouble(); };
const moneyAdd = nf(A.MoneyHozzaad, 'uint', ['int', 'int', 'double']);
const getGyartasAr = nf(va(0x4acaf0), 'int', ['int', 'int']);
// rndlog strips the directory at the last backslash (no backslash: NULL dereference).
const BS = String.fromCharCode(92);
const fileName = Memory.allocAnsiString('..' + BS + 'sources_base' + BS + 'starmap.cpp');

// Deterministic LCG for fuzzing; nothing on disk changes.
let fz = 12345;
const rnd = (n) => { fz = (Math.imul(fz, 1103515245) + 12345) >>> 0; return (fz >>> 8) % n; };
const EVENTS = [200, 202, 204, 0x5d3, 0x91b, 0xc30, -0xc44, -0x1f4a, -0x673, 0xc3a, 0x1f41, 0x1f42,
  -0x1f7c, 100, 0x619, 0x669, 0x64b];

function fuzzPlanet(i) {
  const p = A.Bolygok.add(i * 0x188);
  if (p.add(0x20).readU8() === 0) return;
  p.add(0x34).writeU8(rnd(8));
  if (rnd(3) === 0) p.add(0x28).writeS32(1100 + rnd(80000));
  p.add(0x30).writeS32(rnd(3) === 0 ? rnd(200) : rnd(1000));          // low morale often
  if (rnd(5) === 0) p.add(0xb8).writeS16(rnd(400));
  for (let e = 1; e < 19; e++) if (rnd(12) === 0) p.add(0xc4 + e).writeU8(rnd(3));
  for (let e = 1; e < 19; e++) if (rnd(10) === 0) p.add(0xd8 + 2 * e).writeS16(rnd(3) - 1);
  if (rnd(4) === 0) { p.add(0x174).writeS32(rnd(300)); p.add(0x178).writeS32(rnd(60)); }
  for (let k = 0; k < 6; k++) p.add(0x168 + 2 * k).writeS16(0);
  if (rnd(3) === 0) p.add(0x16a + 2 * rnd(5)).writeS16(EVENTS[rnd(EVENTS.length)]);
  if (rnd(6) === 0) p.add(0x186).writeU8(1); else p.add(0x186).writeU8(0);
  if (rnd(6) === 0) p.add(0x187).writeU8(1); else p.add(0x187).writeU8(0);
  p.add(0x64).writeU8(rnd(8) === 0 ? 1 + rnd(3) : 0);                // whole byte is tested (2, 3 too)
  p.add(0x68).writeS32(rnd(5) === 0 ? -1000 + rnd(1100) : rnd(101));
  p.add(0x6c).writeU8(rnd(8) === 0 ? 0 : 1 + rnd(10));
  p.add(0x6d).writeU8(rnd(8) === 0 ? 0 : 1 + rnd(3));
  A.Lazad.add(4 * i).writeS32(rnd(3) === 0 ? 27 + rnd(5) : rnd(30));
  const nb = A.HanyEpuletLefoglalva.add(4 * i).readS32();
  const arr = A.Epuletek.add(4 * i).readPointer();
  for (let j = 1; j <= nb; j++) {
    const b = arr.add(0x20 * j);
    const kind = b.readU8();
    if (kind === 0) continue;
    if (rnd(6) === 0) b.add(4).writeS32(rnd(60000));
    if (rnd(10) === 0) b.add(0x19).writeU8(b.add(0x19).readU8() ^ 0x20);
    if (rnd(12) === 0) b.add(8).writeU16(rnd(4) === 0 ? 0 : rnd(3000));
    if (rnd(20) === 0) b.writeU8([43, 28, 32, 36, 9, 25, 13, 14, 15, 16, 26, 50, 8, 1, 52][rnd(15)]);
    if (rnd(40) === 0) b.writeU8(0);                                     // empty a slot
  }
  if (rnd(25) === 0) {                                                   // many spy centres: hit the cap of 12
    for (let j = 1; j <= nb; j++) {
      const b = arr.add(0x20 * j);
      if (b.readU8() !== 0 && b.readU8() !== 1 && rnd(2) === 0) { b.writeU8(26); b.add(8).writeU16(0); }
    }
  }
}

function fuzzGlobals() {
  const n = A.HanyBolygo.readS32();
  A.GlobalisMoral.writeS32(rnd(4) === 0 ? rnd(200) - 50 : 0);
  for (let e = 1; e < 9; e++) A.Kidnap.add(4 * e).writeS32(rnd(6) === 0 ? rnd(3) : 0);
  A.StoryAdat.add(0x24).writeS32(rnd(3) === 0 ? 1 + rnd(8) : 0);          // labs also credited to EN
  for (let i = 0; i < 3; i++) A.StoryAdat.add(0x9c + 4 * i).writeS32(rnd(3) === 0 ? rnd(4) : 0);
  for (let a = 1; a < 19; a++) for (let b = 1; b < 19; b++) if (rnd(6) === 0) A.Ismeri.add(a * 19 + b).writeU8(rnd(2));
  for (let e = 1; e < 19; e++) {
    const pools = A.Pools.add(e * 48), hatar = A.Hatar.add(e * 48);
    for (let k = 1; k < 4; k++) {
      pools.add(8 * k).writeDouble(rnd(4) === 0 ? rnd(200) : rnd(60000));
      if (rnd(3) === 0) hatar.add(8 * k).writeDouble(rnd(5000));
    }
    hatar.writeDouble(rnd(4) === 0 ? rnd(3000) : 0);
    A.Money.add(8 * e).writeDouble(pools.add(8).readDouble() + 0 + pools.add(16).readDouble() + pools.add(24).readDouble());
    if (rnd(3) === 0) A.LaborOssz.add(e * 16 + 4 * (1 + rnd(3))).writeS32(rnd(12));
  }
  for (let e = 1; e < 9; e++) {
    if (rnd(3) === 0) A.KemOssz.add(4 * e).writeS32(rnd(16));
    if (rnd(3) === 0) A.TradePorts.add(4 * e).writeS32(rnd(2));
  }
  for (let i = 1; i <= n; i++) if (rnd(2) === 0) fuzzPlanet(i);
}

function fuzzMoney(e) {
  const pools = A.Pools.add(e * 48), hatar = A.Hatar.add(e * 48), sz = A.Szorzo.add(e * 48);
  for (let k = 1; k < 6; k++) pools.add(8 * k).writeDouble(rnd(5) === 0 ? 0 : (rnd(100000) - (rnd(6) === 0 ? 30000 : 0)) / (rnd(2) ? 1 : 7));
  for (let k = 0; k < 4; k++) hatar.add(8 * k).writeDouble(rnd(3) === 0 ? rnd(20000) : 0);
  if (rnd(4) === 0) sz.add(8 * (1 + rnd(3))).writeDouble(rnd(3) === 0 ? 0.005 : rnd(100) / 100);
  if (rnd(4) === 0) {                                     // surplus just above the 0.001 / 0.01 cut-offs
    for (let k = 1; k < 4; k++) pools.add(8 * k).writeDouble(hatar.add(8 * k).readDouble() + [0.0005, 0.005, 0.02][rnd(3)]);
  }
  A.Money.add(8 * e).writeDouble(pools.add(8).readDouble() + 0 + pools.add(16).readDouble() + pools.add(24).readDouble());
}

let frames = 0, started = false, loadedAt = 0, done = 0, seq = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true;
      betolt(Memory.allocAnsiString(cfg.scenario));
      setSpeed(cfg.speed, 1);
      loadedAt = frames;
      emit({ k: 'loaded', ido: A.Ido.readS32() });
      emit(statics());
      return;
    }
    if (!started || cfg.mode === 'economy' || cfg.mode === 'arrive' || done >= cfg.count) return;
    if (frames < loadedAt + 60 || (frames - loadedAt) % cfg.every !== 0) return;
    try { direct(); } catch (e) { emit({ k: 'error', msg: e.toString(), stage }); }
  },
});

let stage = '';
function pickPlanet(owned) {
  const n = A.HanyBolygo.readS32();
  for (let tries = 0; tries < 400; tries++) {
    const p = 1 + rnd(n);
    if ((A.Bolygok.add(p * 0x188 + 0x20).readU8() !== 0) === owned) return p;
  }
  return 0;
}

function direct() {
  const n = A.HanyBolygo.readS32();
  fuzzGlobals();
  if (cfg.mode === 'money') {
    const calls = [];
    const before = snapshotMoney();
    for (let c = 0; c < 20; c++) {
      const e = 1 + rnd(18);
      if (rnd(4) === 0) fuzzMoney(e);
      const k = rnd(5) === 0 ? 0 : 1 + rnd(3);
      const mp = A.MultiPlayerMode.readU8();
      if (rnd(4) === 0) A.MultiPlayerMode.writeU8(1);    // restored right after the call
      if (rnd(4) === 0) {
        const race = A.PlayerFaj.add(e).readU8();
        const r2 = rnd(4) === 0 ? 5 : 1 + rnd(14);
        A.PlayerFaj.add(e).writeU8(r2);
        const t = rnd(4) === 0 ? 13 : 1 + rnd(192);
        calls.push({ f: 'price', e, race: r2, t, r: getGyartasAr(e, t) });
        A.PlayerFaj.add(e).writeU8(race);
      } else if (rnd(3) === 0) {
        const pre = snapshotMoney();
        const r = moneyMennyi(e, k);
        calls.push({ f: 'mennyi', e, k, r, pre });
      } else {
        const x = (rnd(3) === 0 ? 1 : -1) * rnd(rnd(4) === 0 ? 200000 : 20000) / (rnd(3) === 0 ? 3 : 1);
        const pre = snapshotMoney();
        const r = moneyAdd(e, k, x) & 0xff;
        calls.push({ f: 'add', e, k, x, r, pre, post: snapshotMoney() });
      }
      A.MultiPlayerMode.writeU8(mp);
    }
    const prices = Array.from({ length: 193 }, (_, t) => A.Tech.add(t * 0x114 + 0x14).readS32());
    emit({ k: 'money', seq: seq++, before, calls, prices });
    done++;
    return;
  }
  // Conquest and rebellion on owned planets; colonisation on free ones (the game's own cases).
  const p = pickPlanet(cfg.mode !== 'colonise');
  if (p === 0) return;
  // Mode-specific set-up goes before the snapshot.
  const to = rnd(3) === 0 ? 1 + rnd(8) : 0;
  const force = rnd(4) === 0 ? 1 : 0;
  if (cfg.mode === 'fellazad') {
    // Unforced calls at morale 20..2019 so that morale/20 spans the 1..100 roll (the suppression edge).
    if (!force) A.Bolygok.add(p * 0x188 + 0x30).writeS32(20 + rnd(2000));
    A.Bolygok.add(p * 0x188 + 0x187).writeU8(rnd(8) === 0 ? 1 : 0);
  }
  stage = 'snapshot';
  const before = snapshot();
  let args, ret = null;
  if (cfg.mode === 'elfoglal') {
    const owner = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10][rnd(11)];
    args = { p, owner };
    elfoglal(p, owner);
  } else if (cfg.mode === 'fellazad') {
    args = { p, to, force };
    ret = fellazad(p, to, force);
  } else if (cfg.mode === 'colonise') {
    const owner = 1 + rnd(8);
    args = { p, owner };
    stage = 'elfoglal'; elfoglal(p, owner);
    stage = 'gen0'; genPlanet0(p, owner, A.PlayerFaj.add(owner).readU8(), 0);
    stage = 'rndlog'; const d = rndlog(1, 0, 0, fileName, 0);
    stage = 'search'; ret = epuletSearch(p, 1, 0, d, -1, -1);
    stage = 'after';
  } else {
    return;
  }
  emit({ k: cfg.mode, seq: seq++, args, ret, before, after: snapshot() });
  done++;
}

function snapshotMoney() {
  return { money: hex(A.Money, 19 * 8), pools: hex(A.Pools, 19 * 48), hatar: hex(A.Hatar, 19 * 48),
    szorzo: hex(A.Szorzo, 19 * 48), nehezseg: A.Nehezseg.readS32(), playerKi: hex(A.PlayerKi, 19),
    mp: A.MultiPlayerMode.readU8() };
}

let rnds = null;
// arrive: stage colonising/terraforming arrivals for this FRISSIT_Starmap call.
function stageArrivals() {
  const fl = A.Flottak.readPointer(), nf = A.FlottaN.readS32(), np = A.HanyBolygo.readS32();
  const en = A.EN.readS32();
  const ais = [];
  for (let e = 1; e <= 8; e++) if (e !== en && A.El.add(e).readU8()) ais.push(e);
  const live = [];
  for (let i = 1; i <= nf; i++) {
    const o = fl.add(0x80 * i + 0x15).readU8();
    if (o !== 0xff && o !== en) live.push(i);
  }
  const staged = [];
  let p = 0;
  const n = 1 + rnd(2);
  // Half of the two-fleet ticks send both to one planet, the colonist on the lower fleet index: the
  // later fleet must see the planet already owned (mid-loop effect, fleet-movement.md 4.3).
  const shared = n === 2 && rnd(2) === 0;
  const picks = [];
  for (let k = 0; k < n && live.length; k++) picks.push(live.splice(rnd(live.length), 1)[0]);
  if (shared) picks.sort((x, y) => x - y);
  for (let k = 0; k < picks.length && ais.length; k++) {
    const i = picks[k];
    if (p === 0 || !shared) p = pickPlanet(false);
    if (p === 0) break;
    const f = fl.add(0x80 * i), b = A.Bolygok.add(0x188 * p);
    const terra = shared ? k === 1 && rnd(2) === 0 : rnd(3) === 0;
    const owner = ais[rnd(ais.length)];
    f.add(0x15).writeU8(owner);
    // Give the owner another race now and then: terraforming reads FajBolygoViszony by the owner's
    // empire number, colonisation uses PlayerFaj (colony-side.md 5, 5.1); this tells them apart.
    if (rnd(2) === 0) A.PlayerFaj.add(owner).writeU8(1 + rnd(14));
    f.add(0x18).writeU8(f.add(0x18).readU8() & ~3);               // not hidden, not scripted
    const flags = f.add(0x52).readU16() & ~(1 | 8 | 0x10);         // not in battle
    f.add(0x52).writeU16(flags | (terra ? 0x10 : 8));
    f.add(terra ? 0x4e : 0x4c).writeS16(1);
    f.add(0x40).writeS16(1); f.add(0x42).writeS16(1); f.add(0x44).writeFloat(p); f.add(0x48).writeFloat(0);
    f.add(0x30).writeFloat(b.add(0x18).readFloat()); f.add(0x34).writeFloat(b.add(0x1c).readFloat() + 2);
    if (f.add(0x38).readFloat() <= 0) f.add(0x38).writeFloat(0.5);
    staged.push({ fleet: i, planet: p, terra });
  }
  return staged;
}

let starmapTicks = 0;
Interceptor.attach(A.FRISSIT_Starmap, {
  onEnter() {
    this.on = false;
    if (cfg.mode !== 'arrive' || !started || done >= cfg.count || frames < loadedAt + 60) return;
    starmapTicks++;
    if (A.Ido2.readS32() % 10 === 1 || starmapTicks % cfg.every !== 0) return;
    if (cfg.fuzz) fuzzGlobals();
    const staged = stageArrivals();
    if (!staged.length) return;
    this.on = true;
    this.staged = staged;
    this.before = snapshot();
    rnds = [];
  },
  onLeave() {
    if (!this.on) return;
    emit({ k: 'arrive', seq: seq++, staged: this.staged, before: this.before, after: snapshot(), rnds });
    rnds = null;
    done++;
  },
});

let depth = 0;
Interceptor.attach(A.FRISSIT_Planet, {
  onEnter(args) {
    this.full = args[0].toInt32() & 0xff;
    this.only = args[1].toInt32();
    this.outer = cfg.mode === 'economy' && depth === 0 && this.full !== 0 && this.only === 0;
    depth++;
    if (!this.outer) {
      if (rnds) rnds.push({ nested: this.only, full: this.full });
      return;
    }
    if (cfg.fuzz) fuzzGlobals();
    this.before = snapshot();
    rnds = [];
  },
  onLeave() {
    depth--;
    if (!this.outer) return;
    emit({ k: 'economy', seq: seq++, before: this.before, after: snapshot(), rnds });
    rnds = null;
  },
});

Interceptor.attach(A.rndlog, {
  onEnter(args) {
    if (!rnds) return;
    this.on = true;
    this.s = args[0].toInt32(); this.lo = args[1].toInt32(); this.hi = args[2].toInt32();
    this.line = args[4].toInt32();
  },
  onLeave(ret) {
    if (this.on) rnds.push({ s: this.s, lo: this.lo, hi: this.hi, line: this.line, r: ret.toInt32() });
  },
});

send({ type: 'ready', base: mod.base.toString() });
