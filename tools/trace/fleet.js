// Frida agent: capture fleet movement (FRISSIT_Starmap, GOG ig2.exe 0x594ae0) tick by tick.
// For each sampled sim tick it snapshots all fleets, the waypoint pool, wormholes, planet positions
// and trade buildings, relations, RNG streams and globals before and after the call, plus ships and
// the designs they use on speed-refresh ticks (Ido2 % 10 == 1), and every rndlog made inside.
// Spec: re/rules/fleet-movement.md. Addresses are image VAs, rebased at runtime.
'use strict';

const IMAGE_BASE = 0x400000;
const mod = Process.getModuleByName('ig2.exe');
const va = (x) => mod.base.add(x - IMAGE_BASE);

const A = {
  Fake_FRISSITES: va(0x4de720),
  Betolt_Scenario: va(0x4f5ba0),
  SetIdoSpeed: va(0x541330),
  FRISSIT_Starmap: va(0x594ae0),
  FlottaMozgat: va(0x53d270),     // FlottaMozgat(fleet, order, kind, float x, float y), cdecl
  rndlog: va(0x542da0),
  RandSeed: va(0xbaf2d0),
  Ido: va(0xbae04c), Ido2: va(0xbae050), Mult: va(0xbae25c), Nap: va(0xbae048),
  Flottak: va(0xbb40a4), FlottaCount: va(0xbb40a8),       // pointer, count
  Waypointek: va(0xbb41a4), WaypointCount: va(0xbb60f0),  // pointer, count
  Hajok: va(0xbb4100), HajoCount: va(0xbb4104),           // pointer, count
  Tervek: va(0xb9aa18),                                   // pointer per empire
  Wormhole: va(0xbb60f8), WormholeCount: va(0xbb61a0),    // array (1-based, 0x1c), count
  Bolygok: va(0xb85ea8), HanyBolygo: va(0x912ac8),
  HanyEpuletLefoglalva: va(0xba9840), Epuletek: va(0xba9e90),
  Viszony: va(0xdabf38),          // [a][b] at a*0x2d2 + b*0x26; +0x24 ack bit, +0x25 level
  EN: va(0x912a3c), MultiPlayerMode: va(0xbae2c0), DEBUGX: va(0xbae587),
  AllVisible: va(0xf265d4), DebugSpeed: va(0xb77240),
  PlayerKi: va(0xbae560), PlayerFaj: va(0xbae574), BirodalomEl: va(0xbae6ac),
  StoryAdat: va(0xf273f8),
  TradeIncomeThisYear: va(0xbae150), // i32[19]
  // Radar and visibility (fleet-movement.md §6).
  RadarHatotav: va(0x913898),     // f32[6], followed by RadarHatotav2Init f32[8] @0x9138b0
  RadarHatotav2: va(0xbb61a8), RadarHatotav2Negyzet: va(0xbb6408), // f32[19][8]
  RadarHatotavSzorzo: va(0xbb6668), RadarHatotavSzorzoNegyzet: va(0xbb66b8), // f32[19]
  BirodalomIsmeri: va(0xbaebb8),  // u8[19][19]
  OsszHanyKolonizalo: va(0xbb4108), OsszHanyTerraformalo: va(0xbb4158), // i32[19]
  ProgramPlayers: va(0x912acc), ProgramPlayers2: va(0x912ad0),
  Talalmanyok: va(0xb9c0c8),      // live techs, 0x114 B; +0x45+e = per-empire state (5 developed)
  TutorialActive: va(0xf6915d), TutorialPaused: va(0xf66f2a), TutorialEvent: va(0xf66f40),
  // SetIdoLelassit (0x541270) inputs: SP speed lock, MP program speed.
  SpeedLocked: va(0xbad0d4), ProgramSebesseg: va(0xbae54c),
};

const cfg = { scenario: 'scenario/quick.scn', startAfterFrames: 1200, speedPlan: [], ticks: 1500,
  fuzz: false, fuzzRadar: false, fuzzApproach: false, mpRadar: false };
