// Frida agent: space-combat rules on the GOG ig2.exe (spec re/rules/space-combat.md).
// Modes (config.mode):
//   design  - call TervUjraSzamol (0x5423b0) on live design slots with (optionally fuzzed) hulls, pods,
//             pod-table index, race and old price; snapshot the 0x84-byte record before/after, then restore.
//   auto    - call SimulateSpBattle (0x5a5e60) on fleet-vs-fleet and fleet-vs-planet pairs; snapshot
//             ships, vehicles, both fleets (and fleet 0), the planet, its buildings, the designs of both
//             owners, RNG streams and every rndlog before/after. --fuzz reshuffles compositions first.
//   indit   - call SpaceWarIndit (0x5a7670) on random pairs and record which path it took (simulate /
//             tactical); a tactical battle is left frozen in its spw slot (no screen switch), then the
//             following FRISSIT_Starmap ticks are recorded with the battle unit lists and fleet flags
//             (the in-battle marking loop after the radar block).
// Addresses are image VAs, rebased at runtime. Output holds game state: keep it under _local/.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720), Betolt_Scenario: va(0x4f5ba0),
  SetIdoSpeed: va(0x541330), SetIdoPause: va(0x541300),
  FRISSIT_Starmap: va(0x594ae0), rndlog: va(0x542da0),
  TervUjraSzamol: va(0x5423b0), SimulateSpBattle: va(0x5a5e60), SpaceWarIndit: va(0x5a7670),
  SpaceWar_NewBattle: va(0x563700), FlottakUjraSzamol: va(0x53dd90), GroundWarIndit: va(0x59c2d0),
  RandSeed: va(0xbaf2d0), Ido2: va(0xbae050),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8),
  Hajok: va(0xbb4100), HajoCount: va(0xbb4104),
  Tankok: va(0xbb41a8), TankCount: va(0xbb4154),          // static array, 8 B, 1-based
  Tervek: va(0xb9aa18), TervCount: va(0xb9c028),           // pointer / i32 per empire
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8),
  HanyEpulet: va(0xba9840), Epuletek: va(0xba9e90),
  IonGun: va(0xb99fe0),                                    // i32 per planet
  PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574), EN: va(0x912a3c), MP: va(0xbae2c0),
  MpBattleStatus: va(0xbae558), DebugAlwaysSim: va(0xb77230), DebugNoLopsided: va(0xb77238),
  StorySpWarBan: va(0xf42664), StorySimBan: va(0xf4266c), StoryNoDiplo: va(0xf27480),
  SpWarKiNyert: va(0xbaa1ec), spKozeliBolygo: va(0xbaa1e4), AktKepKell: va(0xbae250),
  CentreX: va(0xdb2848), CentreY: va(0xdb2898),
  Spw: va(0xe423e0),                                       // battle b: units ptr +0, count +8, active +0x130e; stride 0x2d9c
  Talalmanyok: va(0xb9c0c8),
  FlottaNew: va(0x53d400), HajoNew: va(0x53e800), TankNew: va(0x541750), // as Legyart (0x59d740) uses them
};
const SPW_STRIDE = 0x2d9c, UNIT = 600;

const cfg = { mode: 'auto', scenario: 'scenario/quick.scn', startAfterFrames: 1200, count: 200, fuzz: false,
  every: 6, ticks: 300, seed: 4242 };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 5 || cfg.mode === 'indit') flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

