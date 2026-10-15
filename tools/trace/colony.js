// Frida agent: capture the colony economy (FRISSIT_Planet) of the GOG ig2.exe (PE32, MSVC 2013).
// For every outer economy call FRISSIT_Planet(true, 0) it snapshots all owned planets, their
// building arrays, the RNG streams and the economy globals before and after, plus every rndlog
// made inside the call. Spec: re/rules/colony-economy.md. Addresses are image VAs, rebased at runtime.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720),
  Betolt_Scenario: va(0x4f5ba0),
  SetIdoSpeed: va(0x541330),
  FRISSIT_Planet: va(0x474930),   // FRISSIT_Planet(bool full, int only_planet), cdecl
  rndlog: va(0x542da0),
  RandSeed: va(0xbaf2d0),
  Ido: va(0xbae04c), Ev: va(0xbae040), Honap: va(0xbae044), Nap: va(0xbae048),
  Bolygok: va(0xb85ea8),          // BolygoTipus[201], 0x188 bytes each, index 1..HanyBolygo
  HanyBolygo: va(0x912ac8),
  HanyEpuletLefoglalva: va(0xba9840), // i32[201]: allocated building slots per planet
  Epuletek: va(0xba9e90),         // ptr[201]: building array per planet (slot j at +0x20*j)
  EN: va(0x912a3c),               // human player's empire index
  MultiPlayerMode: va(0xbae2c0),
  PlayerKi: va(0xbae560),         // u8[19]: controller per empire (1 = human)
  PlayerFaj: va(0xbae574),        // u8[19]: race per empire
  ProgramPlayers2: va(0x912ad0),
  GlobalisMoral: va(0xbaeac4), GlobalisMoralSebesseg: va(0xbaeb14),
  BolygoNeededMoral: va(0xb9a630),
  BolygoMennyiIdejeLazad: va(0xb99668),
  FullGDP: va(0xbae060), FullTax: va(0xbae0b0),
  StoryAdat: va(0xf273f8),        // 0x128 bytes
  CsaladtagElrabolvaKi: va(0xbb405c),
  ProblemakLeiro: va(0x912858),   // 20 x 24 bytes; entry 4 rate/weight is rewritten at runtime
  M: va(0xa3eb74),                // FRISSIT_Planet::M (population momentum, static)
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, speed: 3 };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 20) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 500);

function hex(p, n) {
  const b = new Uint8Array(p.readByteArray(n));
  let s = '';
  for (let i = 0; i < b.length; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}
const i32s = (p, n) => Array.from({ length: n }, (_, i) => p.add(4 * i).readS32());

function snapshot(indices) {
  const n = A.HanyBolygo.readS32();
  const planets = [];
  for (let i = 1; i <= n; i++) {
    const p = A.Bolygok.add(i * 0x188);
    if (indices ? !indices.has(i) : p.add(0x20).readU8() === 0) continue;
    const nb = A.HanyEpuletLefoglalva.add(4 * i).readS32();
    const arr = A.Epuletek.add(4 * i).readPointer();
    planets.push({ i, raw: hex(p, 0x188), nb, bld: nb > 0 ? hex(arr.add(0x20), nb * 0x20) : '' });
  }
  return {
    ido: A.Ido.readS32(), ev: A.Ev.readS32(), honap: A.Honap.readS32(), nap: A.Nap.readS32(),
    seeds: Array.from({ length: 21 }, (_, i) => A.RandSeed.add(4 * i).readU32()),
    en: A.EN.readS32(), mp: A.MultiPlayerMode.readU8(), players2: A.ProgramPlayers2.readS32(),
    playerKi: hex(A.PlayerKi, 19), playerFaj: hex(A.PlayerFaj, 19),
    gmoral: A.GlobalisMoral.readS32(), gmoralSpeed: A.GlobalisMoralSebesseg.readS32(),
    neededMoral: i32s(A.BolygoNeededMoral, n + 1), lazad: i32s(A.BolygoMennyiIdejeLazad, n + 1),
    fullGdp: i32s(A.FullGDP, 19), fullTax: i32s(A.FullTax, 19),
    story: hex(A.StoryAdat, 0x128), kidnap: i32s(A.CsaladtagElrabolvaKi, 9),
    problems: hex(A.ProblemakLeiro, 480), m: A.M.readS32(),
    planets,
  };
}

const betolt = new NativeFunction(A.Betolt_Scenario, 'void', ['pointer'], 'mscdecl');
const setSpeed = new NativeFunction(A.SetIdoSpeed, 'void', ['int', 'int'], 'mscdecl');
let frames = 0, started = false;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true;
      betolt(Memory.allocAnsiString(cfg.scenario));
      setSpeed(cfg.speed, 1);
      emit({ k: 'loaded', ido: A.Ido.readS32() });
    }
  },
});