recv('config', (m) => { Object.assign(cfg, m.payload); });

let buf = [];
function emit(ev) { buf.push(ev); if (buf.length >= 10) flush(); }
function flush() { if (buf.length) { send({ type: 'batch', events: buf }); buf = []; } }
setInterval(flush, 300);

function hex(p, n) {
  const b = new Uint8Array(p.readByteArray(n));
  let s = '';
  for (let i = 0; i < b.length; i++) s += (b[i] < 16 ? '0' : '') + b[i].toString(16);
  return s;
}

// Finished buildings of a type that are working (SearchBuilding(p, type, true)).
function countBuildings(p, types) {
  const out = types.map(() => 0);
  const nb = A.HanyEpuletLefoglalva.add(4 * p).readS32();
  if (nb <= 0) return out;
  const arr = A.Epuletek.add(4 * p).readPointer();
  for (let j = 1; j <= nb; j++) {
    const b = arr.add(0x20 * j);
    const k = types.indexOf(b.readU8());
    if (k >= 0 && b.add(8).readS16() === 0 && (b.add(0x19).readU8() & 1)) out[k]++;
  }
  return out;
}

// Colony hub slots (type 1) that are finished (+8 == 0), working or not (radar level 1, §6.3).
function hubs(p) {
  const nb = A.HanyEpuletLefoglalva.add(4 * p).readS32();
  if (nb <= 0) return 0;
  const arr = A.Epuletek.add(4 * p).readPointer();
  let n = 0;
  for (let j = 1; j <= nb; j++) if (arr.add(0x20 * j).readU8() === 1 && arr.add(0x20 * j + 8).readS16() === 0) n++;
  return n;
}

function snapshot(withShips) {
  const fl = A.Flottak.readPointer(), nf = A.FlottaCount.readS32();
  const fleets = [];
  for (let i = 1; i <= nf; i++) {
    const f = fl.add(0x80 * i);
    fleets.push({ i, raw: hex(f, 0x80) }); // freed slots too: stale records are still read
  }
  const nw = A.WaypointCount.readS32();
  const s = {
    ido: A.Ido.readS32(), ido2: A.Ido2.readS32(), mult: A.Mult.readS32(), nap: A.Nap.readS32(),
    seeds: Array.from({ length: 21 }, (_, i) => A.RandSeed.add(4 * i).readU32()),
    en: A.EN.readS32(), mp: A.MultiPlayerMode.readU8(), debug: A.DEBUGX.readU8(),
    allVisible: A.AllVisible.readS32(), debugSpeed: A.DebugSpeed.readS32(),
    playerKi: hex(A.PlayerKi, 19), playerFaj: hex(A.PlayerFaj, 19), alive: hex(A.BirodalomEl, 19),
    story: hex(A.StoryAdat, 0x128),
    tradeIncome: Array.from({ length: 19 }, (_, e) => A.TradeIncomeThisYear.add(4 * e).readS32()),
    nfleet: nf, fleets,
    waypoints: nw > 0 ? hex(A.Waypointek.readPointer(), (nw + 1) * 12) : '',
    wormholes: hex(A.Wormhole, (A.WormholeCount.readS32() + 1) * 0x1c),
    // Radar (§6): live tables, who-met-whom, idle colony ship/terraformer pools, radar tech states.
    pp: A.ProgramPlayers.readS32(), pp2: A.ProgramPlayers2.readS32(),
    radarRanges: hex(A.RadarHatotav, 14 * 4),
    rh2: hex(A.RadarHatotav2, 0x260), rh2sq: hex(A.RadarHatotav2Negyzet, 0x260),
    rmul: hex(A.RadarHatotavSzorzo, 0x4c), rmulsq: hex(A.RadarHatotavSzorzoNegyzet, 0x4c),
    met: hex(A.BirodalomIsmeri, 361),
    colonyPool: hex(A.OsszHanyKolonizalo, 0x4c), terraPool: hex(A.OsszHanyTerraformalo, 0x4c),
    radarTech: [0x21, 0x22, 0x23, 0x24].map((t) => hex(A.Talalmanyok.add(0x114 * t + 0x45), 19)),
    tutorial: [A.TutorialActive.readU8(), A.TutorialPaused.readU8(), A.TutorialEvent.readS32()],
    speedLocked: A.SpeedLocked.readS32(), programSpeed: A.ProgramSebesseg.readS32(),
  };
  if (withShips) {
    const nh = A.HajoCount.readS32(), hp = A.Hajok.readPointer();
    s.ships = nh > 0 ? hex(hp, (nh + 1) * 6) : '';
    const designs = {};
    for (let i = 1; i <= nh; i++) {
      const fi = hp.add(6 * i).readS16(), d = hp.add(6 * i + 2).readS16();
      if (d === 0x7fff || fi < 1 || fi > nf) continue;
      const e = fl.add(0x80 * fi + 0x15).readU8();
      const key = e + ':' + d;
      if (!(key in designs)) designs[key] = hex(A.Tervek.add(4 * e).readPointer().add(0x84 * d), 0x84);
    }
    s.designs = designs;
  }
  return s;
}