function hex(p, n) {
  const b = new Uint8Array(p.readByteArray(n));
  let s = '';
  for (let i = 0; i < b.length; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}
let fz = 1;
// xorshift32 (the earlier LCG's low bits cycled badly).
const rnd = (n) => { fz ^= fz << 13; fz >>>= 0; fz ^= fz >>> 17; fz ^= fz << 5; fz >>>= 0; return fz % n; };

const betolt = new NativeFunction(A.Betolt_Scenario, 'void', ['pointer'], { abi: 'mscdecl', traps: 'all' });
const setSpeed = new NativeFunction(A.SetIdoSpeed, 'void', ['int', 'int'], { abi: 'mscdecl', traps: 'all' });
const setPause = new NativeFunction(A.SetIdoPause, 'void', ['int', 'int'], { abi: 'mscdecl', traps: 'all' });
const terv = new NativeFunction(A.TervUjraSzamol, 'void', ['int', 'int', 'int', 'int'], { abi: 'mscdecl', traps: 'all' });
const simulate = new NativeFunction(A.SimulateSpBattle, 'int', ['int', 'int', 'int'], { abi: 'mscdecl', traps: 'all' });
const indit = new NativeFunction(A.SpaceWarIndit, 'void', ['int', 'int', 'int'], { abi: 'mscdecl', traps: 'all' });
const recount = new NativeFunction(A.FlottakUjraSzamol, 'void', [], { abi: 'mscdecl', traps: 'all' });

const flottaNew = new NativeFunction(A.FlottaNew, 'int', ['pointer', 'int', 'int', 'float', 'float', 'float'], { abi: 'mscdecl', traps: 'all' });
const hajoNew = new NativeFunction(A.HajoNew, 'void', ['int', 'int'], { abi: 'mscdecl', traps: 'all' });
const tankNew = new NativeFunction(A.TankNew, 'void', ['int', 'int', 'int'], { abi: 'mscdecl', traps: 'all' });
const fleetName = Memory.allocAnsiString('Probe');

const fleetPtr = (i) => A.Flottak.readPointer().add(0x80 * i);
const owner = (i) => fleetPtr(i).add(0x15).readU8();
const nFleets = () => A.FlottaCount.readS32();
const designs = (e) => A.Tervek.add(4 * e).readPointer();
const seeds = () => Array.from({ length: 21 }, (_, i) => A.RandSeed.add(4 * i).readU32());

function liveFleets(pred) {
  const out = [];
  for (let i = 1; i <= nFleets(); i++) { const o = owner(i); if (o !== 0xff && o >= 1 && o <= 9 && (!pred || pred(i))) out.push(i); }
  return out;
}

// quick.scn has no warships (only 8 unarmed traders): give every empire with planets and designs a few
// fleets of its own designs next to its planets, built the way production does (Legyart).
function populate() {
  const np = A.HanyBolygo.readS32();
  for (let e = 1; e <= 8; e++) {
    const cnt = A.TervCount.add(4 * e).readS32();
    if (cnt < 1) continue;
    const ships = [], vehicles = [];
    for (let d = 1; d <= cnt; d++) {
      const p = designs(e).add(0x84 * d), cls = p.add(0x16).readS16();
      if (p.add(0x2c).readS32() <= 0 && p.add(0x30).readS32() <= 0) continue;
      if (cls === 1 && p.add(0x36).readU8() === 0) ships.push(d); else if (cls === 1 || cls === 2) vehicles.push(d);
    }
    if (!ships.length) continue;
    const mine = [];
    for (let q = 1; q <= np; q++) if (A.Bolygok.add(0x188 * q + 0x20).readU8() === e) mine.push(q);
    if (!mine.length) continue;
    for (let k = 0; k < 4; k++) {
      const q = A.Bolygok.add(0x188 * mine[rnd(mine.length)]);
      const f = flottaNew(fleetName, e, 0, q.add(0x18).readFloat() + rnd(9) - 4, q.add(0x1c).readFloat() + rnd(9) - 4, 0);
      if (f < 1) continue;
      for (let j = 0; j < 1 + rnd(8); j++) hajoNew(f, ships[rnd(ships.length)]);
      if (vehicles.length && rnd(2)) tankNew(f, vehicles[rnd(vehicles.length)], 1 + rnd(5));
    }
  }
  emit({ k: 'populated', fleets: nFleets(), ships: A.HajoCount.readS32() });
}

// ---------------------------------------------------------------- design
const INTERESTING = [0x11, 0x14, 0x18, 0x1c, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x27, 0x31, 0x32, 0x33,
  0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x41, 0x44, 0x48, 0x91, 0x95, 0xa1, 0xa5];
function designProbe(n) {
  do { n = designPass(n); } while (cfg.fuzz && n > 0);
}
function designPass(n) {
  for (let e = 1; e <= 8 && n > 0; e++) {
    const cnt = A.TervCount.add(4 * e).readS32();
    for (let d = 1; d <= cnt && n > 0; d++) {
      const p = designs(e).add(0x84 * d);
      const saved = p.readByteArray(0x84);
      const race = A.PlayerFaj.add(e).readU8();
      let flag = 1;
      if (cfg.fuzz) {
        p.add(0x44).writeS32(rnd(25));
        p.add(0x40).writeS16(1 + rnd(18));
        for (let k = 1; k <= 12; k++) {
          p.add(0x50 + k).writeU8(rnd(3) === 0 ? 0 : (rnd(2) ? INTERESTING[rnd(INTERESTING.length)] : rnd(193)));
          p.add(0x5d + k).writeU8(rnd(6));
        }
        p.add(0x24).writeS32(rnd(100000) - 1000);
        p.add(0x28).writeU8(rnd(256));
        flag = rnd(2);
        A.PlayerFaj.add(e).writeU8(1 + rnd(15));
      }
      const before = hex(p, 0x84), faj = A.PlayerFaj.add(e).readU8();
      terv(e, d, 0, flag);
      emit({ k: 'design', e, d, race: faj, flag, before, after: hex(p, 0x84) });
      p.writeByteArray(saved);
      A.PlayerFaj.add(e).writeU8(race);
      n--;
    }
  }
  return n;
}

// ---------------------------------------------------------------- auto battles
function planetState(pl) {
  if (pl < 1) return null;
  const nb = A.HanyEpulet.add(4 * pl).readS32();
  return { i: pl, raw: hex(A.Bolygok.add(0x188 * pl), 0x188), nb,
    buildings: nb > 0 ? hex(A.Epuletek.add(4 * pl).readPointer(), 0x20 * (nb + 1)) : '' };
}
function snapshotAuto(f1, f2, pl) {
  const nh = A.HajoCount.readS32(), nt = A.TankCount.readS32();
  const o1 = owner(f1), o2 = f2 ? owner(f2) : A.Bolygok.add(0x188 * pl + 0x20).readU8();
  const des = {};
  for (const e of [o1, o2]) {
    if (e === 0 || e === 0xff || e in des) continue;
    const c = A.TervCount.add(4 * e).readS32();
    des[e] = hex(designs(e), 0x84 * (c + 1));
  }
  return { seeds: seeds(), ships: nh > 0 ? hex(A.Hajok.readPointer(), 6 * (nh + 1)) : '', nships: nh,
    tanks: hex(A.Tankok, 8 * (nt + 1)), ntanks: nt,
    fleet0: hex(fleetPtr(0), 0x80), fleet1: hex(fleetPtr(f1), 0x80), fleet2: f2 ? hex(fleetPtr(f2), 0x80) : '',
    planet: planetState(pl), designs: des,
    playerKi: hex(A.PlayerKi, 19), en: A.EN.readS32(),
    centreX: hex(A.CentreX, 76), centreY: hex(A.CentreY, 76), nfleet: nFleets() };
}

function fuzzAuto(f1, f2, pl) {
  const hp = A.Hajok.readPointer(), nh = A.HajoCount.readS32();
  for (const f of [f1, f2]) {
    if (!f || owner(f) > 8) continue; // traders (9) never carry warships; fuzzing them crashed the exe
    const e = owner(f), cnt = A.TervCount.add(4 * e).readS32();
    if (cnt < 1) continue;
    // Pull a few ships of the same owner into this fleet, re-design a few.
    for (let k = 0; k < 1 + rnd(6) && nh > 0; k++) {
      const h = 1 + rnd(nh), fi = hp.add(6 * h).readS16();
      if (hp.add(6 * h + 2).readS16() === 0x7fff || fi < 1 || fi > nFleets() || owner(fi) !== e) continue;
      hp.add(6 * h).writeS16(f);
      if (rnd(2)) hp.add(6 * h + 2).writeS16(1 + rnd(cnt));
    }
    // Vehicles (fighters class 1 / ground class 2 designs, any design really).
    for (let k = 0; k < rnd(3); k++) {
      const nt = A.TankCount.readS32();
      let t = 1 + rnd(nt + 1);
      if (t > nt) { if (nt >= 900) continue; A.TankCount.writeS32(nt + 1); t = nt + 1; }
      const r = A.Tankok.add(8 * t);
      r.writeS16(f); r.add(2).writeS16(1 + rnd(cnt)); r.add(4).writeS32(1 + rnd(6));
    }
  }
  if (pl > 0) {
    const p = A.Bolygok.add(0x188 * pl), e = p.add(0x20).readU8();
    const cnt = e ? A.TervCount.add(4 * e).readS32() : 0;
    const n = cnt ? rnd(6) : 0;
    p.add(0x114).writeS32(n);
    for (let k = 1; k <= n; k++) p.add(0xfe + 2 * k).writeS16(1 + rnd(cnt));
    const nb = A.HanyEpulet.add(4 * pl).readS32();
    for (let k = 0; k < 3 && nb > 0; k++) {
      const b = A.Epuletek.add(4 * pl).readPointer().add(0x20 * (1 + rnd(nb)));
      if (b.readU8() === 0) continue;
      b.writeU8(28 + rnd(4));
      b.add(8).writeS16(rnd(4) ? 0 : 5);
    }
  }
  recount();
  A.RandSeed.add(4 * 7).writeU32((rnd(65536) << 16 | rnd(65536)) >>> 0);
}

function ownedPlanet(not) {
  const np = A.HanyBolygo.readS32();
  for (let k = 0; k < 40; k++) {
    const p = 1 + rnd(np), o = A.Bolygok.add(0x188 * p + 0x20).readU8();
    if (o >= 1 && o <= 8 && o !== not) return p;
  }
  return 0;
}
// Put the battle's ships, vehicles, fleets and planet back afterwards, so quick.scn's few fleets
// survive hundreds of probes (each probe still starts from the game's real, possibly fuzzed, state).
function saveWorld(f1, f2, pl) {
  const keep = [];
  const grab = (p, n) => keep.push([p, p.readByteArray(n)]);
  const nh = A.HajoCount.readS32(), nt = A.TankCount.readS32();
  grab(A.Hajok.readPointer(), 6 * (nh + 1));
  grab(A.Tankok, 8 * (nt + 1));
  grab(A.TankCount, 4);
  for (const f of [0, f1, f2]) grab(fleetPtr(f), 0x80);
  const others = [];
  for (let i = 1; i <= nFleets(); i++) others.push(owner(i));
  // Buildings are not restored: EpuletDelete also updates counters and the colony map.
  if (pl > 0) grab(A.Bolygok.add(0x188 * pl), 0x188);
  return () => {
    for (const [p, b] of keep) p.writeByteArray(b);
    // Fleets deleted as followers' targets etc. keep their slots; only the three records are restored.
    for (let i = 1; i <= others.length; i++) if (i !== f1 && i !== f2 && owner(i) !== others[i - 1]) fleetPtr(i).add(0x15).writeU8(others[i - 1]);
  };
}

function autoProbe() {
  const fl = liveFleets((i) => fleetPtr(i).add(0x54).readS32() > 0);
  const emp = fl.filter((i) => owner(i) <= 8);
  if (fl.length < 2) return false;
  const pool = emp.length && rnd(5) ? emp : fl;
  const f1 = pool[rnd(pool.length)], o1 = owner(f1);
  let f2 = 0, pl = 0;
  if (rnd(10) < 3) {
    pl = ownedPlanet(o1);
  } else {
    const others = fl.filter((i) => owner(i) !== o1);
    if (!others.length) return false;
    f2 = others[rnd(others.length)];
    if (rnd(3) === 0) {
      // fleet-vs-fleet at a planet: SimulateSpBattle also gets the planet's defences.
      const np = A.HanyBolygo.readS32();
      for (let k = 0; k < 40 && !pl; k++) { const p = 1 + rnd(np); if (A.Bolygok.add(0x188 * p + 0x20).readU8() === owner(f2)) pl = p; }
    }
  }
  if (!f2 && !pl) return false;
  if (cfg.fuzz) fuzzAuto(f1, f2, pl);
  const before = snapshotAuto(f1, f2, pl);
  const saved = saveWorld(f1, f2, pl);
  if (cfg.fuzz) { emit({ k: 'pre', f1, f2, pl, before }); flush(); }
  rnds = [];
  const w = simulate(f1, f2, pl);
  const r = rnds; rnds = null;
  emit({ k: 'auto', f1, f2, pl, winner: w, kinyert: A.SpWarKiNyert.readS32(), before,
    after: snapshotAuto(f1, f2, pl), afterOwner1: owner(f1), afterOwner2: f2 ? owner(f2) : -1, rnds: r });
  saved();
  return true;
}

// ---------------------------------------------------------------- SpaceWarIndit + in-battle marking
let tookSim = false, tookTac = false;
Interceptor.attach(A.SimulateSpBattle, { onEnter() { tookSim = true; } });
Interceptor.attach(A.SpaceWar_NewBattle, { onEnter() { tookTac = true; } });
// Ground wars after a planet battle would leave the starmap; neutralise them during probes.
let probing = false;
Interceptor.attach(A.GroundWarIndit, { onEnter() { if (probing) emit({ k: 'groundwar' }); } });

function fleetBrief(i) {
  const f = fleetPtr(i);
  return { i, raw: hex(f, 0x80) };
}
function inditProbe() {
  const fl = liveFleets((i) => fleetPtr(i).add(0x54).readS32() > 0 && fleetPtr(i).add(0x5c).readS32() === 0);
  if (fl.length < 2) return false;
  const en = A.EN.readS32();
  const human = fl.filter((i) => owner(i) === en), ai = fl.filter((i) => owner(i) !== en);
  const emp = ai.filter((i) => owner(i) <= 8);
  let f1 = (human.length && rnd(2) === 0) ? human[rnd(human.length)] : (emp.length && rnd(4) ? emp[rnd(emp.length)] : fl[rnd(fl.length)]);
  const others = fl.filter((i) => owner(i) !== owner(f1));
  if (!others.length) return false;
  let f2 = others[rnd(others.length)], pl = 0;
  if (rnd(5) === 0) {
    const np = A.HanyBolygo.readS32(), p = 1 + rnd(np);
    if (A.Bolygok.add(0x188 * p + 0x20).readU8() !== owner(f1) && A.Bolygok.add(0x188 * p + 0x20).readU8() !== 0) { pl = p; f2 = 0; }
  }
  if (cfg.fuzz) {
    A.DebugAlwaysSim.writeS32(rnd(8) === 0 ? 1 : 0);
    A.DebugNoLopsided.writeS32(rnd(8) === 0 ? 1 : 0);
    A.StorySimBan.writeS32(rnd(8) === 0 ? 1 : 0);
    const np = A.HanyBolygo.readS32();
    for (let k = 0; k < 6; k++) A.IonGun.add(4 * (1 + rnd(np))).writeS32(rnd(3) ? 0 : rnd(4000));
  }
  const np = A.HanyBolygo.readS32();
  const planets = [];
  for (let p = 1; p <= np; p++) { const b = A.Bolygok.add(0x188 * p); planets.push([b.add(0x18).readFloat(), b.add(0x1c).readFloat(), b.add(0x20).readU8(), A.IonGun.add(4 * p).readS32()]); }
  const before = { f1: fleetBrief(f1), f2: f2 ? fleetBrief(f2) : null, planetOwner: pl ? A.Bolygok.add(0x188 * pl + 0x20).readU8() : 0,
    playerKi: hex(A.PlayerKi, 19), en, mp: A.MP.readU8(), mpStatus: A.MpBattleStatus.readS32(),
    dbgSim: A.DebugAlwaysSim.readS32(), dbgNoLop: A.DebugNoLopsided.readS32(), simBan: A.StorySimBan.readS32(),
    spwBan: A.StorySpWarBan.readS32(), planets };
  const restore = saveWorld(f1, f2, pl);
  const kep = A.AktKepKell.readS32();
  const s7 = A.RandSeed.add(28).readU32();
  const slots = () => { let n = 0; for (let b = 0; b < 10; b++) { const base = A.Spw.add(SPW_STRIDE * b); if (base.add(0x130e).readU8()) n += 1000 + base.add(8).readS32(); } return n; };
  const sl = slots();
  // A dummy move order on f1: the real-time path clears it (FlottaMozgat(f, 0, ...)) even when all
  // 10 battle slots are taken and SpaceWar_NewBattle fails.
  const fp = fleetPtr(f1);
  fp.add(0x40).writeS16(1); fp.add(0x42).writeS16(0);
  fp.add(0x44).writeFloat(fp.add(0x30).readFloat()); fp.add(0x48).writeFloat(fp.add(0x34).readFloat());
  tookSim = false; tookTac = false; probing = true;
  indit(f1, f2, pl);
  probing = false;
  // Interceptor hooks don't fire inside our own NativeFunction call; detect the path from its effects:
  // the auto-resolve always rolls stream 7, the real-time battle sets screen 0xB or fills a slot.
  const sim = tookSim || A.RandSeed.add(28).readU32() !== s7;
  const tac = tookTac || A.AktKepKell.readS32() === 0xb || slots() !== sl
    || (owner(f1) !== 0xff && fp.add(0x40).readS16() === 0);
  if (tac) { A.AktKepKell.writeS32(kep); setPause(0, 0); setSpeed(3, 1); }
  else if (sim) restore(); // keep fleets for later probes (a real-time battle references its ships)
  emit({ k: 'indit', f1, f2, pl, near: A.spKozeliBolygo.readS32(), sim, tactical: tac, before });
  return true;
}

function battles() {
  const out = [];
  for (let b = 0; b < 10; b++) {
    const base = A.Spw.add(SPW_STRIDE * b);
    const active = base.add(0x130e).readU8();
    if (!active) continue;
    const n = base.add(8).readS32(), arr = base.readPointer();
    const units = [];
    for (let u = 0; u < n; u++) {
      const p = arr.add(UNIT * u);
      const cnt = p.add(0x1b0).readS32(), sp = p.add(0x28).readPointer();
      const ships = [];
      for (let k = 0; k < cnt && k < 10000 && !sp.isNull(); k++) ships.push(sp.add(4 * k).readS32());
      units.push({ alive: p.readU8(), owner: p.add(0x14).readS32(), ships });
    }
    out.push({ b, units });
  }
  return out;
}
function fleetFlags() {
  const out = [];
  for (let i = 0; i <= nFleets(); i++) out.push([owner(i), fleetPtr(i).add(0x52).readU16()]);
  return out;
}
function fuzzBattles() {
  for (const bt of battles()) {
    const base = A.Spw.add(SPW_STRIDE * bt.b), arr = base.readPointer();
    for (let u = 0; u < bt.units.length; u++) {
      const p = arr.add(UNIT * u);
      switch (rnd(10)) {
        case 0: p.writeU8(0); break;
        case 1: p.writeU8(2); break;            // exe tests != 0 (ELF: & 1)
        case 2: p.writeU8(1); break;
        case 3: p.add(0x14).writeS32(0); break;  // scenery owner
        default: break;
      }
    }
  }
  // Clear bit0 on random fleets so non-refresh ticks show the marking too.
  for (let i = 1; i <= nFleets(); i++) if (rnd(2)) { const f = fleetPtr(i).add(0x52); f.writeU16(f.readU16() & 0xfffe); }
}

let tick = 0, recorded = 0, rnds = null;
Interceptor.attach(A.FRISSIT_Starmap, {
  onEnter() {
    this.on = false;
    if (cfg.mode !== 'indit' || phase !== 2 || recorded >= cfg.ticks) return;
    tick++;
    if (cfg.fuzz && tick % 3 === 0) fuzzBattles();
    const bt = battles();
    if (!bt.length) return;
    this.on = true;
    this.before = { ido2: A.Ido2.readS32(), flags: fleetFlags(), battles: bt,
      ships: hex(A.Hajok.readPointer(), 6 * (A.HajoCount.readS32() + 1)) };
  },
  onLeave() {
    if (!this.on) return;
    emit({ k: 'mark', seq: recorded++, before: this.before, after: { flags: fleetFlags(), battles: battles() } });
  },
});

Interceptor.attach(A.rndlog, {
  onEnter(args) {
    if (!rnds) return;
    this.on = true;
    this.s = args[0].toInt32(); this.lo = args[1].toInt32(); this.hi = args[2].toInt32(); this.line = args[4].toInt32();
  },
  onLeave(ret) {
    if (this.on && rnds) rnds.push({ s: this.s, lo: this.lo, hi: this.hi, line: this.line, r: ret.toInt32() });
    this.on = false;
  },
});

// ---------------------------------------------------------------- driver
let frames = 0, started = false, phase = 0, done = 0, loadedAt = 0, populated = false, misses = 0;
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
    if (!started || frames < loadedAt + 300 || phase > 2) return;
    if (frames % cfg.every !== 0) return;
    try {
      if (cfg.mode === 'design') {
        designProbe(cfg.count); phase = 3; emit({ k: 'done' });
      } else if (cfg.mode === 'auto') {
        if (!populated) { populate(); populated = true; return; }
        if (autoProbe()) done++;
        if (done >= cfg.count) { phase = 3; emit({ k: 'done' }); }
      } else if (cfg.mode === 'indit') {
        if (!populated) { populate(); populated = true; return; }
        if (phase === 0) {
          if (inditProbe()) { done++; misses = 0; } else misses++;
          if (done >= cfg.count || misses > 200) { phase = 2; emit({ k: 'probes', done }); }
        }
        else if (phase === 2 && recorded >= cfg.ticks) { phase = 3; emit({ k: 'done' }); }
      }
    } catch (e) { emit({ k: 'error', msg: e.toString(), stack: e.stack }); emit({ k: 'done' }); phase = 3; }
  },
});

send({ type: 'ready', base: mod.base.toString() });