// --fuzz: before each economy call, push the live game into rarely seen states so the exe itself
// computes the outcome (coverage for the golden test). Deterministic LCG; nothing on disk changes.
let fz = 12345;
const rnd = (n) => { fz = (Math.imul(fz, 1103515245) + 12345) >>> 0; return (fz >>> 8) % n; };
const EVENTS = [200, 201, 202, 203, 204, 0x5d3, 0x91b, 0xc30, -0xc44, -0x1f4a, -0x673, 0xc3a, 0x1f41,
  0x1f42, -0x1f7c];
function fuzz() {
  const n = A.HanyBolygo.readS32();
  A.GlobalisMoral.writeS32(rnd(4) === 0 ? rnd(200) - 50 : 0);
  for (let e = 1; e < 9; e++) A.CsaladtagElrabolvaKi.add(4 * e).writeS32(rnd(6) === 0 ? rnd(3) : 0);
  for (let i = 1; i <= n; i++) {
    const p = A.Bolygok.add(i * 0x188);
    if (p.add(0x20).readU8() === 0 || rnd(2) === 0) continue;
    p.add(0x34).writeU8(rnd(8));                                       // tax level
    if (rnd(3) === 0) p.add(0x28).writeS32(1100 + rnd(80000));         // population
    if (rnd(3) === 0) p.add(0x30).writeS32(rnd(1000));                 // morale
    if (rnd(5) === 0) p.add(0xb8).writeS16(rnd(400));                  // epidemic
    if (rnd(4) === 0) p.add(0xc4 + 1 + rnd(8)).writeU8(1 + rnd(2));    // a spy
    if (rnd(4) === 0) { p.add(0x174).writeS32(rnd(300)); p.add(0x178).writeS32(rnd(60)); }
    for (let k = 0; k < 6; k++) p.add(0x168 + 2 * k).writeS16(0);
    if (rnd(3) === 0) p.add(0x168 + 2 * rnd(6)).writeS16(EVENTS[rnd(EVENTS.length)]);
    A.BolygoMennyiIdejeLazad.add(4 * i).writeS32(rnd(4) === 0 ? rnd(29) : 0);
    const nb = A.HanyEpuletLefoglalva.add(4 * i).readS32();
    const arr = A.Epuletek.add(4 * i).readPointer();
    for (let j = 1; j <= nb; j++) {
      const b = arr.add(0x20 * j);
      const kind = b.readU8();
      if (kind === 0) continue;
      if (rnd(6) === 0) b.add(4).writeS32(rnd(60000));                 // damage
      if (rnd(8) === 0) b.add(0x18).writeU8(50 + rnd(100));            // yield %
      if (rnd(10) === 0) b.add(0x19).writeU8(b.add(0x19).readU8() ^ 0x20); // switch off/on
      if (rnd(25) === 0) b.writeU8([43, 28, 29, 30, 31, 9, 25, 13][rnd(8)]); // swap type
    }
  }
}

let depth = 0, seq = 0, rnds = null;
Interceptor.attach(A.FRISSIT_Planet, {
  onEnter(args) {
    this.full = args[0].toInt32() & 0xff;
    this.only = args[1].toInt32();
    this.outer = depth === 0 && this.full !== 0 && this.only === 0;
    depth++;
    if (!this.outer) {
      if (rnds) rnds.push({ nested: this.only, full: this.full });
      return;
    }
    if (cfg.fuzz) fuzz();
    this.before = snapshot(null);
    this.idx = new Set(this.before.planets.map((p) => p.i));
    rnds = [];
  },
  onLeave() {
    depth--;
    if (!this.outer) return;
    const after = snapshot(this.idx);
    emit({ k: 'economy', seq: seq++, before: this.before, after, rnds });
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