function planets() {
  const n = A.HanyBolygo.readS32();
  const out = [];
  for (let i = 1; i <= n; i++) {
    const p = A.Bolygok.add(0x188 * i);
    const [centres, banks, ports] = countBuildings(i, [48, 49, 50]);
    out.push({ i, x: p.add(0x18).readFloat(), y: p.add(0x1c).readFloat(), owner: p.add(0x20).readU8(),
      flags64: p.add(0x64).readU8(), epidemic: p.add(0xb8).readS16(), centres, banks, ports,
      radar: p.add(0x6e).readU8(), notice: p.add(0xbb).readU8(), vis: hex(p.add(0xd8), 38), hub: hubs(i) });
  }
  return out;
}

function relations() {
  let s = '';
  for (let a = 0; a < 19; a++) for (let b = 0; b < 19; b++) s += hex(A.Viszony.add(a * 0x2d2 + b * 0x26 + 0x24), 2);
  return s;
}

const betolt = new NativeFunction(A.Betolt_Scenario, 'void', ['pointer'], 'mscdecl');
const setSpeed = new NativeFunction(A.SetIdoSpeed, 'void', ['int', 'int'], 'mscdecl');
const mozgat = new NativeFunction(A.FlottaMozgat, 'void', ['int', 'int', 'int', 'float', 'float'], 'mscdecl');
let frames = 0, started = false, t0 = 0, speedIdx = 0;
Interceptor.attach(A.Fake_FRISSITES, {
  onEnter() {
    frames++;
    if (!started && frames === cfg.startAfterFrames) {
      started = true;
      t0 = Date.now();
      betolt(Memory.allocAnsiString(cfg.scenario));
      emit({ k: 'loaded', ido: A.Ido.readS32() });
    }
    if (started) {
      const now = Date.now() - t0;
      while (speedIdx < cfg.speedPlan.length && now >= cfg.speedPlan[speedIdx].at) {
        setSpeed(cfg.speedPlan[speedIdx].speed, 1);
        speedIdx++;
      }
      // --fuzz-approach: the "enemy approaching" slow-down drops the clock to x1; go back to x16.
      if (cfg.fuzzApproach && A.Mult.readS32() !== 16) setSpeed(3, 1);
    }
  },
});

// --fuzz: every 40 ticks, give some fleets (traders included) random move orders (points, planets, other
// fleets, waypoint chains, patrols) and speed overrides, so the exe computes rarely seen movements.
let fz = 777;
const rnd = (n) => { fz = (Math.imul(fz, 1103515245) + 12345) >>> 0; return (fz >>> 8) % n; };
function fuzz() {
  const fl = A.Flottak.readPointer(), nf = A.FlottaCount.readS32(), np = A.HanyBolygo.readS32();
  // Trade buildings (types 48 centre, 49 bank, 50 port) on random owned planets: trader weights/income.
  for (let k = 0; k < 3; k++) {
    const p = 1 + rnd(np);
    const nb = A.HanyEpuletLefoglalva.add(4 * p).readS32();
    if (A.Bolygok.add(0x188 * p + 0x20).readU8() === 0 || nb < 1) continue;
    const b = A.Epuletek.add(4 * p).readPointer().add(0x20 * (1 + rnd(nb)));
    if (b.readU8() !== 0 && b.add(8).readS16() === 0) b.writeU8([48, 49, 50][rnd(3)]);
  }
  for (let i = 1; i <= nf; i++) {
    const f = fl.add(0x80 * i);
    const owner = f.add(0x15).readU8();
    if (owner === 0xff || rnd(2) !== 0) continue;
    if (rnd(6) === 0) f.add(0x3c).writeFloat(rnd(2) ? 0 : 0.5 + rnd(40) / 10);
    // Targets near the fleet, so legs finish and waypoint chains advance and loop.
    const fx = f.add(0x30).readFloat(), fy = f.add(0x34).readFloat();
    const near = () => [fx + rnd(61) - 30, fy + rnd(61) - 30];
    const [x, y] = near();
    switch (rnd(5)) {
      case 0: mozgat(i, 1, 0, x, y); break;
      case 1: mozgat(i, 1, 1, 1 + rnd(np), 0); break;
      case 2: { const t = 1 + rnd(nf); if (t !== i && fl.add(0x80 * t + 0x15).readU8() !== 0xff) mozgat(i, 1, 2, t, 0); break; }
      case 3: mozgat(i, 1, 0, x, y); for (let k = 0; k < 1 + rnd(3); k++) { const [wx, wy] = near(); mozgat(i, 13, 0, wx, wy); } break;
      default: mozgat(i, 1, 0, x, y); for (let k = 0; k < 2; k++) { const [wx, wy] = near(); mozgat(i, 14, 0, wx, wy); } break;
    }
  }
}

// --fuzz-radar: every 40 ticks, perturb what the radar pass reads (§6): alliances, planet owners,
// radar stations (+0x6e >= 2), planet visibility and story notices, who-met-whom, radar techs, design
// radar/stealth, fleet hidden/ignored/notice bits, radar-booster story ids and the story radar fields.
function fuzzRadar() {
  const fl = A.Flottak.readPointer(), nf = A.FlottaCount.readS32(), np = A.HanyBolygo.readS32();
  const pp2 = A.ProgramPlayers2.readS32();
  for (let k = 0; k < 2; k++) {
    const a = 1 + rnd(8), b = 1 + rnd(8);
    if (a === b) continue;
    const lv = rnd(2) ? 5 : 1 + rnd(4);
    A.Viszony.add(a * 0x2d2 + b * 0x26 + 0x25).writeU8(lv);
    if (rnd(3)) A.Viszony.add(b * 0x2d2 + a * 0x26 + 0x25).writeU8(lv); // sometimes one-sided
  }
  for (let k = 0; k < 4; k++) {
    const p = A.Bolygok.add(0x188 * (1 + rnd(np)));
    switch (rnd(6)) {
      case 0: p.add(0x20).writeU8(rnd(9)); break;
      case 1: p.add(0x6e).writeU8(2 + rnd(6)); break;
      case 2: p.add(0x6e).writeU8(0); break;
      case 3: p.add(0xbb).writeU8(1); break;
      case 4: p.add(0xd8 + 2 * (1 + rnd(8))).writeS16(rnd(4) ? 0 : -1); break;
      default: p.add(0xd8 + 2 * p.add(0x20).readU8()).writeS16(rnd(2) ? 0 : -1); break;
    }
  }
  for (let k = 0; k < 6; k++) A.BirodalomIsmeri.add(19 * (1 + rnd(8)) + rnd(19)).writeU8(0);
  if (rnd(2)) {
    const e = 1 + rnd(pp2), t = 0x21 + rnd(4);
    A.Talalmanyok.add(0x114 * t + 0x45 + e).writeU8(rnd(2) ? 5 : 0);
  }
  const nh = A.HajoCount.readS32(), hp = A.Hajok.readPointer();
  for (let k = 0; k < 3 && nh > 0; k++) {
    const h = 1 + rnd(nh), fi = hp.add(6 * h).readS16(), d = hp.add(6 * h + 2).readS16();
    if (d === 0x7fff || fi < 1 || fi > nf) continue;
    const e = fl.add(0x80 * fi + 0x15).readU8();
    if (e === 0xff) continue;
    const des = A.Tervek.add(4 * e).readPointer().add(0x84 * d);
    des.add(0x6a).writeU8(rnd(6));
    des.add(0x6b).writeU8(rnd(3) ? 0 : 1);
  }
  for (let k = 0; k < 4; k++) {
    const f = fl.add(0x80 * (1 + rnd(nf)));
    if (f.add(0x15).readU8() === 0xff) continue;
    switch (rnd(5)) {
      case 0: f.add(0x52).writeU8(f.add(0x52).readU8() | 0x40); break;   // story radar notice
      case 1: f.add(0x52).writeU8(f.add(0x52).readU8() ^ 0x20); break;   // ignored by radar
      case 2: f.add(0x18).writeU8(f.add(0x18).readU8() ^ 1); break;      // hidden
      case 3: f.add(0x16).writeS16([1011, 1021, 1311][rnd(3)]); break;  // radar booster
      default: for (let e = 1; e <= 8; e++) f.add(0x19 + e).writeU8(0); break; // forget sightings
    }
  }
  const st = A.StoryAdat;
  switch (rnd(5)) {
    case 0: st.add(0x28).writeS32(rnd(2) ? 1 + rnd(nf) : 0); break; // tracked story fleet
    case 1: st.add(0x38).writeS32(rnd(2)); break;                 // human radar bonus
    case 2: st.add(0x48).writeS32(rnd(4) ? 0 : 1); break;         // no supply limit
    case 3: st.add(0x50).writeS32(rnd(2) ? 1 + rnd(np) : 0); break; // supply planet
    default: break;
  }
}

// --fuzz-approach: on refresh ticks, make 1-2 fleets an unseen enemy attacking the human (EN): owner an
// alive AI empire (or a trader, owner 9), attack order (2) on a human planet (kind 1) or a fleet turned
// human (kind 2), placed inside the human's radar circle of that planet with its EN sighting cleared, so
// the radar pass sends message 0x0E and calls SetIdoLelassit(1) before the movement loop (§6.3).
// Sometimes the SP speed lock (nemlehetidotallitani) is set for that tick (restored after the call).
// The next tick cancels the staged orders: an attack reaching the human opens the real-time battle
// (or ground war) screen, which stops strategic time.
let staged = [];
function fuzzApproach() {
  const fl = A.Flottak.readPointer(), nf = A.FlottaCount.readS32(), np = A.HanyBolygo.readS32();
  const en = A.EN.readS32();
  const own = [];
  for (let p = 1; p <= np; p++) {
    const b = A.Bolygok.add(0x188 * p);
    if (b.add(0x20).readU8() === en && b.add(0x6e).readU8() !== 0) own.push(p);
  }
  if (!own.length) return false;
  const live = [];
  for (let i = 1; i <= nf; i++) if (fl.add(0x80 * i + 0x15).readU8() !== 0xff) live.push(i);
  const ais = [];
  for (let e = 1; e <= 8; e++) if (e !== en && A.BirodalomEl.add(e).readU8()) ais.push(e);
  for (let k = 0; k < 1 + rnd(2) && live.length > 1; k++) {
    const i = live[rnd(live.length)], f = fl.add(0x80 * i);
    if (f.add(0x15).readU8() === en) continue;
    if (ais.length && rnd(3)) f.add(0x15).writeU8(ais[rnd(ais.length)]);
    f.add(0x18).writeU8(f.add(0x18).readU8() & ~1); // not hidden
    const p = own[rnd(own.length)], b = A.Bolygok.add(0x188 * p);
    const lv = b.add(0x6e).readU8();
    const r2 = A.RadarHatotav2Negyzet.add(4 * (8 * en + Math.min(lv, 7))).readFloat()
      * A.RadarHatotavSzorzoNegyzet.add(4 * en).readFloat();
    const rad = Math.sqrt(r2) * (0.3 + rnd(60) / 100), a = rnd(628) / 100;
    f.add(0x30).writeFloat(b.add(0x18).readFloat() + rad * Math.cos(a));
    f.add(0x34).writeFloat(b.add(0x1c).readFloat() + rad * Math.sin(a));
    f.add(0x40).writeS16(2);
    if (rnd(3)) {
      f.add(0x42).writeS16(1); f.add(0x44).writeFloat(p);
    } else {
      const t = live[rnd(live.length)];
      if (t === i) continue;
      fl.add(0x80 * t + 0x15).writeU8(en);
      f.add(0x42).writeS16(2); f.add(0x44).writeFloat(t);
    }
    f.add(0x48).writeFloat(0);
    if (f.add(0x16).readS16() === 0x1f88) f.add(0x16).writeS16(0);
    f.add(0x19 + en).writeU8(0);
    staged.push(i);
  }
  return true;
}

// --mp-radar: on half the refresh ticks, MultiPlayerMode (0xBAE2C0) is 1 for the whole FRISSIT_Starmap call
// (GetENRadarKifejlesztve runs at its start, 0x594afc) and restored after it, so the radar pass takes its MP branches
// (fleet-movement.md §6: no story radar bonus, unowned radar planets see for nobody, no SP extra circle); one or two
// planets are first made unowned radar stations.
function mpRadar() {
  const np = A.HanyBolygo.readS32();
  for (let k = 0; k < 1 + rnd(2); k++) {
    const p = A.Bolygok.add(0x188 * (1 + rnd(np)));
    p.add(0x20).writeU8(0);
    p.add(0x6e).writeU8(2 + rnd(6));
  }
  A.MultiPlayerMode.writeU8(1);
}

let recorded = 0, rnds = null, ticks = 0;
Interceptor.attach(A.FRISSIT_Starmap, {
  onEnter() {
    if (!started || recorded >= cfg.ticks) { this.on = false; return; }
    ticks++;
    if (cfg.fuzz && ticks % 40 === 0) fuzz();
    if (cfg.fuzzRadar && ticks % 40 === 25) fuzzRadar();
    this.on = true;
    const ido2 = A.Ido2.readS32();
    if (staged.length && ido2 % 10 !== 1) {
      for (const i of staged) if (A.Flottak.readPointer().add(0x80 * i + 0x15).readU8() !== 0xff) mozgat(i, 0, 0, 0, 0);
      staged = [];
    }
    this.unlock = false;
    if (cfg.fuzzApproach && ido2 % 10 === 1 && rnd(3) !== 0 && fuzzApproach() && rnd(5) === 0
        && A.SpeedLocked.readS32() === 0) {
      A.SpeedLocked.writeS32(1);
      this.unlock = true;
    }
    this.withShips = ido2 % 10 === 1;
    this.mp = cfg.mpRadar && ido2 % 10 === 1 && rnd(2) === 0;
    if (this.mp) mpRadar();
    this.before = snapshot(this.withShips);
    this.planets = planets();
    this.rel = relations();
    rnds = [];
  },
  onLeave() {
    if (!this.on) return;
    const after = snapshot(false);
    if (this.mp) A.MultiPlayerMode.writeU8(0);
    if (this.unlock) A.SpeedLocked.writeS32(0);
    emit({ k: 'tick', seq: recorded++, before: this.before, after, planets: this.planets,
      planetsAfter: planets(), relations: this.rel, rnds });
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
    if (this.on && rnds) rnds.push({ s: this.s, lo: this.lo, hi: this.hi, line: this.line, r: ret.toInt32() });
  },
});

send({ type: 'ready', base: mod.base.toString() });
